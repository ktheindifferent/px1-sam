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
- Build system updated and all tests passing

### Architecture Overview

#### Core Emulation (`src/psx/`)
- **CPU Module** (`cpu.rs`, `cpu_instructions.rs`): MIPS R3000A processor emulation
- **GPU Module** (`gpu/`): Graphics processing and rendering
- **SPU Module** (`spu/`): Sound processing unit
- **GTE Module** (`gte/`): Geometry Transform Engine for 3D calculations
- **DMA Module** (`dma.rs`): Direct Memory Access controller
- **Memory Management** (`memory_map.rs`, `memory_control.rs`): RAM, ROM, and I/O mapping

#### WASM Implementation Variants
- **wasm.rs**: Full-featured WASM implementation (has CD dependencies)
- **wasm_minimal.rs**: Minimal working implementation without CD support
- **wasm_unified.rs**: Unified interface with multiple format support
- **wasm_enhanced.rs**: Enhanced features including save states
- **wasm_bridge.rs**: Bridge layer for JavaScript interop

#### Error Handling
- **error.rs**: Original error types for libretro build
- **error_stub.rs**: WASM-specific error types with enhanced information
- **error_traits.rs**: Comprehensive error trait system for unified handling

#### Testing Infrastructure
- **tests/**: Comprehensive test suite
  - error_handling_tests.rs
  - save_state_tests.rs
  - performance_monitor_tests.rs
  - input_validation_tests.rs
  - memory_safety_tests.rs
  - integration_tests.rs

### Build Configuration
- **Cargo.toml**: Main build configuration for libretro
- **Cargo-wasm.toml**: WebAssembly-specific configuration
- **build-wasm.sh**: Automated WASM build script
- **test-wasm.sh**: Local testing script

### Recent Improvements
- Fixed compilation errors with proper error type usage
- Added missing flexbuffers dependency
- Resolved all test compilation issues
- Cleaned up unused variable warnings
