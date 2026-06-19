# stripchat-record

[![Discord](https://img.shields.io/badge/Discord-5865F2?style=for-the-badge&logo=discord&logoColor=white)](https://discord.gg/ckrEtDFwxP)
[![Ko-fi](https://img.shields.io/badge/Ko--fi-FF5E5B?style=for-the-badge&logo=ko-fi&logoColor=white)](https://ko-fi.com/ghosty99)

Record Stripchat live streams at **source resolution** via direct WebRTC — no page UI, chat, or sidebar in the output.

Connects to the Doppio CDN WebRTC edge (`wss://edge-webrtc.doppiocdn.com/`), upgrades to max quality (`source`), and saves a stream-only video file.

## Requirements

- **Python 3.10+**
- **ffmpeg** (for `.mp4` remux; [install guide](https://ffmpeg.org/download.html))
- **Chromium** (installed automatically via Playwright on first run)

## Quick start

### macOS / Linux

```bash
git clone https://github.com/ghostyshell/stripchat-record.git
cd stripchat-record
chmod +x run.sh
./run.sh MODEL_USERNAME
```

### Windows

```cmd
git clone https://github.com/ghostyshell/stripchat-record.git
cd stripchat-record
run.bat MODEL_USERNAME
```

Replace `MODEL_USERNAME` with the model's Stripchat username (e.g. `roxyheartley`).

The first run creates a virtual environment, installs Python packages, and downloads Chromium.

## Usage

```bash
# Record until the model goes offline (default)
./run.sh MODEL_USERNAME

# Record for 60 seconds
./run.sh MODEL_USERNAME --duration 60

# Save to a custom directory
./run.sh MODEL_USERNAME --output-dir ~/Videos/recordings

# Full output path (overrides --output-dir)
./run.sh MODEL_USERNAME --output /path/to/recording.webm

# Show the browser window (default is headless)
./run.sh MODEL_USERNAME --headed

# Keep .webm only (skip ffmpeg remux)
./run.sh MODEL_USERNAME --no-remux
```

**Windows:** use `run.bat` instead of `./run.sh` with the same arguments.

### Manual setup (all platforms)

If you prefer not to use the launcher scripts:

```bash
# macOS / Linux
python3 -m venv .venv
source .venv/bin/activate

# Windows (Command Prompt)
python -m venv .venv
.venv\Scripts\activate

# Windows (PowerShell)
py -3 -m venv .venv
.venv\Scripts\Activate.ps1

# All platforms (with venv active)
pip install -r requirements.txt
python -m playwright install chromium
python record_webrtc.py MODEL_USERNAME
```

On Windows, if `python` is not found, use `py -3` or `python3` instead.

## Output

| Default | Description |
| --- | --- |
| `./output/{model}-stream-{timestamp}.webm` | Raw recording |
| `./output/{model}-stream-{timestamp}.mp4` | Remuxed copy (`.webm` removed after success) |

Use `--output-dir` to change the directory, or `--output` for an explicit file path.

Example log line:

```
Recorded stream-only video/webm at 1280x720 (source, 12,345,678 bytes) in 65.2s, stopped: video_track_ended
```

## How it works

1. Fetches `streamName` from `GET /api/front/v1/broadcasts/{model}`
2. Opens a minimal local page (`webrtc_client.html`) in headless/headed Chromium
3. Negotiates WebRTC over `wss://edge-webrtc.doppiocdn.com/`
4. Upgrades quality to the highest available profile (usually `source` / 1280×720)
5. Records the raw `MediaStream` with `MediaRecorder`
6. Stops when `--duration` elapses, or when the stream ends (if no duration given)
7. Remuxes to `.mp4` with ffmpeg and deletes the intermediate `.webm`

## Platform notes

### macOS

```bash
brew install ffmpeg python
./run.sh MODEL_USERNAME
```

### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install -y python3 python3-venv ffmpeg
./run.sh MODEL_USERNAME
```

### Linux (Fedora)

```bash
sudo dnf install python3 ffmpeg
./run.sh MODEL_USERNAME
```

### Windows

1. Install [Python 3](https://www.python.org/downloads/) — check **"Add Python to PATH"** during setup
2. Install [ffmpeg](https://www.gyan.dev/ffmpeg/builds/) and add its `bin` folder to PATH
3. Open Command Prompt in the project folder:

```cmd
run.bat MODEL_USERNAME
```

If `python` is not recognized, the launcher falls back to `py -3`.

## Troubleshooting

| Problem | Fix |
| --- | --- |
| `command python not found` | Use `python3`, `py -3`, or activate the venv (`source .venv/bin/activate`) |
| `No module named 'httpx'` | Run via `./run.sh` / `run.bat`, or `pip install -r requirements.txt` inside `.venv` |
| `Model is not live` | The model must be streaming; check their page in a browser |
| `Timed out waiting for stream` | Model offline, geo-blocked, or network issue — retry with `--headed` to open a visible browser |
| `ffmpeg remux skipped` | Install ffmpeg and ensure it is on `PATH` |
| Low resolution output | Fixed in current version — waits for max quality before recording |

## Project layout

```
stripchat-record/
├── record_webrtc.py      # CLI entrypoint
├── webrtc_client.html    # WebRTC + MediaRecorder client
├── run.sh                # macOS / Linux launcher
├── run.bat               # Windows launcher
├── requirements.txt
├── README.md
├── AGENTS.md             # Agent / automation docs
└── output/               # Default recordings (gitignored)
```

## Agent / development docs

See [AGENTS.md](AGENTS.md) for architecture, graphify usage, and conventions for AI-assisted development.

## License

Use responsibly and in compliance with applicable laws and site terms of service.
