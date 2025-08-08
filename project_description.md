# Rustation-NG PlayStation Emulator Project Description

## Overview
Rustation-NG is a PlayStation 1 emulator written entirely in Rust, designed as a libretro core that can be used with frontends like RetroArch. The project aims to provide accurate PlayStation emulation with clean, well-documented code.

## Project Status
- **Development Phase**: Early development, functional but incomplete
- **Language**: Rust (2018 edition)
- **Architecture**: Libretro core library
- **License**: GPL-2.0+

## Key Features
- Pure Rust implementation for safety and performance
- Software renderer with 24-bit color support
- Multi-threaded architecture (separate rendering thread)
- Low-level CD controller emulation with firmware support
- GDB debugger interface for development
- Cross-platform support (Linux, macOS, Windows)

## Technical Architecture

### Core Components

#### 1. **CPU Emulation** (`src/psx/cpu.rs`)
   - **Processor**: MIPS R3000A implementation
   - **Registers**: 32 general-purpose registers, HI/LO for multiplication/division
   - **Pipeline**: Branch delay slot emulation
   - **Cache**: 4KB instruction cache (256 4-word cachelines)
   - **Concurrency**: DIV/MULT operations run concurrently with other instructions
   - **PC Management**: Tracks current_pc, pc, and next_pc for proper exception handling

#### 2. **System Controller** (`src/psx/cop0.rs`)
   - COP0 coprocessor for system control
   - Exception handling and interrupt management
   - Memory management unit control

#### 3. **GPU System** (`src/psx/gpu/`)
   - **Rasterizer**: Software-based with fixed-point arithmetic
   - **Command Processing**: GP0 command FIFO buffer
   - **Display Modes**: Support for NTSC/PAL video standards
   - **Features**:
     - 24-bit color rendering
     - Interlaced/progressive display
     - DMA integration for fast transfers
     - Drawing time budget simulation
     - Clipping area support
   - **Timing**: Accurate HSYNC/VSYNC timing simulation

#### 4. **Memory System** (`src/psx/xmem.rs`)
   - Main RAM management
   - BIOS ROM mapping
   - Scratch pad memory (1KB fast RAM)
   - Memory control registers

#### 5. **Audio (SPU)** (`src/psx/spu/`)
   - Sound Processing Unit emulation
   - FIFO buffer management
   - FIR filtering and reverb processing
   - Resampling support

#### 6. **CD-ROM Controller** (`src/psx/cd/`)
   - CDC (CD Controller) with microcontroller emulation
   - ISO9660 filesystem support
   - Multi-threaded cache implementation
   - Requires specific firmware (SCPH-5502)

#### 7. **GTE** (`src/psx/gte/`)
   - Geometry Transformation Engine
   - 3D mathematics coprocessor
   - Concurrent operation with main CPU

#### 8. **DMA Controller** (`src/psx/dma.rs`)
   - 7 DMA channels for fast data transfers
   - CPU stalling during DMA operations
   - Timing penalty simulation

#### 9. **Input/Memory Cards** (`src/psx/pad_memcard/`)
   - Gamepad device support
   - Memory card emulation for save data

#### 10. **Timers** (`src/psx/timers.rs`)
   - System timer implementation
   - Interrupt generation

#### 11. **IRQ System** (`src/psx/irq.rs`)
   - Interrupt state management
   - Priority handling

#### 12. **MDEC** (`src/psx/mdec/`)
   - Motion Decoder for FMV playback
   - FIFO-based data processing

### Synchronization Model
- **Cycle-accurate timing**: Uses `CycleCount` type for synchronization
- **Multi-component sync**: `sync::Synchronizer` coordinates all components
- **Frame synchronization**: GPU drives frame timing
- **DMA penalties**: Simulates CPU slowdown during DMA operations

## Dependencies
- libretro API for frontend integration
- cdimage library for disc image handling
- Standard Rust libraries (serde, log, etc.)

## Build Requirements
- Rust toolchain (via rustup)
- Platform-specific tools:
  - Linux: Standard development tools
  - macOS: Xcode Command Line Tools
  - Windows: Visual Studio build tools

## BIOS System

### BIOS Management
- **Size**: Fixed 512KB BIOS images required
- **Database**: Internal database of 24 known BIOS versions
- **Validation**: SHA-256 hash verification against known good dumps
- **Regions**: Support for Japan, North America, and Europe regions
- **Versions**: Tracks major/minor version numbers
- **Quality Check**: Identifies known bad dumps

### BIOS Features
- Region detection and matching with game disc
- Animation jump hook points for customization
- Debug UART patching support (for select BIOS versions)
- Automatic BIOS selection based on game region

## CD-ROM System

### CD Controller (CDC)
- **Low-level emulation**: MC68HC05 microcontroller emulation
- **Firmware requirement**: SCPH-5502 firmware (16,896 bytes)
- **SHA-256 verification**: `bf590fbf6055f428138510b26a2f2006b7eab54ead48c1ddb1a1a5d2699242db`
- **Region patching**: Firmware auto-patched for non-European regions
- **License string patching**: SCEE (Europe), SCEI (Japan), SCEA (North America)

### CD Features
- **Disc formats**: BIN/CUE support with ZIP archive support
- **ISO9660**: Full filesystem implementation
- **Multi-threading**: Separate cache thread for performance
- **DMA integration**: Direct memory access for data streaming
- **Speed control**: Variable loading speed support
- **Hot-swapping**: Disc eject/load during runtime

### CD Implementation Details
- State machine-based CDC emulation
- Position tracking for sled movement
- MDEC integration for FMV playback
- Interrupt generation for async operations

## Current Limitations
- Early development stage - expect glitches
- Only BIN/CUE format supported (no CHD yet)
- Requires specific CD controller firmware (SCPH-5502)
- Single firmware version support

## Sound Processing Unit (SPU)

### SPU Architecture
- **Voices**: 24 individual voice channels
- **RAM**: Internal 512KB SPU RAM (16-bit wide)
- **Audio Buffer**: 2048 sample buffer for output
- **Sample Rate**: 44.1kHz audio generation

### SPU Features
- **Volume Control**: Independent left/right main volume
- **Voice Control**: Start/stop/loop control per voice
- **Effects**:
  - LFSR noise generation
  - Frequency modulation between voices
  - Reverb processing with dedicated working memory
- **CD Audio**: Direct CD audio mixing with volume control
- **IRQ Support**: Memory-based interrupt triggering

### SPU Implementation Details
- ADPCM decoder FIFO for compressed audio
- FIR filtering support
- Reverb with downsampling/upsampling
- Capture buffers for recording
- Register-based control (320 16-bit registers)

## Documentation Progress

### Completed Tasks
- ✅ Project overview and description created
- ✅ Core architecture documentation
- ✅ BIOS handling and compatibility analysis
- ✅ CD controller firmware and implementation
- ✅ SPU audio system documentation

### Pending Analysis
- GTE 3D processing
- DMA controller operations
- GPU command processing and rasterization
- Input device handling
- Debugger interface
- Error handling patterns
- Build system details
- Testing strategies
- Performance optimization opportunities

## Next Steps
1. Deep dive into CPU architecture and instruction set
2. Document memory mapping and management
3. Analyze BIOS loading and validation process
4. Review CD controller firmware requirements
5. Profile performance bottlenecks
6. Identify areas for improvement

---
*Last Updated: 2025-08-08*