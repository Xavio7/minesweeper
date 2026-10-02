#!/usr/bin/env bash
# Builds the deployable site into dist/:
#   dist/v1/  the original 2017 Vanilla JS game, copied as is
#   dist/v2/  the Rust + WebAssembly rewrite
# Used by Vercel (see vercel.json) and runnable locally. Installs the Rust
# toolchain and wasm-bindgen when they are missing, so it works on a clean
# build machine.
set -euo pipefail

# Keep in sync with the wasm-bindgen pin in v2/Cargo.toml.
WASM_BINDGEN_VERSION="0.2.129"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/dist"
TOOLS="$ROOT/.tools"

log() { printf '\n==> %s\n' "$*"; }

if ! command -v cargo >/dev/null 2>&1; then
  log "Installing Rust (minimal profile)"
  mkdir -p "$TOOLS"
  curl --proto '=https' --tlsv1.2 -sSfL -o "$TOOLS/rustup-init" \
    "https://static.rust-lang.org/rustup/dist/$(uname -m)-unknown-linux-gnu/rustup-init"
  chmod +x "$TOOLS/rustup-init"
  "$TOOLS/rustup-init" -y --no-modify-path --profile minimal --default-toolchain stable
fi
# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown >/dev/null
fi

WASM_BINDGEN="$(command -v wasm-bindgen || true)"
if [ -z "$WASM_BINDGEN" ] || ! "$WASM_BINDGEN" --version | grep -q " $WASM_BINDGEN_VERSION\$"; then
  WASM_BINDGEN="$TOOLS/wasm-bindgen-$WASM_BINDGEN_VERSION/wasm-bindgen"
  if [ ! -x "$WASM_BINDGEN" ]; then
    log "Downloading wasm-bindgen $WASM_BINDGEN_VERSION"
    mkdir -p "$TOOLS"
    name="wasm-bindgen-$WASM_BINDGEN_VERSION-x86_64-unknown-linux-musl"
    curl -sSfL "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$WASM_BINDGEN_VERSION/$name.tar.gz" \
      | tar -xz -C "$TOOLS"
    mv "$TOOLS/$name" "$TOOLS/wasm-bindgen-$WASM_BINDGEN_VERSION"
  fi
fi

log "Compiling v2 to WebAssembly"
cargo build --manifest-path "$ROOT/v2/Cargo.toml" --release --target wasm32-unknown-unknown --locked

log "Assembling $OUT"
rm -rf "$OUT"
mkdir -p "$OUT"
cp -R "$ROOT/v1" "$OUT/v1"
cp -R "$ROOT/v2/web" "$OUT/v2"
"$WASM_BINDGEN" --target web --no-typescript --out-dir "$OUT/v2/pkg" \
  "$ROOT/v2/target/wasm32-unknown-unknown/release/minesweeper_v2.wasm"

du -sh "$OUT/v2/pkg"/*.wasm
log "Done"
