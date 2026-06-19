#!/usr/bin/env python3
"""Record Stripchat live stream (WebRTC media only, no page UI)."""

from __future__ import annotations

import argparse
import asyncio
import base64
import json
import re
import signal
import sys
import time
from datetime import datetime
from pathlib import Path
from urllib.parse import quote

import httpx
from playwright.async_api import async_playwright

DEFAULT_MODEL = "roxyheartley"
DEFAULT_SPLIT_SIZE = "1G"
OUTPUT_DIR = Path(__file__).resolve().parent / "output"
CLIENT_HTML = Path(__file__).resolve().parent / "webrtc_client.html"
WEBRTC_WS = "wss://edge-webrtc.doppiocdn.com/"


def format_bytes(num: int) -> str:
    if num >= 1_000_000_000:
        return f"{num / 1_000_000_000:.2f} GB"
    if num >= 1_000_000:
        return f"{num / 1_000_000:.1f} MB"
    if num >= 1_000:
        return f"{num / 1_000:.1f} KB"
    return f"{num} B"


def format_duration(seconds: float) -> str:
    whole = int(seconds)
    mins, secs = divmod(whole, 60)
    if mins:
        return f"{mins}m {secs:02d}s"
    return f"{secs}s"


def parse_split_size(raw: str | None) -> int | None:
    """Return split threshold in bytes, or None to disable splitting."""
    if raw is None:
        raw = DEFAULT_SPLIT_SIZE
    value = raw.strip().lower()
    if value in {"", "0", "none", "off", "no", "false", "-1"}:
        return None

    match = re.fullmatch(r"(\d+(?:\.\d+)?)([kmg]?)(?:ib|b)?", value)
    if not match:
        raise argparse.ArgumentTypeError(
            f"Invalid split size {raw!r}; use e.g. 1G, 512M, 1073741824, or 0/none"
        )

    amount = float(match.group(1))
    unit = match.group(2) or ""
    if unit == "k":
        size = int(amount * 1024)
    elif unit == "m":
        size = int(amount * 1024 * 1024)
    elif unit == "g":
        size = int(amount * 1024 * 1024 * 1024)
    else:
        size = int(amount)

    if size <= 0:
        return None
    return size


def viewport_for_stream(width: int | None, height: int | None) -> dict[str, int]:
    """Size the browser window to the stream's native aspect ratio."""
    if width and height and width > 0 and height > 0:
        w, h = int(width), int(height)
        cap = 1920
        longest = max(w, h)
        if longest > cap:
            scale = cap / longest
            w = max(2, int(w * scale))
            h = max(2, int(h * scale))
        # Playwright prefers even dimensions.
        if w % 2:
            w += 1
        if h % 2:
            h += 1
        return {"width": w, "height": h}
    return {"width": 1280, "height": 720}


def part_path(base: Path, part: int) -> Path:
    return base.with_name(f"{base.stem}-part{part:03d}{base.suffix}")


class SegmentWriter:
    def __init__(self, base_path: Path) -> None:
        self.base_path = base_path
        self.paths: list[Path] = []
        self._seen: set[int] = set()

    def path_for(self, part: int) -> Path:
        return part_path(self.base_path, part)

    def write_chunk(self, part: int, chunk_index: int, b64: str) -> None:
        path = self.path_for(part)
        data = base64.b64decode(b64)
        mode = "wb" if chunk_index == 0 else "ab"
        with path.open(mode) as handle:
            handle.write(data)
        if part not in self._seen:
            self._seen.add(part)
            self.paths.append(path)

    def note_part(self, part: int, size: int, reason: str) -> None:
        path = self.path_for(part)
        print(f"\n  Saved part {part}: {path} ({format_bytes(size)}, {reason})")


async def fetch_stream_info(model: str) -> dict:
    url = f"https://stripchat.com/api/front/v1/broadcasts/{quote(model)}"
    headers = {
        "User-Agent": (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
            "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
        ),
        "Accept": "application/json",
        "Referer": f"https://stripchat.com/{model}",
    }
    async with httpx.AsyncClient(timeout=20.0, follow_redirects=True) as client:
        resp = await client.get(url, headers=headers)
        resp.raise_for_status()
        payload = resp.json()

    item = payload.get("item") or {}
    stream_name = item.get("streamName")
    if not stream_name:
        raise RuntimeError(f"Model {model!r} is not live (no streamName in API response)")

    settings = item.get("settings") or {}
    return {
        "model": model,
        "stream_name": str(stream_name),
        "transport": settings.get("mediaTransport", "unknown"),
        "web_rtc_app_key": item.get("webRTCAppKey", "callbackApp"),
        "is_live": item.get("isLive", False),
        "width": settings.get("width"),
        "height": settings.get("height"),
    }


