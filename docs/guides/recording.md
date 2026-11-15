# Recording a room

1. Open **Record**.
2. Type the model username exactly as it appears in the stripchat.com URL.
3. Press **Check**. The card shows whether the room is live and which quality
   variants the CDN is offering.
4. Pick a variant (the highest is selected by default) or leave it on `source`.
5. Optionally set **Stop after** in seconds. Leave it empty to record until the
   room goes offline.
6. Press **Start recording**.

## What ends a recording

- The room goes offline: the playlist ends and the file is closed.
- The stop-after timer elapses.
- You press **Stop and save** (or Ctrl+C in the CLI): the current segment is
  finished, the file is closed, and the final remux still runs.

## What is on disk while it records

The file is named `<model>-<timestamp>.recording.mp4`. The `.recording` marker
means "this may still be growing". It is a valid fragmented MP4 at every
moment, so you can open it while it records.

A single video file: Stripchat Recorder writes video and audio in the same fragment.

## Parts

With splitting on (the default, 1 GiB) long recordings rotate into
`-part001`, `-part002`, and so on. Each part starts with its own init segment,
so every part plays on its own.
