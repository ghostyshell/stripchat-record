#!/usr/bin/env python3
"""Rewrite the README download table from an R2 upload manifest.

Usage: python3 -I scripts/patch-release-links.py <links.txt> [version]

Rows are `name | url | size | sha256`. A row whose URL is missing or EMPTY
still appears with `pending`, so a failed upload never blanks out a row that
worked. Writes docs/downloads.json, which the site reads at runtime.
"""
import datetime
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
README = ROOT / "README.md"
MANIFEST = ROOT / "docs" / "downloads.json"
START = "<!-- downloads:start -->"
END = "<!-- downloads:end -->"

# The PS3/Switch CLIs ship an MCP server too; their labels say so.
CLI_SUFFIX = ""


def label_for(name):
    low = name.lower()
    if low.endswith(".dmg"):
        return "macOS (Intel)" if "x64" in low else "macOS (Apple Silicon)"
    if low.endswith(".tar.gz"):
        if "linux" in low:
            return f"Linux (x86_64, CLI{CLI_SUFFIX})"
        arch = "Intel" if "x64" in low else "Apple Silicon"
        return f"macOS ({arch}, CLI{CLI_SUFFIX})"
    if low.endswith(".zip"):
        return f"Windows (x86_64, CLI{CLI_SUFFIX})"
    if low.endswith(".appimage"):
        return "Linux (x86_64, AppImage)"
    if low.endswith(".deb"):
        return "Linux (x86_64, deb)"
    if low.endswith(".exe"):
        return "Windows (x86_64, installer)"
    return name


def cell(url):
    return f"[Download]({url})" if url and url != "EMPTY" else "pending"


def order(name):
    low = name.lower()
    if low.endswith(".dmg"):
        return (0, name)
    if low.endswith(".exe"):
        return (1, name)
    if low.endswith(".appimage"):
        return (2, name)
    if low.endswith(".deb"):
        return (3, name)
    return (4, name)


def load(path):
    rows = []
    for line in Path(path).read_text().splitlines():
        parts = [p.strip() for p in line.split("|")]
        if len(parts) >= 2 and parts[0]:
            rows.append((parts + [""] * 4)[:4])
    return rows


def build(rows, version):
    artifacts = [
        {
            "name": name,
            "label": label_for(name),
            "url": "" if url == "EMPTY" else url,
            "size": size,
            "sha256": sha,
        }
        for name, url, size, sha in sorted(rows, key=lambda r: order(r[0]))
    ]
    if not version and artifacts:
        match = re.search(r"\d+\.\d+\.\d+", artifacts[0]["name"])
        if match:
            version = match.group(0)
    return {"version": version, "updated": datetime.date.today().isoformat(), "artifacts": artifacts}


def table(manifest):
    lines = ["| Build | Download | Size |", "| --- | --- | --- |"]
    for art in manifest["artifacts"]:
        lines.append(f"| `{art['name']}` | {cell(art['url'])} | {art['size'] or 'pending'} |")
    return "\n".join(lines)


def patch_readme(manifest):
    text = README.read_text()
    start = text.index(START) + len(START)
    end = text.index(END)
    README.write_text(text[:start] + "\n" + table(manifest) + "\n" + text[end:])


def demo():
    rows = [
        ("godsend-x-1.0.0-arm64.dmg", "https://covers.debrid.in/x/releases/1.0.0/godsend-x-1.0.0-arm64.dmg", "5.4 MiB", "a"),
        ("godsend-x-cli-1.0.0-windows-x64.zip", "EMPTY", "2.1 MiB", "b"),
    ]
    m = build(rows, "")
    assert m["version"] == "1.0.0", m["version"]
    t = table(m)
    assert "| `godsend-x-1.0.0-arm64.dmg` | [Download](https://covers.debrid.in/x/releases/1.0.0/godsend-x-1.0.0-arm64.dmg) | 5.4 MiB |" in t, t
    assert "| `godsend-x-cli-1.0.0-windows-x64.zip` | pending | 2.1 MiB |" in t, t
    assert m["artifacts"][1]["url"] == "" and m["artifacts"][0]["url"].startswith("https://"), m
    print("demo OK")


if __name__ == "__main__":
    if "--demo" in sys.argv:
        demo()
    elif len(sys.argv) < 2:
        raise SystemExit(__doc__)
    else:
        rows = load(sys.argv[1])
        if not rows:
            raise SystemExit("no usable rows in the upload manifest")
        manifest = build(rows, sys.argv[2] if len(sys.argv) > 2 else "")
        MANIFEST.parent.mkdir(parents=True, exist_ok=True)
        MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n")
        patch_readme(manifest)
        print(f"patched {len(rows)} rows for {manifest['version']}")