def save_recording_result(
    result: dict,
    output_path: Path,
    segment_writer: SegmentWriter | None,
    split_size_bytes: int | None,
    elapsed: float,
) -> list[Path]:
    if result.get("cancelled"):
        if segment_writer and segment_writer.paths:
            paths = segment_writer.paths
            total_size = sum(path.stat().st_size for path in paths if path.exists())
            print(
                f"Saved {len(paths)} partial part(s) ({format_bytes(total_size)}) "
                f"in {elapsed:.1f}s, stopped: interrupted"
            )
            for path in paths:
                print(f"Saved: {path}")
            return paths
        print("Stopped before any recording was saved.")
        return []

    end_reason = result.get("endReason", "unknown")
    width = result.get("width")
    height = result.get("height")
    quality = result.get("quality", "unknown")
    resolution = f"{width}x{height}" if width and height else "unknown"
    fallback = result.get("qualityFallback")
    quality_note = f"{quality}, fallback={fallback}" if fallback else quality

    if split_size_bytes:
        paths = segment_writer.paths if segment_writer else []
        total_size = sum(path.stat().st_size for path in paths if path.exists())
        print(
            f"Recorded {len(paths)} part(s) at {resolution} "
            f"({quality_note}, {format_bytes(total_size)} total) "
            f"in {elapsed:.1f}s, stopped: {end_reason}"
        )
        for path in paths:
            print(f"Saved: {path}")
        return paths

    raw = base64.b64decode(result["base64"])
    output_path.write_bytes(raw)
    print(
        f"Recorded stream-only {result['mimeType']} at {resolution} "
        f"({quality_note}, {len(raw):,} bytes) in {elapsed:.1f}s, stopped: {end_reason}"
    )
    print(f"Saved: {output_path}")
    return [output_path]


async def record_direct_webrtc(
    info: dict,
    duration_sec: int | None,
    headless: bool,
    output_path: Path,
    split_size_bytes: int | None,
) -> list[Path]:
    """Connect to Doppio WebRTC directly and record the raw MediaStream."""
    duration_ms = duration_sec * 1000 if duration_sec is not None else None
    html = CLIENT_HTML.read_text(encoding="utf-8")
    segment_writer = SegmentWriter(output_path) if split_size_bytes else None
    viewport = viewport_for_stream(info.get("width"), info.get("height"))

    async with async_playwright() as p:
        launch_args = ["--autoplay-policy=no-user-gesture-required"]
        if not headless:
            launch_args.append(
                f"--window-size={viewport['width']},{viewport['height']}"
            )
        browser = await p.chromium.launch(
            headless=headless,
            args=launch_args,
        )
        context = await browser.new_context(
            viewport=viewport,
            user_agent=(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
            ),
        )
        page = await context.new_page()

        def on_progress(message: str) -> None:
            print(f"  [{message}]")

        def on_recording_stats(
            duration_sec: float,
            part_bytes: int,
            total_bytes: int,
            part_index: int,
            target_duration_sec: float | None,
        ) -> None:
            duration = format_duration(duration_sec)
            part_size = format_bytes(part_bytes)
            total_size = format_bytes(total_bytes)
            if split_size_bytes:
                size_note = f"part {part_index}: {part_size}, total {total_size}"
            else:
                size_note = part_size
            if target_duration_sec:
                target = format_duration(target_duration_sec)
                line = f"  Recording: {duration} / {target}, {size_note}"
            else:
                line = f"  Recording: {duration}, {size_note}"
            print(f"\r{line}", end="", flush=True)

        async def on_segment_chunk(
            part: int, chunk_index: int, b64: str, _is_first: bool, _is_last: bool
        ) -> None:
            assert segment_writer is not None
            segment_writer.write_chunk(part, chunk_index, b64)

        async def on_segment_complete(part: int, size: int, reason: str) -> None:
            assert segment_writer is not None
            segment_writer.note_part(part, size, reason)

        async def on_stream_dimensions(width: int, height: int) -> None:
            vp = viewport_for_stream(width, height)
            await page.set_viewport_size(vp)
            on_progress(f"Viewport matched to stream: {vp['width']}x{vp['height']}")

        await page.expose_function("__onProgress", on_progress)
        await page.expose_function("__onRecordingStats", on_recording_stats)
        await page.expose_function("__onStreamDimensions", on_stream_dimensions)
        if split_size_bytes:
            await page.expose_function("__onSegmentChunk", on_segment_chunk)
            await page.expose_function("__onSegmentComplete", on_segment_complete)
        await page.set_content(html, wait_until="domcontentloaded")

        if duration_sec is None:
            duration_note = "recording until the stream ends"
        else:
            duration_note = f"recording {duration_sec}s"
        if split_size_bytes:
            split_note = f", split at {format_bytes(split_size_bytes)}"
        else:
            split_note = ""
        print(
            f"Connecting WebRTC to stream {info['stream_name']} "
            f"via {WEBRTC_WS} ({duration_note}{split_note}, "
            f"viewport {viewport['width']}x{viewport['height']}) ..."
        )
        started = time.monotonic()
        evaluate_args = {
            "streamName": info["stream_name"],
            "appKey": info["web_rtc_app_key"],
            "wsUrl": WEBRTC_WS,
            "durationMs": duration_ms,
            "splitSizeBytes": split_size_bytes,
        }
        record_task = asyncio.create_task(
            page.evaluate(
                """
                async (args) => {
                  return window.startStreamRecording(args);
                }
                """,
                evaluate_args,
            )
        )

        stop_attempts = {"count": 0}

        async def request_browser_stop() -> None:
            try:
                await page.evaluate("window.requestStopRecording()")
            except Exception:
                pass

        loop = asyncio.get_running_loop()

        def sigint_handler() -> None:
            stop_attempts["count"] += 1
            if stop_attempts["count"] == 1:
                print("\n  Ctrl+C — stopping recording and saving file ...", flush=True)
                loop.create_task(request_browser_stop())
            else:
                print("\n  Force quit.", flush=True)
                record_task.cancel()

        loop.add_signal_handler(signal.SIGINT, sigint_handler)
        result: dict | None = None
        try:
            result = await record_task
        except asyncio.CancelledError:
            raise KeyboardInterrupt from None
        except Exception as exc:
            if segment_writer and segment_writer.paths:
                result = {"endReason": "interrupted", "split": True, "cancelled": False}
            elif "stopped by user" in str(exc).lower():
                print(f"\n  {exc}")
                result = {"cancelled": True, "endReason": "interrupted"}
            else:
                raise
        finally:
            try:
                loop.remove_signal_handler(signal.SIGINT)
            except Exception:
                pass

        elapsed = time.monotonic() - started
        await context.close()
        await browser.close()

    print()
    assert result is not None
    return save_recording_result(
        result, output_path, segment_writer, split_size_bytes, elapsed
    )


