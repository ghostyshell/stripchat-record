# Changelog

All notable changes to **stripchat-record** are listed here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.4.0] - 2025-06-27

### Added
- Discord and Ko-fi support badges in the README.
- `--split-size` flag (default `1G`): long recordings rotate into `-partNNN.webm`
  files instead of one fragile huge file. Each part is remuxed to `.mp4`
  individually. Use `--split-size 0` to record a single file.
- Viewport matching: the headless Chromium window is sized to the stream's
  native aspect ratio.
- Graceful Ctrl+C shutdown: the current split part is saved before exit.
- Live recording duration and size in the terminal while recording.
- `CHANGELOG.md`, `CLAUDE.md`, `.cursor/rules/sync-docs.mdc`, `.githooks/`, and
  `scripts/install-hooks.sh` for cross-tool agent workflows.

### Fixed
- Stream-failure saves and default to headless recording.
- Ctrl+C save during split recording.

## [0.1.0] - 2025-06-19

### Added
- Initial release: Stripchat WebRTC stream recorder.
  - Fetches `streamName` from the broadcasts API.
  - Connects to the Doppio WebRTC edge and upgrades to `source` quality.
  - Records the raw `MediaStream` with `MediaRecorder`.
  - Remuxes to `.mp4` with ffmpeg.

[Unreleased]: https://github.com/ghostyshell/stripchat-record/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/ghostyshell/stripchat-record/releases/tag/v0.4.0
[0.1.0]: https://github.com/ghostyshell/stripchat-record/releases/tag/v0.1.0
