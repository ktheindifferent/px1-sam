#!/bin/bash

echo "🎮 Rustation WebAssembly Test Script"
echo "===================================="
echo ""

if [ ! -d "wasm-pkg" ]; then
    echo "⚠️  WASM package not found. Building..."
    ./build-wasm.sh
    
    if [ $? -ne 0 ]; then
        echo "❌ Build failed. Please check build-wasm.sh output."
        exit 1
    fi
fi

echo "✅ WASM package found in wasm-pkg/"
echo ""
echo "📦 Files needed for testing:"
echo "  1. PlayStation BIOS file (e.g., SCPH1001.BIN)"
echo "  2. PlayStation game ISO/BIN/CUE file"
echo ""
echo "🌐 Starting web server on http://localhost:8000"
echo "   Press Ctrl+C to stop the server"
echo ""
echo "👉 Open http://localhost:8000 in your browser to test!"
echo ""

python3 -m http.server 8000