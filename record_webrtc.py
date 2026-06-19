#!/usr/bin/env python3
"""Record Stripchat live stream (WebRTC media only, no page UI)."""

from __future__ import annotations

import argparse
import asyncio
import base64
import json
import sys
import time
from datetime import datetime
from pathlib import Path
from urllib.parse import quote

import httpx
from playwright.async_api import async_playwright

DEFAULT_MODEL = "roxyheartley"
OUTPUT_DIR = Path(__file__).resolve().parent / "output"
CLIENT_HTML = Path(__file__).resolve().parent / "webrtc_client.html"
WEBRTC_WS = "wss://edge-webrtc.doppiocdn.com/"


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
    }


async def record_direct_webrtc(
    info: dict,
    duration_sec: int | None,
    headless: bool,
    output_path: Path,
) -> Path:
    """Connect to Doppio WebRTC directly and record the raw MediaStream."""
    duration_ms = duration_sec * 1000 if duration_sec is not None else None
    html = CLIENT_HTML.read_text(encoding="utf-8")

    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=headless,
            args=["--autoplay-policy=no-user-gesture-required"],
        )
        context = await browser.new_context(
            viewport={"width": 1920, "height": 1080},
            user_agent=(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
            ),
        )
        page = await context.new_page()
        await page.expose_function("__onProgress", lambda message: print(f"  [{message}]"))
        await page.set_content(html, wait_until="domcontentloaded")

        if duration_sec is None:
            print(
                f"Connecting WebRTC to stream {info['stream_name']} "
                f"via {WEBRTC_WS} (recording until stream ends) ..."
            )
        else:
            print(
                f"Connecting WebRTC to stream {info['stream_name']} "
                f"via {WEBRTC_WS} (recording {duration_sec}s) ..."
            )
        started = time.monotonic()
        result = await page.evaluate(
            """
            async (args) => {
              return window.startStreamRecording(args);
            }
            """,
            {
                "streamName": info["stream_name"],
                "appKey": info["web_rtc_app_key"],
                "wsUrl": WEBRTC_WS,
                "durationMs": duration_ms,
            },
        )
        elapsed = time.monotonic() - started
        await context.close()
        await browser.close()

    raw = base64.b64decode(result["base64"])
    output_path.write_bytes(raw)
    end_reason = result.get("endReason", "unknown")
    width = result.get("width")
    height = result.get("height")
    quality = result.get("quality", "unknown")
    resolution = f"{width}x{height}" if width and height else "unknown"
    fallback = result.get("qualityFallback")
    quality_note = f"{quality}, fallback={fallback}" if fallback else quality
    print(
        f"Recorded stream-only {result['mimeType']} at {resolution} "
        f"({quality_note}, {len(raw):,} bytes) in {elapsed:.1f}s, stopped: {end_reason}"
    )
    print(f"Saved: {output_path}")
    return output_path


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
        await record_direct_webrtc(
            info=info,
            duration_sec=args.duration,
            headless=args.headless,
            output_path=output_path,
        )
    except Exception as exc:
        print(f"Recording failed: {exc}", file=sys.stderr)
        return 1

    if not args.no_remux:
        await maybe_reencode(output_path)

    return 0


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
