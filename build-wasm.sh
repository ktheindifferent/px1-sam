#!/bin/bash

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

trap 'log_error "Build failed at line $LINENO"' ERR

log_info "Building Rustation for WebAssembly..."

if ! command -v wasm-pack &> /dev/null; then
    log_warning "wasm-pack not found. Installing..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

if ! command -v wasm-opt &> /dev/null; then
    log_error "wasm-opt not found. Please install binaryen tools:"
    echo "  - On macOS: brew install binaryen"
    echo "  - On Ubuntu/Debian: apt-get install binaryen"
    echo "  - On Arch: pacman -S binaryen"
    exit 1
fi

log_info "Building WASM module..."

# Set up WASI SDK environment if available
if [ -d "wasi-sdk-20.0" ]; then
    export WASI_SDK_PATH=$PWD/wasi-sdk-20.0
    export CC_wasm32_unknown_unknown=$WASI_SDK_PATH/bin/clang
    export AR_wasm32_unknown_unknown=$WASI_SDK_PATH/bin/llvm-ar
    export CFLAGS_wasm32_unknown_unknown="--sysroot=$WASI_SDK_PATH/share/wasi-sysroot"
    log_info "Using WASI SDK at $WASI_SDK_PATH"
fi

# Ensure we have the WASM Cargo.toml
if [ -f "Cargo.toml.main" ]; then
    cp Cargo.toml.main Cargo.toml
fi

# Build with wasm-pack (skip wasm-opt if it's incompatible)
WASM_PACK_NO_OPT_COMPATIBILITY_MODE=1 wasm-pack build --target web --out-dir wasm-pkg --no-opt

log_info "Attempting to optimize WASM binary..."
if wasm-opt -Oz \
    -o wasm-pkg/rustation_wasm_bg_optimized.wasm \
    wasm-pkg/rustation_wasm_bg.wasm 2>/dev/null; then
    mv wasm-pkg/rustation_wasm_bg_optimized.wasm wasm-pkg/rustation_wasm_bg.wasm
    log_info "WASM optimization successful"
else
    log_warning "wasm-opt optimization failed, using unoptimized binary"
    log_warning "This is likely due to version incompatibility and doesn't affect functionality"
fi

log_info "WASM build complete! Output in wasm-pkg/"
log_info "To test in browser, run: python3 -m http.server 8000"
log_info "Then open http://localhost:8000/index.html"