async def maybe_reencode(input_path: Path) -> Path | None:
    mp4_path = input_path.with_suffix(".mp4")
    proc = await asyncio.create_subprocess_exec(
        "ffmpeg",
        "-y",
        "-i",
        str(input_path),
        "-c",
        "copy",
        str(mp4_path),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    _, stderr = await proc.communicate()
    if proc.returncode != 0:
        print("ffmpeg remux skipped:", stderr.decode(errors="replace")[-400:])
        return None
    print(f"Remuxed: {mp4_path}")
    if input_path.suffix.lower() == ".webm" and input_path.exists():
        input_path.unlink()
        print(f"Removed: {input_path}")
    return mp4_path


async def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", nargs="?", default=DEFAULT_MODEL)
    parser.add_argument(
        "--duration",
        type=int,
        default=None,
        help="Seconds to record (default: record until the stream ends)",
    )
    parser.add_argument(
        "--split-size",
        type=parse_split_size,
        default=parse_split_size(DEFAULT_SPLIT_SIZE),
        metavar="SIZE",
        help=(
            "Start a new file when a part reaches this size "
            f"(default: {DEFAULT_SPLIT_SIZE}; use 0 or none to disable)"
        ),
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Directory for recordings (default: ./output)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Full output file path (overrides --output-dir)",
    )
    parser.add_argument("--headless", action="store_true")
    parser.add_argument("--no-remux", action="store_true")
    args = parser.parse_args()

    output_dir = (args.output_dir or OUTPUT_DIR).expanduser().resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    if args.output:
        output_path = args.output.expanduser().resolve()
        output_path.parent.mkdir(parents=True, exist_ok=True)
    else:
        output_path = output_dir / f"{args.model}-stream-{stamp}.webm"

    print("Fetching stream metadata ...")
    try:
        info = await fetch_stream_info(args.model)
    except Exception as exc:
        print(f"Failed: {exc}", file=sys.stderr)
        return 1

    print(json.dumps(info, indent=2))

    try:
        saved_paths = await record_direct_webrtc(
            info=info,
            duration_sec=args.duration,
            headless=args.headless,
            output_path=output_path,
            split_size_bytes=args.split_size,
        )
    except KeyboardInterrupt:
        print("\nRecording aborted.", file=sys.stderr)
        return 130
    except Exception as exc:
        print(f"Recording failed: {exc}", file=sys.stderr)
        return 1

    if saved_paths and not args.no_remux:
        for path in saved_paths:
            await maybe_reencode(path)

    return 0


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
