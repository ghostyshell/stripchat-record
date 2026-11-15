# Changelog

All notable changes to **stripchat-record** are listed here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.5.1] - 2026-11-15

### Fixed
- A failed re-encode no longer deletes an output file that already existed, and the
  encoder refuses to write over its own source.
- An unreachable site API no longer ends a recording as "stream ended"; only a
  definite offline answer does, and a cancel during a retry stays a cancel.
- A rotated pkey now re-fetches the master and re-parses the variants, so the
  recovery path actually reconnects instead of retrying the dead URL.
- A CDN media-sequence restart resumes at the new window instead of filtering
  every segment out and recording nothing for the rest of the session.
- Two recordings started in the same second no longer share one file.
- Media and playlist fetches reject non-CDN hosts, URL userinfo tricks and
  redirects; playlists are checked as well as segments.
- Deleting from the Library only touches finished videos inside the recordings
  folder, never a file a running job is writing.
- Settings are written atomically, so a kill mid-save cannot silently reset them.
- A rotation that lands on the last segment no longer leaves an empty
  `.recording.mp4` behind.
- The CLI exits on a second Ctrl+C instead of only setting the stop flag again.

### Changed
- One part failing to finalize no longer aborts the rest, and a recording that
  ended early says so in the job message instead of reporting plain success.

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
