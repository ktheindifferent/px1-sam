#!/bin/bash

set -e

echo "Building Rustation for WebAssembly..."

if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack not found. Installing..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

if ! command -v wasm-opt &> /dev/null; then
    echo "wasm-opt not found. Please install binaryen tools:"
    echo "  - On macOS: brew install binaryen"
    echo "  - On Ubuntu/Debian: apt-get install binaryen"
    echo "  - On Arch: pacman -S binaryen"
    exit 1
fi

echo "Building WASM module..."
wasm-pack build --target web --out-dir wasm-pkg --manifest-path Cargo-wasm.toml

echo "Optimizing WASM binary..."
wasm-opt -Oz \
    -o wasm-pkg/rustation_wasm_bg_optimized.wasm \
    wasm-pkg/rustation_wasm_bg.wasm

mv wasm-pkg/rustation_wasm_bg_optimized.wasm wasm-pkg/rustation_wasm_bg.wasm

echo "WASM build complete! Output in wasm-pkg/"
echo "To test in browser, run: python3 -m http.server 8000"
echo "Then open http://localhost:8000/index.html"