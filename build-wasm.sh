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

# Set up WASI SDK environment if available
if [ -d "wasi-sdk-20.0" ]; then
    export WASI_SDK_PATH=$PWD/wasi-sdk-20.0
    export CC_wasm32_unknown_unknown=$WASI_SDK_PATH/bin/clang
    export AR_wasm32_unknown_unknown=$WASI_SDK_PATH/bin/llvm-ar
    export CFLAGS_wasm32_unknown_unknown="--sysroot=$WASI_SDK_PATH/share/wasi-sysroot"
    echo "Using WASI SDK at $WASI_SDK_PATH"
fi

# Temporarily swap Cargo.toml files for wasm-pack
if [ -f "Cargo.toml" ]; then
    mv Cargo.toml Cargo.toml.main
fi
mv Cargo-wasm.toml Cargo.toml

# Build with wasm-pack
wasm-pack build --target web --out-dir wasm-pkg

# Restore original Cargo.toml
mv Cargo.toml Cargo-wasm.toml
if [ -f "Cargo.toml.main" ]; then
    mv Cargo.toml.main Cargo.toml
fi

echo "Optimizing WASM binary..."
wasm-opt -Oz \
    -o wasm-pkg/rustation_wasm_bg_optimized.wasm \
    wasm-pkg/rustation_wasm_bg.wasm

mv wasm-pkg/rustation_wasm_bg_optimized.wasm wasm-pkg/rustation_wasm_bg.wasm

echo "WASM build complete! Output in wasm-pkg/"
echo "To test in browser, run: python3 -m http.server 8000"
echo "Then open http://localhost:8000/index.html"