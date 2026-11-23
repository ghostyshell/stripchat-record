# Build from source

## Requirements

- Rust 1.85 or newer
- Node 20 or newer
- Tauri 2 prerequisites: Xcode command line tools on macOS, MSVC and WebView2
  on Windows, `webkit2gtk-4.1` on Linux

## Build

```sh
npm install --prefix app
cargo build --release -p stripchat-recorder-cli
npx --prefix app tauri build
```

The desktop bundles land in `app/src-tauri/target/release/bundle/`.

## Develop the UI without a network

```sh
npm run --prefix app dev:mock
```

That opens `http://localhost:1420/?mock=1`, where a fixture bridge stands in
for every command, so the whole UI works with no backend and no recording.

## Tests

```sh
cargo test --workspace
npm run --prefix app build
```

## Release

```sh
scripts/bump-version.sh 0.5.1 "Fixed: ..."
scripts/publish-release.sh
```

The publish script builds the app, uploads the artifacts to Cloudflare R2,
rewrites the README table and `docs/downloads.json`, prunes older R2 versions,
commits, pushes and then checks the live feed.
