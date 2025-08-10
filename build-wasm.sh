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

# Build with wasm-pack (skip wasm-opt if it's incompatible)
WASM_PACK_NO_OPT_COMPATIBILITY_MODE=1 wasm-pack build --target web --out-dir wasm-pkg --no-opt

# Restore original Cargo.toml
mv Cargo.toml Cargo-wasm.toml
if [ -f "Cargo.toml.main" ]; then
    mv Cargo.toml.main Cargo.toml
fi

echo "Attempting to optimize WASM binary..."
if wasm-opt -Oz \
    -o wasm-pkg/rustation_wasm_bg_optimized.wasm \
    wasm-pkg/rustation_wasm_bg.wasm 2>/dev/null; then
    mv wasm-pkg/rustation_wasm_bg_optimized.wasm wasm-pkg/rustation_wasm_bg.wasm
    echo "WASM optimization successful"
else
    echo "WARNING: wasm-opt optimization failed, using unoptimized binary"
    echo "This is likely due to version incompatibility and doesn't affect functionality"
fi

echo "WASM build complete! Output in wasm-pkg/"
echo "To test in browser, run: python3 -m http.server 8000"
echo "Then open http://localhost:8000/index.html"