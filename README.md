# Minesweeper

Two versions of Minesweeper, one domain:

| Path   | Version | Built with |
|--------|---------|------------|
| `/v2/` | Responsive rewrite for phones, tablets and desktops ([`v2/`](v2/)) | Rust → WebAssembly |
| `/v1/` | The original 2017 learning project, served as is ([`v1/`](v1/)) | Vanilla JavaScript |

`/` redirects to `/v2/`.

## v2 highlights

- Game engine and UI written in Rust and compiled to WebAssembly (~176 KB).
- Safe first click, chording, Easy / Medium / Hard / Custom boards.
- Works on any device: tap to dig, long-press or a Dig/Flag switch to flag, right click and keyboard on desktop.
- Light and dark themes, best times and streaks, a help dialog, and an installable web app.

Design and implementation details are in [`featureDocument/v2/featureDocumentV2.md`](featureDocument/v2/featureDocumentV2.md) (also as `.odt`). The original v1 design documents are in [`featureDocument/`](featureDocument/).

## Project layout

```
v1/                 original game (unchanged)
v2/src/engine.rs    game rules, pure Rust, unit-tested
v2/src/ui.rs        DOM rendering and input handling (web-sys)
v2/src/storage.rs   localStorage, statistics
v2/web/             index.html, style.css, icons, manifest
scripts/build.sh    builds dist/ (v1 + v2)
vercel.json         Vercel build and routing
```

## Running locally

Requires Rust (https://rustup.rs). The build script installs the WebAssembly target and `wasm-bindgen` automatically.

```sh
bash scripts/build.sh           # outputs dist/
cd dist && python3 -m http.server 8000
# http://localhost:8000/v2/  and  http://localhost:8000/v1/base.html
```

Run the engine's unit tests:

```sh
cd v2 && cargo test
```

## Deploying on Vercel (free)

1. Sign in at [vercel.com](https://vercel.com) with GitHub.
2. Click **Add New… → Project** and import this repository.
3. Leave **Framework Preset** as **Other**. `vercel.json` already sets the build command (`bash scripts/build.sh`) and the output directory (`dist`). Click **Deploy**.

Every push to `master` deploys to production; every other branch gets its own preview URL. The first build takes a couple of minutes because it installs Rust.
