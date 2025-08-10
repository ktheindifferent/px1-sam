# Rustation-NG Migration Guide

## Version 2.0 Migration - Backward Compatibility Assurance

This guide ensures smooth migration from Rustation-NG v1.x to v2.0 while maintaining complete backward compatibility.

## Table of Contents
1. [API Compatibility](#api-compatibility)
2. [Save State Migration](#save-state-migration)
3. [Configuration Migration](#configuration-migration)
4. [Feature Flags](#feature-flags)
5. [Testing Migration](#testing-migration)
6. [Rollback Procedures](#rollback-procedures)

## API Compatibility

### LibRetro API Preservation

All existing libretro API functions remain unchanged. New features are added through optional extensions:

```rust
// Existing API - UNCHANGED
pub extern "C" fn retro_run() {
    // Original implementation preserved
    context.run_frame();
}

// New optional API extensions
pub extern "C" fn retro_run_extended() {
    // Enhanced implementation with new features
    if context.features_enabled() {
        context.run_frame_with_enhancements();
    } else {
        context.run_frame(); // Fallback to original
    }
}
```

### Controller Input Compatibility

Existing controller mappings are preserved:

```rust
// Legacy controller support
pub fn handle_input_legacy(port: u32, device: u32, index: u32, id: u32) -> i16 {
    // Original input handling preserved
    match device {
        RETRO_DEVICE_JOYPAD => handle_joypad(port, id),
        RETRO_DEVICE_ANALOG => handle_analog(port, index, id),
        _ => 0,
    }
}

// New controller support (opt-in)
pub fn handle_input_extended(port: u32, device: u32, index: u32, id: u32) -> i16 {
    if use_legacy_input() {
        return handle_input_legacy(port, device, index, id);
    }
    
    // Enhanced input with new device support
    match device {
        RETRO_DEVICE_JOYPAD => handle_joypad_enhanced(port, id),
        RETRO_DEVICE_LIGHTGUN => handle_lightgun(port, id),
        RETRO_DEVICE_MOUSE => handle_mouse(port, id),
        _ => handle_input_legacy(port, device, index, id),
    }
}
```

## Save State Migration

### Version Detection and Migration

Save states are versioned to ensure compatibility:

```rust
#[derive(Serialize, Deserialize)]
pub struct SaveState {
    version: u32,
    #[serde(flatten)]
    data: SaveStateData,
}

pub fn load_state(bytes: &[u8]) -> Result<SaveState> {
    // Try to detect version
    let version = detect_save_state_version(bytes)?;
    
    match version {
        1 => migrate_v1_to_current(bytes),
        2 => load_v2_state(bytes),
        CURRENT_VERSION => deserialize_state(bytes),
        _ => Err(Error::UnsupportedSaveStateVersion(version)),
    }
}

fn migrate_v1_to_current(bytes: &[u8]) -> Result<SaveState> {
    // Load v1 state
    let v1_state: SaveStateV1 = bincode::deserialize(bytes)?;
    
    // Migrate to current format
    let current_state = SaveState {
        version: CURRENT_VERSION,
        data: SaveStateData {
            cpu: migrate_cpu_state(v1_state.cpu),
            gpu: migrate_gpu_state(v1_state.gpu),
            spu: migrate_spu_state(v1_state.spu),
            // New fields with defaults
            performance_stats: Default::default(),
            enhancement_state: Default::default(),
        },
    };
    
    Ok(current_state)
}
```

### Field Migration Examples

```rust
// CPU state migration
fn migrate_cpu_state(v1: CpuStateV1) -> CpuState {
    CpuState {
        // Existing fields copied
        registers: v1.registers,
        pc: v1.pc,
        hi: v1.hi,
        lo: v1.lo,
        
        // New fields with safe defaults
        instruction_cache: Default::default(),
        pipeline_state: PipelineState::default(),
        performance_counters: Default::default(),
    }
}

// GPU state migration with resolution scaling
fn migrate_gpu_state(v1: GpuStateV1) -> GpuState {
    GpuState {
        // Core state preserved
        vram: v1.vram,
        display_mode: v1.display_mode,
        drawing_area: v1.drawing_area,
        
        // New renderer state
        renderer: RendererState {
            backend: RendererBackend::Software, // Default to software for compatibility
            internal_resolution: 1, // Native resolution
            shaders: Vec::new(),
        },
    }
}
```

## Configuration Migration

### Settings File Migration

```rust
pub fn migrate_config(config_path: &Path) -> Result<Config> {
    let raw = std::fs::read_to_string(config_path)?;
    
    // Try parsing as current version
    if let Ok(config) = toml::from_str::<Config>(&raw) {
        return Ok(config);
    }
    
    // Try parsing as v1
    if let Ok(v1_config) = toml::from_str::<ConfigV1>(&raw) {
        return Ok(migrate_config_v1(v1_config));
    }
    
    // Manual migration for very old configs
    Ok(migrate_legacy_config(&raw)?)
}

fn migrate_config_v1(v1: ConfigV1) -> Config {
    Config {
        // Core settings preserved
        bios_path: v1.bios_path,
        save_path: v1.save_path,
        
        // Graphics settings with defaults
        graphics: GraphicsConfig {
            renderer: if v1.use_opengl { 
                RendererBackend::OpenGL 
            } else { 
                RendererBackend::Software 
            },
            internal_resolution: 1,
            texture_filtering: TextureFilter::Nearest,
            ..Default::default()
        },
        
        // New features disabled by default
        enhancements: EnhancementConfig {
            achievements: false,
            video_recording: false,
            performance_overlay: false,
            ..Default::default()
        },
    }
}
```

### Environment Variable Compatibility

```rust
pub fn load_config() -> Config {
    let mut config = Config::default();
    
    // Legacy environment variables (preserved)
    if let Ok(bios) = env::var("RUSTATION_BIOS") {
        config.bios_path = PathBuf::from(bios);
    }
    
    // New environment variables (optional)
    if let Ok(renderer) = env::var("RUSTATION_RENDERER") {
        config.graphics.renderer = renderer.parse().unwrap_or_default();
    }
    
    // Feature flags
    if env::var("RUSTATION_LEGACY_MODE").is_ok() {
        config.compatibility_mode = true;
        disable_all_enhancements(&mut config);
    }
    
    config
}
```

## Feature Flags

### Compile-Time Features

```toml
[features]
default = ["software-renderer", "libretro"]

# Core features (always included)
software-renderer = []
libretro = []

# Optional enhancements
opengl-renderer = ["gl", "glsl"]
vulkan-renderer = ["ash", "spirv"]
achievements = ["rcheevos"]
video-recording = ["ffmpeg-sys"]
performance-monitor = []
network-play = ["ggpo"]

# Compatibility mode
legacy-mode = []  # Disables all enhancements
```

### Runtime Feature Detection

```rust
pub struct FeatureFlags {
    pub hardware_rendering: bool,
    pub achievements: bool,
    pub video_recording: bool,
    pub performance_overlay: bool,
    pub network_play: bool,
}

impl FeatureFlags {
    pub fn detect() -> Self {
        FeatureFlags {
            hardware_rendering: cfg!(feature = "opengl-renderer") 
                && gpu_available(),
            achievements: cfg!(feature = "achievements") 
                && !legacy_mode(),
            video_recording: cfg!(feature = "video-recording") 
                && ffmpeg_available(),
            performance_overlay: cfg!(feature = "performance-monitor"),
            network_play: cfg!(feature = "network-play"),
        }
    }
    
    pub fn apply_compatibility_mode(&mut self) {
        // Disable all enhancements for maximum compatibility
        self.hardware_rendering = false;
        self.achievements = false;
        self.video_recording = false;
        self.performance_overlay = false;
        self.network_play = false;
    }
}
```

## Testing Migration

### Regression Test Suite

```rust
#[cfg(test)]
mod migration_tests {
    use super::*;
    
    #[test]
    fn test_v1_save_state_loading() {
        let v1_state = include_bytes!("test_data/save_state_v1.bin");
        let state = load_state(v1_state).unwrap();
        
        assert_eq!(state.version, CURRENT_VERSION);
        assert_eq!(state.data.cpu.pc, 0x80001000); // Verify data preserved
    }
    
    #[test]
    fn test_legacy_config_migration() {
        let legacy_config = r#"
            bios_path = "/path/to/bios"
            memory_card_1 = "card1.mcd"
        "#;
        
        let config = migrate_legacy_config(legacy_config).unwrap();
        assert_eq!(config.bios_path, PathBuf::from("/path/to/bios"));
        assert!(!config.enhancements.achievements); // New features off by default
    }
    
    #[test]
    fn test_api_compatibility() {
        // Test that old API calls still work
        let mut ctx = Context::new();
        
        // Legacy API
        retro_init();
        retro_load_game(&game_info);
        retro_run();
        
        // Should work without any new features
        assert!(ctx.is_running());
        assert!(!ctx.hardware_rendering_active());
    }
}
```

### Compatibility Test Matrix

```yaml
# .github/workflows/compatibility.yml
name: Compatibility Tests

on: [push, pull_request]

jobs:
  test-matrix:
    strategy:
      matrix:
        include:
          - name: "Legacy Mode"
            features: "--features legacy-mode"
            
          - name: "Software Only"
            features: "--features software-renderer"
            
          - name: "Full Features"
            features: "--all-features"
            
          - name: "Minimal Build"
            features: "--no-default-features --features libretro"
    
    steps:
      - uses: actions/checkout@v2
      
      - name: Run compatibility tests
        run: |
          cargo test ${{ matrix.features }} --test compatibility
          
      - name: Test save state compatibility
        run: |
          ./scripts/test_save_states.sh ${{ matrix.features }}
          
      - name: Test config migration
        run: |
          ./scripts/test_config_migration.sh
```

## Rollback Procedures

### Version Rollback Support

```rust
pub struct VersionManager {
    current_version: Version,
    fallback_version: Option<Version>,
}

impl VersionManager {
    pub fn initialize() -> Result<Self> {
        let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
        
        // Check for compatibility issues
        if let Err(e) = self.verify_compatibility() {
            warn!("Compatibility issue detected: {}", e);
            
            // Attempt to use fallback mode
            if let Some(fallback) = Self::find_fallback_version() {
                info!("Using fallback version: {}", fallback);
                return Ok(VersionManager {
                    current_version: fallback,
                    fallback_version: Some(current),
                });
            }
        }
        
        Ok(VersionManager {
            current_version: current,
            fallback_version: None,
        })
    }
    
    fn verify_compatibility(&self) -> Result<()> {
        // Check critical components
        self.verify_save_states()?;
        self.verify_configuration()?;
        self.verify_bios_compatibility()?;
        Ok(())
    }
}
```

### Emergency Compatibility Mode

```rust
pub fn enable_emergency_compatibility_mode() {
    warn!("Enabling emergency compatibility mode");
    
    // Disable all enhancements
    set_renderer(RendererBackend::Software);
    disable_achievements();
    disable_video_recording();
    disable_performance_overlay();
    
    // Use conservative timing
    set_timing_mode(TimingMode::Conservative);
    
    // Disable all optimizations
    disable_jit();
    disable_fastmem();
    
    // Log compatibility mode
    info!("Running in emergency compatibility mode - all enhancements disabled");
}
```

## Migration Checklist

### For Users

- [ ] Backup existing save states before upgrading
- [ ] Backup configuration files
- [ ] Test with one game before full migration
- [ ] Report any compatibility issues

### For Developers

- [ ] Run full regression test suite
- [ ] Test save state migration from v1.x
- [ ] Verify API compatibility
- [ ] Test with popular frontends (RetroArch, etc.)
- [ ] Document any breaking changes
- [ ] Update migration scripts

## Support

If you encounter any issues during migration:

1. Enable legacy mode: `RUSTATION_LEGACY_MODE=1`
2. Check the [compatibility matrix](docs/compatibility.md)
3. Report issues at [GitHub Issues](https://github.com/rustation/rustation-ng/issues)
4. Join our [Discord](https://discord.gg/rustation) for support

## Version History

| Version | Release Date | Migration Notes |
|---------|-------------|-----------------|
| 1.0.0   | 2024-01-01  | Initial release |
| 1.5.0   | 2024-06-01  | Save state format v1 |
| 2.0.0   | 2025-01-01  | Major enhancements, full backward compatibility |

---
*Last Updated: 2025-08-09*
*Migration Guide Version: 2.0*