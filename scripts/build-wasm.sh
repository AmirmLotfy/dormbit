#!/bin/sh
# Builds the demo engine (crates/dormbit-wasm) for the browser and writes the wasm-bindgen
# glue to apps/demo/src/wasm/pkg, plus the fixture the demo loads (judge-demo Task 38).
set -eu
# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
cd "$(dirname "$0")/.."
out=apps/demo/src/wasm/pkg
pub=apps/demo/public/demo-data
cargo build -p dormbit-wasm --target wasm32-unknown-unknown --release --locked
mkdir -p "$out" "$pub"
wasm-bindgen --target web --out-dir "$out" \
  target/wasm32-unknown-unknown/release/dormbit_wasm.wasm
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    "$out/dormbit_wasm_bg.wasm" -o "$out/dormbit_wasm_bg.wasm"
fi
cp fixtures/dockline/dockline-invoices.tar "$pub/dockline-invoices.tar"
cp fixtures/dockline/dockline-invoices.sidecar.json "$pub/dockline-invoices.sidecar.json"
ls -l "$out" "$pub"
