// Save state management for PSX emulator
use serde::{Serialize, Deserialize};
use crate::error::{PsxError, Result};
use std::io::{Read, Write};
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::Compression;
use crate::psx::zram::{ZramSystem, DataType, CompressionAlgorithm};

const SAVE_STATE_VERSION: u32 = 1;
const SAVE_STATE_MAGIC: &[u8; 8] = b"PSXSTATE";

/// Save state header for version compatibility
#[derive(Serialize, Deserialize, Debug)]
pub struct SaveStateHeader {
    magic: [u8; 8],
    version: u32,
    timestamp: u64,
    game_id: Option<String>,
    checksum: u32,
}

/// Complete save state for the PSX emulator
#[derive(Serialize, Deserialize, Debug)]
pub struct SaveState {
    header: SaveStateHeader,
    cpu_state: CpuState,
    gpu_state: GpuState,
    spu_state: SpuState,
    memory_state: MemoryState,
    controller_state: ControllerState,
    timing_state: TimingState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CpuState {
    pub pc: u32,
    pub next_pc: u32,
    pub regs: [u32; 32],
    pub hi: u32,
    pub lo: u32,
    pub cop0_regs: [u32; 32],
    pub load_delay_slot: Option<(u8, u32)>,
    pub branch_delay: bool,
    pub exception_pending: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GpuState {
    pub vram: Vec<u16>,
    pub display_mode: u32,
    pub display_area: DisplayArea,
    pub draw_area: DrawArea,
    pub texture_window: TextureWindow,
    pub draw_offset: (i32, i32),
    pub mask_settings: u32,
    pub status_register: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DisplayArea {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DrawArea {
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TextureWindow {
    pub mask_x: u8,
    pub mask_y: u8,
    pub offset_x: u8,
    pub offset_y: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SpuState {
    pub voices: Vec<VoiceState>,
    pub control_register: u16,
    pub status_register: u16,
    pub reverb_settings: ReverbSettings,
    pub volume_left: u16,
    pub volume_right: u16,
    pub cd_volume_left: u16,
    pub cd_volume_right: u16,
    pub ram: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VoiceState {
    pub pitch: u16,
    pub volume_left: u16,
    pub volume_right: u16,
    pub adsr: AdsrState,
    pub current_address: u32,
    pub repeat_address: u32,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdsrState {
    pub attack_rate: u8,
    pub decay_rate: u8,
    pub sustain_level: u8,
    pub sustain_rate: u8,
    pub release_rate: u8,
    pub current_level: u16,
    pub current_phase: AdsrPhase,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum AdsrPhase {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReverbSettings {
    pub enabled: bool,
    pub depth_left: u16,
    pub depth_right: u16,
    pub delay: u16,
    pub feedback: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemoryState {
    pub main_ram: Vec<u8>,
    pub scratchpad: Vec<u8>,
    pub bios: Vec<u8>,
    pub memory_cards: [Option<MemoryCardData>; 2],
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MemoryCardData {
    pub data: Vec<u8>,
    pub dirty: bool,
    pub last_access: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ControllerState {
    pub port1: ControllerData,
    pub port2: ControllerData,
    pub multitap: [bool; 2],
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ControllerData {
    pub controller_type: ControllerType,
    pub button_state: u16,
    pub analog_state: Option<AnalogState>,
    pub rumble_state: Option<RumbleState>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ControllerType {
    Digital,
    Analog,
    DualShock,
    Mouse,
    Guncon,
    NeGcon,
    None,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalogState {
    pub left_x: u8,
    pub left_y: u8,
    pub right_x: u8,
    pub right_y: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RumbleState {
    pub small_motor: u8,
    pub large_motor: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimingState {
    pub system_clock: u64,
    pub gpu_clock: u64,
    pub spu_clock: u64,
    pub timers: [TimerState; 3],
    pub frame_counter: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimerState {
    pub counter: u16,
    pub target: u16,
    pub mode: u16,
    pub prescaler: u8,
    pub irq_pending: bool,
}

impl SaveState {
    /// Create a new save state from current emulator state
    pub fn new() -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        SaveState {
            header: SaveStateHeader {
                magic: *SAVE_STATE_MAGIC,
                version: SAVE_STATE_VERSION,
                timestamp,
                game_id: None,
                checksum: 0,
            },
            cpu_state: CpuState::default(),
            gpu_state: GpuState::default(),
            spu_state: SpuState::default(),
            memory_state: MemoryState::default(),
            controller_state: ControllerState::default(),
            timing_state: TimingState::default(),
        }
    }

    /// Serialize save state to compressed bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        
        // Serialize with bincode
        let data = bincode::serialize(self)
            .map_err(|e| PsxError::SaveStateError {
                operation: "serialize".to_string(),
                reason: e.to_string(),
            })?;
        
        encoder.write_all(&data)
            .map_err(|e| PsxError::SaveStateError {
                operation: "compress".to_string(),
                reason: e.to_string(),
            })?;
        
        encoder.finish()
            .map_err(|e| PsxError::SaveStateError {
                operation: "finalize".to_string(),
                reason: e.to_string(),
            })
    }

    /// Serialize save state with ZRAM compression for better ratio
    pub fn to_bytes_zram(&self) -> Result<Vec<u8>> {
        // Serialize with bincode first
        let data = bincode::serialize(self)
            .map_err(|e| PsxError::SaveStateError {
                operation: "serialize".to_string(),
                reason: e.to_string(),
            })?;
        
        // Use Zstandard with level 5 for save states (good compression)
        let compressed = zstd::encode_all(&data[..], 5)
            .map_err(|e| PsxError::SaveStateError {
                operation: "zram_compress".to_string(),
                reason: e.to_string(),
            })?;
        
        Ok(compressed)
    }
    
    /// Deserialize save state from ZRAM compressed bytes
    pub fn from_bytes_zram(data: &[u8]) -> Result<Self> {
        // Decompress with Zstandard
        let decompressed = zstd::decode_all(data)
            .map_err(|e| PsxError::SaveStateError {
                operation: "zram_decompress".to_string(),
                reason: e.to_string(),
            })?;
        
        let state: SaveState = bincode::deserialize(&decompressed)
            .map_err(|e| PsxError::SaveStateError {
                operation: "deserialize".to_string(),
                reason: e.to_string(),
            })?;
        
        // Validate header
        if state.header.magic != *SAVE_STATE_MAGIC {
            return Err(PsxError::SaveStateError {
                operation: "validate".to_string(),
                reason: "Invalid magic number".to_string(),
            });
        }
        
        if state.header.version > SAVE_STATE_VERSION {
            return Err(PsxError::SaveStateError {
                operation: "validate".to_string(),
                reason: format!("Unsupported version: {}", state.header.version),
            });
        }
        
        Ok(state)
    }

    /// Deserialize save state from compressed bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| PsxError::SaveStateError {
                operation: "decompress".to_string(),
                reason: e.to_string(),
            })?;
        
        let state: SaveState = bincode::deserialize(&decompressed)
            .map_err(|e| PsxError::SaveStateError {
                operation: "deserialize".to_string(),
                reason: e.to_string(),
            })?;
        
        // Validate header
        if state.header.magic != *SAVE_STATE_MAGIC {
            return Err(PsxError::SaveStateError {
                operation: "validate".to_string(),
                reason: "Invalid save state magic".to_string(),
            });
        }
        
        if state.header.version != SAVE_STATE_VERSION {
            return Err(PsxError::SaveStateError {
                operation: "validate".to_string(),
                reason: format!(
                    "Incompatible save state version: expected {}, got {}",
                    SAVE_STATE_VERSION,
                    state.header.version
                ),
            });
        }
        
        Ok(state)
    }

    /// Calculate checksum for integrity verification
    pub fn calculate_checksum(&self) -> u32 {
        // Simple CRC32 checksum
        let data = bincode::serialize(self).unwrap_or_default();
        crc32fast::hash(&data)
    }

    /// Validate save state integrity
    pub fn validate(&self) -> Result<()> {
        // Check basic sanity
        if self.memory_state.main_ram.len() != 2 * 1024 * 1024 {
            return Err(PsxError::SaveStateError {
                operation: "validate".to_string(),
                reason: "Invalid RAM size".to_string(),
            });
        }

        if self.memory_state.bios.len() != 512 * 1024 {
            return Err(PsxError::SaveStateError {
                operation: "validate".to_string(),
                reason: "Invalid BIOS size".to_string(),
            });
        }

        Ok(())
    }
}

// Default implementations
impl Default for CpuState {
    fn default() -> Self {
        CpuState {
            pc: 0xbfc00000,
            next_pc: 0xbfc00004,
            regs: [0; 32],
            hi: 0,
            lo: 0,
            cop0_regs: [0; 32],
            load_delay_slot: None,
            branch_delay: false,
            exception_pending: false,
        }
    }
}

impl Default for GpuState {
    fn default() -> Self {
        GpuState {
            vram: vec![0; 1024 * 512],
            display_mode: 0,
            display_area: DisplayArea {
                x: 0,
                y: 0,
                width: 640,
                height: 480,
            },
            draw_area: DrawArea {
                left: 0,
                top: 0,
                right: 1023,
                bottom: 511,
            },
            texture_window: TextureWindow {
                mask_x: 0,
                mask_y: 0,
                offset_x: 0,
                offset_y: 0,
            },
            draw_offset: (0, 0),
            mask_settings: 0,
            status_register: 0x14802000,
        }
    }
}

impl Default for SpuState {
    fn default() -> Self {
        SpuState {
            voices: vec![VoiceState::default(); 24],
            control_register: 0,
            status_register: 0,
            reverb_settings: ReverbSettings::default(),
            volume_left: 0x3fff,
            volume_right: 0x3fff,
            cd_volume_left: 0,
            cd_volume_right: 0,
            ram: vec![0; 512 * 1024],
        }
    }
}

impl Default for VoiceState {
    fn default() -> Self {
        VoiceState {
            pitch: 0x1000,
            volume_left: 0,
            volume_right: 0,
            adsr: AdsrState::default(),
            current_address: 0,
            repeat_address: 0,
            enabled: false,
        }
    }
}

impl Default for AdsrState {
    fn default() -> Self {
        AdsrState {
            attack_rate: 0,
            decay_rate: 0,
            sustain_level: 0,
            sustain_rate: 0,
            release_rate: 0,
            current_level: 0,
            current_phase: AdsrPhase::Off,
        }
    }
}

impl Default for ReverbSettings {
    fn default() -> Self {
        ReverbSettings {
            enabled: false,
            depth_left: 0,
            depth_right: 0,
            delay: 0,
            feedback: 0,
        }
    }
}

impl Default for MemoryState {
    fn default() -> Self {
        MemoryState {
            main_ram: vec![0; 2 * 1024 * 1024],
            scratchpad: vec![0; 1024],
            bios: vec![0; 512 * 1024],
            memory_cards: [None, None],
        }
    }
}

impl Default for ControllerState {
    fn default() -> Self {
        ControllerState {
            port1: ControllerData::default(),
            port2: ControllerData::default(),
            multitap: [false; 2],
        }
    }
}

impl Default for ControllerData {
    fn default() -> Self {
        ControllerData {
            controller_type: ControllerType::Digital,
            button_state: 0xffff,
            analog_state: None,
            rumble_state: None,
        }
    }
}

impl Default for TimingState {
    fn default() -> Self {
        TimingState {
            system_clock: 0,
            gpu_clock: 0,
            spu_clock: 0,
            timers: [TimerState::default(); 3],
            frame_counter: 0,
        }
    }
}

impl Default for TimerState {
    fn default() -> Self {
        TimerState {
            counter: 0,
            target: 0,
            mode: 0,
            prescaler: 1,
            irq_pending: false,
        }
    }
}

/// Quick save/load slot management
pub struct SaveSlotManager {
    slots: [Option<SaveState>; 10],
    auto_save: Option<SaveState>,
}

impl SaveSlotManager {
    pub fn new() -> Self {
        SaveSlotManager {
            slots: Default::default(),
            auto_save: None,
        }
    }

    pub fn save_to_slot(&mut self, slot: usize, state: SaveState) -> Result<()> {
        if slot >= 10 {
            return Err(PsxError::SaveStateError {
                operation: "save_slot".to_string(),
                reason: format!("Invalid slot number: {}", slot),
            });
        }
        
        self.slots[slot] = Some(state);
        Ok(())
    }

    pub fn load_from_slot(&self, slot: usize) -> Result<SaveState> {
        if slot >= 10 {
            return Err(PsxError::SaveStateError {
                operation: "load_slot".to_string(),
                reason: format!("Invalid slot number: {}", slot),
            });
        }
        
        self.slots[slot].clone().ok_or_else(|| {
            PsxError::SaveStateError {
                operation: "load_slot".to_string(),
                reason: format!("Slot {} is empty", slot),
            }
        })
    }

    pub fn auto_save(&mut self, state: SaveState) {
        self.auto_save = Some(state);
    }

    pub fn load_auto_save(&self) -> Result<SaveState> {
        self.auto_save.clone().ok_or_else(|| {
            PsxError::SaveStateError {
                operation: "load_auto".to_string(),
                reason: "No auto-save available".to_string(),
            }
        })
    }
}