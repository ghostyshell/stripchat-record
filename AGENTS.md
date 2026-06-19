# stripchat-record — agent guide

## Project overview

Records Stripchat live WebRTC streams at source resolution without loading the site UI.

| File | Role |
| --- | --- |
| `record_webrtc.py` | CLI entrypoint: fetches stream metadata, drives Playwright, saves recording |
| `webrtc_client.html` | Minimal in-browser WebRTC client (Doppio signaling + `MediaRecorder`) |
| `run.sh` | Unix launcher: creates venv, installs deps, runs recorder |
| `run.bat` | Windows launcher (same as `run.sh`) |
| `requirements.txt` | Python deps: `playwright`, `httpx` |

### Data flow

1. `GET /api/front/v1/broadcasts/{model}` → `streamName`, `webRTCAppKey`
2. Playwright loads `webrtc_client.html` (blank page, no Stripchat chrome)
3. WebSocket `wss://edge-webrtc.doppiocdn.com/` → WebRTC negotiation
4. Quality upgrade: `240p` → max profile (`source`, typically 1280×720)
5. `MediaRecorder` captures raw stream until duration elapses or stream ends
6. Optional ffmpeg remux to `.mp4`; `.webm` is deleted after successful remux

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
```

Windows: `run.bat MODEL_USERNAME --duration 60 --output-dir D:\recordings`

## CLI flags

| Flag | Default | Purpose |
| --- | --- | --- |
| `model` | `roxyheartley` | Stripchat model username |
| `--duration` | _(none)_ | Seconds to record; omit to record until stream ends |
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
- After changing `record_webrtc.py` or `webrtc_client.html`, update `README.md` if user-facing behaviour changes.

## External dependencies

- **Python 3.10+** with venv
- **ffmpeg** on `PATH` for remux (optional but default-on)
- **Chromium** via Playwright (`python -m playwright install chromium`)

## Git remote

| Remote | URL |
| --- | --- |
| `origin` | `https://github.com/ghostyshell/stripchat-record.git` |

Push with: `git push -u origin main`
