# Rustation-NG Development TODO List

## 🔴 Critical Priority - Core Functionality Fixes

### Unimplemented Functions (89 total)
- [ ] **SPU Audio System** (`src/psx/spu/mod.rs`)
  - [ ] Fix byte store operations (line 907)
  - [ ] Implement volume sweep functionality (line 1064, 1420)
  - [ ] Complete ADSR envelope edge cases
  - [ ] Fix voice key on/off timing

- [ ] **Timer System** (`src/psx/timers.rs`)
  - [ ] Implement timer register reads (line 622, 642)
  - [ ] Fix timer synchronization modes
  - [ ] Complete pixel/hblank clock sources
  - [ ] Add timer interrupt edge cases

- [ ] **Gamepad/Memory Card** (`src/psx/pad_memcard/mod.rs`)
  - [ ] Implement TX without selection (line 131)
  - [ ] Fix RX enable edge cases (line 141)
  - [ ] Complete baud rate handling (line 385)
  - [ ] Add multitap protocol support

- [ ] **DMA Controller** (`src/psx/dma.rs`)
  - [ ] Implement OTC DMA mode (line 108)
  - [ ] Fix linked list termination (line 139)
  - [ ] Complete burst mode timing (line 282)
  - [ ] Add DMA priority handling

- [ ] **CD Controller** (`src/psx/cd/cdc/uc/mod.rs`)
  - [ ] Complete CDC firmware opcodes (line 628, 669, 683)
  - [ ] Fix seek timing accuracy
  - [ ] Implement audio track pregap
  - [ ] Add GetlocL/P commands

### Error Handling Refactor (47 panics)
- [ ] Replace panic! with Result in memory access
- [ ] Add validation for register writes
- [ ] Implement graceful BIOS fallback
- [ ] Add error recovery for CD reads
- [ ] Fix unwrap() calls in critical paths

## 🟠 High Priority - Performance & Graphics

### Hardware GPU Acceleration
- [ ] **OpenGL Backend**
  - [ ] Create renderer abstraction interface
  - [ ] Implement OpenGL 3.3 context creation
  - [ ] Port software rasterizer to shaders
  - [ ] Add framebuffer management
  - [ ] Implement texture cache

- [ ] **Resolution Scaling**
  - [ ] Add internal resolution options (2x-16x)
  - [ ] Implement proper viewport scaling
  - [ ] Fix 2D element scaling
  - [ ] Add aspect ratio correction

- [ ] **Performance Optimizations**
  - [ ] Add SIMD for MDEC YUV conversion
  - [ ] Optimize fixed-point math in rasterizer
  - [ ] Implement instruction cache simulation
  - [ ] Add fast-path for common operations

### Performance Monitoring
- [ ] Add FPS counter overlay
- [ ] Implement CPU usage tracking
- [ ] Create GPU utilization metrics
- [ ] Add frame time graph
- [ ] Implement bottleneck detection

## 🟡 Medium Priority - User Experience

### Video Recording
- [ ] **Core Recording**
  - [ ] Integrate FFmpeg library
  - [ ] Implement frame buffer capture
  - [ ] Add audio stream mixing
  - [ ] Create encoding pipeline

- [ ] **Recording Features**
  - [ ] Add H.264/H.265 encoding
  - [ ] Implement WebM output
  - [ ] Add quality presets
  - [ ] Create GIF recording mode
  - [ ] Add screenshot capture

### Save State Enhancements
- [ ] Add zstd compression
- [ ] Implement state thumbnails
- [ ] Add incremental saves
- [ ] Create state metadata
- [ ] Add state validation

### Enhanced Debugging
- [ ] **GUI Debugger**
  - [ ] Create disassembly view
  - [ ] Add memory browser
  - [ ] Implement register display
  - [ ] Add breakpoint manager
  - [ ] Create watch window

- [ ] **Profiling Tools**
  - [ ] Add instruction histogram
  - [ ] Create hotspot analysis
  - [ ] Implement cycle counting
  - [ ] Add DMA profiling

## 🟢 Lower Priority - Modern Features

