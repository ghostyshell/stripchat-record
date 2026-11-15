# Changelog

All notable changes to **stripchat-record** are listed here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.5.0] - 2026-11-15

### Added
- Tauri 2 + Rust desktop app for macOS, Windows and Linux, replacing the
  Python Playwright recorder. React frontend with Record, Library, Convert and
  Settings pages, a job queue with progress and cancel, and a `?mock=1`
  fixture bridge for UI work without a network.
- `stripchat-core` engine: site API + MOUFLON v2 URI decoding + fMP4 recording
- `stripchat-recorder-cli` headless CLI: `record`, `convert`, `presets`, `library`,
  `doctor`.
- Recording is append-only and per segment, so a dropped connection or a hard
  kill leaves a playable `.recording.mp4` instead of a corrupt file.
- Size-based splitting into `-partNNN` files, each with its own init segment.
- Conversion presets (H.264 / H.265 CRF, 720p downscale) modelled on Keepsake
  Atelier's rip settings, plus a lossless remux.
- Site page at https://ghostyshell.github.io/stripchat-record/, guides, AGENTS.md and DESIGN.md.

### Removed
- The Python recorder, `webrtc_client.html`, `run.sh` / `run.bat` and the
  Playwright/Chromium dependency.
