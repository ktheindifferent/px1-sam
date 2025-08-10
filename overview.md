# Rustation PSX Emulator - Project Overview

## Project Structure

### Core Components
- **PSX Emulation Core** (`src/psx/`)
  - CPU emulation (MIPS R3000A)
  - GPU rendering
  - SPU audio processing
  - Memory management
  - Controller input handling

### Build Targets
1. **LibRetro Core** - For use with RetroArch and other frontends
2. **WebAssembly Module** - Browser-based emulation

### WASM Implementation
- **Main File:** `src/wasm.rs`
- **Minimal Build:** `src/wasm_minimal.rs` (fallback for dependency issues)
- **Bridge:** `src/wasm_bridge.rs` (interface abstraction)
- **Web Interface:** `index.html` (full-featured browser UI)

### Build System
- **Native Build:** Standard Cargo build for libretro
- **WASM Build:** 
  - Uses `Cargo-wasm.toml` for WebAssembly-specific configuration
  - `build-wasm.sh` script for automated building
  - `test-wasm.sh` for local testing

### Key Features
- Full PlayStation 1 emulation
- Save state support
- WebGL rendering in browser
- Keyboard and gamepad input
- Audio emulation via Web Audio API
- Performance monitoring

### Dependencies
- `wasm-bindgen` - JavaScript/WASM interop
- `web-sys` - Web API bindings
- `js-sys` - JavaScript bindings
- `binaryen` - WASM optimization tools

### Testing
Run `./test-wasm.sh` to build and test the WebAssembly version locally.
Access at `http://localhost:8000` after starting the test server.

### Current Status
- WASM build functional with all core features exposed
- Input handling, save states, and rendering working properly
- Ready for testing with BIOS and game files
