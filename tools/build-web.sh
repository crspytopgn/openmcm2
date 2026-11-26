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
# Size-tuned for the browser: the native release profile is optimised
# for speed (opt-level 3), but the wasm is downloaded on every first
# visit, so size wins there — opt-level "z" plus panic=abort (no
# unwinding tables; nothing in the tree uses catch_unwind).
cargo build --release --target wasm32-unknown-unknown -p mm2-viewer \
    --config 'profile.release.opt-level="z"' \
    --config 'profile.release.panic="abort"'
wasm-bindgen --target web --no-typescript --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/mm2-viewer.wasm
# Smaller and faster, when binaryen's wasm-opt is installed.
if command -v wasm-opt >/dev/null; then
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        web/pkg/mm2-viewer_bg.wasm -o web/pkg/mm2-viewer_bg.wasm
fi
# The page checks this to load a new build fresh and to tell players
# when one is out while they play. It is a hash of the built engine,
# not a timestamp, so rebuilding the same source does not bust every
# player's cached build for nothing.
version="$(git rev-parse --short HEAD 2>/dev/null || echo dev)-$(sha256sum web/pkg/mm2-viewer_bg.wasm | cut -c1-12)"
# The engine's size, and about what it takes to download compressed, for
# the loading screen's progress bar.
engine="$(wc -c < web/pkg/mm2-viewer_bg.wasm | tr -d ' ')"
engine_gz="$(gzip -c web/pkg/mm2-viewer_bg.wasm | wc -c | tr -d ' ')"
printf '{"version":"%s","engine":%s,"engine_gz":%s}\n' "$version" "$engine" "$engine_gz" > web/version.json
echo "built web/; serve it with: python3 -m http.server -d web 8000"
