# Features

## Record
- Room lookup before recording: live state, title and the quality variants the
  CDN is offering.
- Source variant by default; any listed variant can be chosen.
- Optional stop-after-N-seconds.
- Optional compress-on-finish with the chosen preset.
- Optional keep of the intermediate recording file.

## Recording engine
- MOUFLON v2 segment URIs are decoded in-app, and a rotated pkey is reported instead of recorded as garbage.
- Segments are downloaded whole, then appended; a failure never leaves a
  partial fragment in the file.
- Each part starts with the CDN's init segment, so parts play independently.
- Cancelling stops at the next segment boundary, closes the file and still
  runs the final mux/remux.

## Library
- Every `.mp4` in the output folder, newest first, with size and date.
- Reveal in the file manager or delete.

## Convert
- Remux only (lossless).
- H.264 CRF 23, H.265 CRF 26, H.265 CRF 30, and H.264 720p downscale.
- Progress parsed from `ffmpeg -progress`, cancellable.

## Settings
- Output folder, split size, segment retries.
- Proxy for the site API.
- ffmpeg/ffprobe paths (auto-detected, overridable).
- Health checks: ffmpeg, ffprobe, output folder, API reachability, encryption key pairs.
