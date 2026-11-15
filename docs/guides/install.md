# Install

## Download

Grab the build for your platform from the downloads table on
[Stripchat Recorder](https://ghostyshell.github.io/stripchat-record/). macOS (arm64 and x64), Windows (installer and portable) and
Linux (AppImage and deb) are published per release, alongside a headless CLI.

The builds are not notarized. On macOS, right-click the app and choose Open the
first time, or clear the quarantine flag:

```sh
xattr -dr com.apple.quarantine "/Applications/Stripchat Recorder.app"
```

## ffmpeg

The app uses ffmpeg for the final remux, for muxing and for compression. It is
found on `PATH`; if it lives somewhere unusual, set the path in Settings. Health
shows whether it was found.

- macOS: `brew install ffmpeg`
- Windows: `winget install Gyan.FFmpeg`
- Linux: `sudo apt install ffmpeg`

Without ffmpeg the recording still works; the file is simply left with its
`.recording.mp4` name, which plays fine.

## First run

1. Open the app.
2. Check the status bar: `ffmpeg ready` is what you want.
3. Open Settings once to confirm the recordings folder.
