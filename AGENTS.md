# Stripchat Recorder: project overview for agents

Stripchat Recorder records Stripchat live rooms from the CDN as they air. It shares its code
stack and design family with Keepsake Atelier, GODsend 360 and GODsend PSP
(Rust workspace + Tauri 2 + React).

## Components

- **`core/` (`stripchat-core`)**: framework-agnostic engine. `site` (room
  lookup, cookies, playlist requests), `hls` (playlist parsing, variant
  choice), `recorder` (the live loop and the append-only part writer), `media`
  (ffmpeg remux, mux and CRF presets), `jobs` (queue with progress and cancel),
  `library`, `doctor`, `config`.
- **`cli/` (`stripchat-recorder-cli`)**: headless CLI over core (clap 4).
- **`app/`**: Tauri 2 desktop app. `app/src-tauri/src/lib.rs` holds the
  `#[tauri::command]`s; `app/src` is React 18 + TypeScript with one
  hand-rolled `styles.css` (see DESIGN.md) and a mock bridge in `mock.ts`
  behind `?mock=1`.

## Contracts and invariants

- **The recording loop is the product.** A segment is downloaded into memory
  and only appended once it is whole; every part starts with the init segment.
  A hard kill must always leave a decodable file. Do not "optimise" this into
  streaming straight into the file handle.
- `.recording.mp4` is the in-progress file. Finalizing remuxes it into the
  real name; on error the recording is kept, never deleted.
- `core/` stays free of UI and process-framework dependencies.
- Media fetches go through the SSRF guard (`is_allowed_media_host`); never
  fetch an arbitrary URL.
- **Every change ships complete**: bump the version, add a CHANGELOG entry,
  and update the README/site when user-visible behaviour changes.

## Building and testing

```sh
cargo test --workspace                  # unit tests (hls, part writer, jobs)
cargo check --workspace                 # zero warnings expected
npm run --prefix app build              # tsc --noEmit + vite build
npm run --prefix app dev:mock           # browser UI without a network
```

## Conventions

- Public-facing copy: no em dashes, no en dashes, ASCII hyphen only.
- **Version bump + changelog, every change.** No `[Unreleased]` section: newest
  `## [x.y.z] - YYYY-MM-DD` header at the top. `scripts/check-changelog.sh`
  enforces it.
- Frontend types in `app/src/types.ts` mirror the Rust serializers, and the
  mock fixtures mirror real shapes.
- Errors: core returns `stripchat-recorder_core::Result<T>`; the Tauri error type
  serializes as the Display string.
