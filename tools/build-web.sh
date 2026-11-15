#!/bin/sh
# Builds the browser version into web/: serve that folder (any static web
# server) and open it in a browser with WebGPU.
#
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version <the wasm-bindgen in Cargo.lock>
#   sh tools/build-web.sh
#   python3 -m http.server -d web 8000     # then open http://localhost:8000
set -e
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown -p mm2-viewer
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/mm2-viewer.wasm
# Smaller and faster, when binaryen's wasm-opt is installed.
if command -v wasm-opt >/dev/null; then
    wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        web/pkg/mm2-viewer_bg.wasm -o web/pkg/mm2-viewer_bg.wasm
fi
# The page checks this to load a new build fresh and to tell players
# when one is out while they play.
version="$(git rev-parse --short HEAD 2>/dev/null || echo dev)-$(date -u +%Y%m%d%H%M%S)"
printf '{"version":"%s"}\n' "$version" > web/version.json
echo "built web/; serve it with: python3 -m http.server -d web 8000"
