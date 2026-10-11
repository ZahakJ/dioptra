#!/usr/bin/env bash
# Build the engine for the browser: wasm32 + wasm-bindgen into apps/dioptra/src/engine/wasm/pkg.
# Requires `cargo install wasm-bindgen-cli --version 0.2.129` (pinned to the crate version).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
PROFILE="${1:-release}"
if [[ "$PROFILE" == "release" ]]; then
  cargo build -p dioptra-wasm --target wasm32-unknown-unknown --release
  WASM=target/wasm32-unknown-unknown/release/dioptra_wasm.wasm
else
  cargo build -p dioptra-wasm --target wasm32-unknown-unknown
  WASM=target/wasm32-unknown-unknown/debug/dioptra_wasm.wasm
fi
OUT=apps/dioptra/src/engine/wasm/pkg
mkdir -p "$OUT"
wasm-bindgen "$WASM" --out-dir "$OUT" --target web --typescript
ls -la "$OUT"
