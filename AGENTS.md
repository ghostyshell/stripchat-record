# stripchat-record — agent guide

## Project overview

Records Stripchat live WebRTC streams at source resolution without loading the site UI.

| File | Role |
| --- | --- |
| `record_webrtc.py` | CLI entrypoint: fetches stream metadata, drives Playwright, saves recording |
| `webrtc_client.html` | Minimal in-browser WebRTC client (Doppio signaling + `MediaRecorder`) |
| `run.sh` / `run.bat` | Launchers: create venv, install deps, run recorder |
| `requirements.txt` | Python deps: `playwright`, `httpx` |
| `AGENTS.md` | Source of truth for agent instructions (read by Claude Code, Cursor, OpenCode) |
| `CLAUDE.md` | Thin pointer so Claude Code loads this file |
| `CHANGELOG.md` | Release history |
| `.cursor/rules/*.mdc` | Cursor rules: `graphify`, `sync-docs` |
| `.githooks/pre-push` | Version-controlled hook that nudges `/sync-docs` before publishing |
| `scripts/install-hooks.sh` | One-shot installer pointing git at `.githooks/` |

### Data flow

1. `GET /api/front/v1/broadcasts/{model}` → `streamName`, `webRTCAppKey`, native `width`/`height`
2. Playwright loads `webrtc_client.html` (blank page, no Stripchat chrome); viewport is matched to the stream's aspect ratio
3. WebSocket `wss://edge-webrtc.doppiocdn.com/` → WebRTC negotiation
4. Quality upgrade: `240p` → max profile (`source`, typically 1280×720)
5. `MediaRecorder` captures raw stream; `SegmentWriter` rotates files every `--split-size` (1 GiB default), producing `-partNNN.webm`
6. Stops when `--duration` elapses, or when the stream ends (graceful Ctrl+C saves the current part)
7. ffmpeg remuxes each part to `.mp4`; `.webm` parts are deleted after successful remux

## Commands

```bash
# Setup (first time)
python3 -m venv .venv
source .venv/bin/activate          # Windows: .venv\Scripts\activate
pip install -r requirements.txt
python -m playwright install chromium

# Record until stream ends
./run.sh MODEL_USERNAME

# Record 60 seconds to custom directory
./run.sh MODEL_USERNAME --duration 60 --output-dir ~/Videos/recordings

# Record a single file (disable the default 1 GiB splitting)
./run.sh MODEL_USERNAME --split-size 0
```

Windows: `run.bat MODEL_USERNAME --duration 60 --output-dir D:\recordings`

Install the git hooks once after cloning (enables the pre-push sync-docs reminder):

```bash
sh scripts/install-hooks.sh
```

## CLI flags

| Flag | Default | Purpose |
| --- | --- | --- |
| `model` | `roxyheartley` | Stripchat model username |
| `--duration` | _(none)_ | Seconds to record; omit to record until stream ends |
| `--split-size` | `1G` | Rotate to a new file when a part reaches this size (`K`/`M`/`G`, or `0`/`none` for one file) |
| `--output-dir` | `./output` | Directory for output files |
| `--output` | _(auto)_ | Full output path; overrides `--output-dir` |
| `--headed` | off | Show the Chromium window (default: headless) |
| `--no-remux` | off | Keep `.webm` only; skip ffmpeg `.mp4` remux |

## Graphify

This repo includes a graphify knowledge graph at `graphify-out/` (gitignored; regenerate locally).

```bash
graphify .                  # initial build
graphify update .           # after code changes (AST-only, no API cost)
graphify query "how does recording work?"
```

Agents: see `.cursor/rules/graphify.mdc` — run graphify before exploring the codebase.

## Code conventions

- Keep `record_webrtc.py` as the single Python entrypoint; WebRTC logic stays in `webrtc_client.html`.
- Match existing argparse patterns when adding flags.
- Do not commit recordings (`.webm`, `.mp4`) or `.venv/`.
- After changing `record_webrtc.py` or `webrtc_client.html`, update `README.md`, `CHANGELOG.md`, and `AGENTS.md` if user-facing behaviour changes (use the `sync-docs` skill - see below).

## Keeping docs & instructions in sync

This file (`AGENTS.md`) is the **single source of truth** for agent instructions. `CLAUDE.md` is a thin pointer that makes Claude Code load it; Cursor also reads `.cursor/rules/*.mdc`. When you change instructions here, keep `CLAUDE.md` and the Cursor rules coherent.

### Before every push

1. Run the **`sync-docs`** skill (Claude Code / OpenCode) or follow `.cursor/rules/sync-docs.mdc` (Cursor) to audit `README.md`, `CHANGELOG.md`, `AGENTS.md`, and this file's file table / CLI table against the actual code.
2. Key facts to keep accurate: the **`--split-size`** flag (default `1G`, produces `-partNNN.webm`), the output path scheme, the CLI flags table, the data flow, and the project layout / file table.
3. Confirm no em/en dashes in public-facing copy: `rg '[—–]' README.md CHANGELOG.md` (the `humanizer` skill covers the wider de-AI pass for substantive copy).
4. The `.githooks/pre-push` hook prints a non-blocking reminder; install it once with `sh scripts/install-hooks.sh`.

### After code changes

- Run `graphify update .` to refresh the knowledge graph (AST-only, no API cost). Use `graphify query` before exploring the codebase.
- Run the **`code-reviewer`** agent after any substantive change; `security-auditor` for input/secret/network surface.
- Add a `CHANGELOG.md` entry under `[Unreleased]` for user-visible changes.

### Tool-specific notes

| Tool | Instructions file | Native rules |
| --- | --- | --- |
| Claude Code | `CLAUDE.md` → `AGENTS.md` | none needed |
| OpenCode | `AGENTS.md` | none needed |
| Cursor | `AGENTS.md` | `.cursor/rules/graphify.mdc`, `.cursor/rules/sync-docs.mdc` |

## External dependencies

- **Python 3.10+** with venv
- **ffmpeg** on `PATH` for remux (optional but default-on)
- **Chromium** via Playwright (`python -m playwright install chromium`)

## Git remote

| Remote | URL |
| --- | --- |
| `origin` | `https://github.com/ghostyshell/stripchat-record.git` |

Push with: `git push -u origin main`
