# Library and conversion

## Library

**Library** lists every MP4 in the output folder, newest first, with its size
and when it was recorded. **Show** reveals it in the file manager; the bin
deletes it after a confirmation.

An unfinished recording still carries `.recording` in its name. It is playable,
but if the app was killed mid-recording you may want to finalize it: in the
CLI, `convert <file> --preset copy` remuxes it into a clean MP4 with no quality
loss.

## Convert

**Convert** turns any video into a final file:

| Preset | What it does |
| --- | --- |
| Keep original | Stream copy. Instant, identical picture. |
| H.264 balanced | CRF 23, plays everywhere, about half the size. |
| H.265 balanced | CRF 26, smaller than H.264, slower to encode. |
| H.265 compact | CRF 30, smallest files, some fine detail lost. |
| H.264 720p | Scales to 720p at CRF 24, good for sharing. |

Conversion runs on the same queue as recording, reports progress and can be
cancelled. The source file is never modified.