### RetroAchievements
- [ ] Integrate rcheevos library
- [ ] Add achievement popup system
- [ ] Implement progress tracking
- [ ] Add leaderboard support
- [ ] Create hardcore mode

### Shader System
- [ ] **Shader Pipeline**
  - [ ] Create shader manager
  - [ ] Add GLSL compiler
  - [ ] Implement multi-pass rendering
  - [ ] Add uniform management

- [ ] **Shader Presets**
  - [ ] CRT simulation (scanlines, bloom)
  - [ ] Anti-aliasing (FXAA, SMAA)
  - [ ] xBR/SABR texture filtering
  - [ ] Color correction filters

### Network Play
- [ ] Design netplay protocol
- [ ] Implement state synchronization
- [ ] Add rollback networking
- [ ] Create lobby system
- [ ] Add spectator mode

## 📊 Testing & Quality

### Test Coverage Expansion (Target: 80%)
- [ ] **Unit Tests**
  - [ ] SPU audio processing (0% → 80%)
  - [ ] DMA controller (20% → 80%)
  - [ ] Timer system (25% → 80%)
  - [ ] Memory cards (40% → 80%)

- [ ] **Integration Tests**
  - [ ] Full game boot sequences
  - [ ] Save state round-trips
  - [ ] Controller input chains
  - [ ] CD loading scenarios

- [ ] **Performance Tests**
  - [ ] Benchmark suite creation
  - [ ] Regression detection
  - [ ] Memory leak testing
  - [ ] Thread safety validation

### Documentation
- [ ] **Code Documentation**
  - [ ] Document all public APIs
  - [ ] Add inline code examples
  - [ ] Create architecture diagrams
  - [ ] Write contributor guide

- [ ] **User Documentation**
  - [ ] Quick start guide
  - [ ] Configuration manual
  - [ ] Troubleshooting guide
  - [ ] Game compatibility list

## 🐛 Known Bugs

### High Priority Bugs
- [ ] Final Fantasy 7 battle transitions crash
- [ ] Tekken 3 character select graphical glitches
- [ ] Gran Turismo replay desync
- [ ] Crash Bandicoot audio crackling
- [ ] Metal Gear Solid codec freeze

### Medium Priority Bugs
- [ ] Ridge Racer texture corruption
- [ ] Spyro camera jitter
- [ ] Silent Hill fog rendering
- [ ] Vagrant Story text alignment
- [ ] Chrono Cross FMV stuttering

## 🚀 Future Enhancements

### Research Projects
- [ ] Machine learning upscaling
- [ ] Vulkan renderer backend
- [ ] WebAssembly port
- [ ] Mobile platform support
- [ ] VR mode experimentation

### Community Features
- [ ] Discord Rich Presence
- [ ] Twitch integration
- [ ] Built-in game database
- [ ] Automatic updates
- [ ] Cloud save sync

## 📅 Sprint Planning

### Current Sprint (Week 1-2)
1. Fix critical SPU unimplemented functions
2. Complete timer system implementation
3. Begin error handling refactor
4. Start OpenGL backend design

### Next Sprint (Week 3-4)
1. Complete OpenGL implementation
2. Add performance monitoring
3. Fix high-priority bugs
4. Expand test coverage to 50%

### Future Sprints
- Week 5-6: Video recording, save states
- Week 7-8: RetroAchievements, shaders
- Week 9-10: Bug fixes, optimization
- Week 11-12: Documentation, release prep

## 📝 Notes

### Dependencies to Update
- cdimage: Check for newer version
- serde: Update to 1.0.latest
- log: Consider tracing migration

### Technical Debt Items
- Refactor GPU command processing
- Modernize error types
- Update to Rust 2021 idioms
- Remove deprecated APIs

### Performance Bottlenecks
1. Software rasterizer inner loop
2. CPU interpreter dispatch
3. Memory access patterns
4. Thread synchronization overhead

---
*Last Updated: 2025-08-09*
*Total Tasks: 127*
*Completed: 0*
*In Progress: 0*
*Blocked: 0*