#!/bin/sh
# Builds the file readers behind the site's Tools page into
# site/tools/pkg. The page itself is plain HTML and JavaScript in
# site/tools; serve the site folder (with the browser version in
# site/play for its CD and zip readers) to try it:
#
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version <the wasm-bindgen in Cargo.lock>
#   sh tools/build-web-tools.sh
#   mkdir -p site/play && cp web/*.js site/play/
#   python3 -m http.server -d site 8000   # then open http://localhost:8000/tools/
set -e
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown -p mm2-webtools
wasm-bindgen --target web --no-typescript --out-dir site/tools/pkg \
    target/wasm32-unknown-unknown/release/mm2_webtools.wasm
if command -v wasm-opt >/dev/null; then
    wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        site/tools/pkg/mm2_webtools_bg.wasm -o site/tools/pkg/mm2_webtools_bg.wasm
fi
# The AI lap's riders, loaded only when the Course Builder tab asks.
cargo build --release --target wasm32-unknown-unknown -p mm2-preview
wasm-bindgen --target web --no-typescript --out-dir site/tools/sim \
    target/wasm32-unknown-unknown/release/mm2_preview.wasm
if command -v wasm-opt >/dev/null; then
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        site/tools/sim/mm2_preview_bg.wasm -o site/tools/sim/mm2_preview_bg.wasm
fi
# The Free Pack rider's and bike's own motions, for the viewer.
cp crates/mm2-viewer/src/free_motions.bin site/tools/pkg/
echo "built site/tools/pkg"
