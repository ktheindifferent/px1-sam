// Save state tests

use crate::save_state::*;
use crate::error::PsxError;

#[test]
fn test_save_state_creation() {
    let state = SaveState::new();
    assert_eq!(state.header.version, 1);
    assert_eq!(state.header.magic, *b"PSXSTATE");
}

#[test]
fn test_cpu_state_default() {
    let cpu = CpuState::default();
    assert_eq!(cpu.pc, 0xbfc00000);
    assert_eq!(cpu.next_pc, 0xbfc00004);
    assert_eq!(cpu.regs[0], 0);
    assert!(!cpu.branch_delay);
    assert!(!cpu.exception_pending);
}

#[test]
fn test_gpu_state_default() {
    let gpu = GpuState::default();
    assert_eq!(gpu.vram.len(), 1024 * 512);
    assert_eq!(gpu.display_area.width, 640);
    assert_eq!(gpu.display_area.height, 480);
    assert_eq!(gpu.status_register, 0x14802000);
}

#[test]
fn test_save_state_serialization() {
    let state = SaveState::new();
    let bytes = state.to_bytes();
    
    assert!(bytes.is_ok());
    let bytes = bytes.unwrap();
    assert!(!bytes.is_empty());
    
    // Test deserialization
    let restored = SaveState::from_bytes(&bytes);
    assert!(restored.is_ok());
    
    let restored = restored.unwrap();
    assert_eq!(restored.header.version, state.header.version);
}

#[test]
fn test_save_state_validation() {
    let mut state = SaveState::new();
    
    // Valid state should pass
    assert!(state.validate().is_ok());
    
    // Invalid RAM size should fail
    state.memory_state.main_ram = vec![0; 1024];
    assert!(state.validate().is_err());
    
    // Restore valid size
    state.memory_state.main_ram = vec![0; 2 * 1024 * 1024];
    
    // Invalid BIOS size should fail
    state.memory_state.bios = vec![0; 1024];
    assert!(state.validate().is_err());
}

#[test]
fn test_save_slot_manager() {
    let mut manager = SaveSlotManager::new();
    let state = SaveState::new();
    
    // Test saving to slot
    assert!(manager.save_to_slot(0, state.clone()).is_ok());
    assert!(manager.save_to_slot(9, state.clone()).is_ok());
    
    // Test invalid slot
    assert!(manager.save_to_slot(10, state.clone()).is_err());
    
    // Test loading from slot
    let loaded = manager.load_from_slot(0);
    assert!(loaded.is_ok());
    
    // Test loading from empty slot
    let empty = manager.load_from_slot(5);
    assert!(empty.is_err());
}

#[test]
fn test_auto_save() {
    let mut manager = SaveSlotManager::new();
    let state = SaveState::new();
    
    // No auto-save initially
    assert!(manager.load_auto_save().is_err());
    
    // Create auto-save
    manager.auto_save(state);
    
    // Load auto-save
    let loaded = manager.load_auto_save();
    assert!(loaded.is_ok());
}

#[test]
fn test_controller_state() {
    let controller = ControllerState::default();
    
    assert_eq!(controller.port1.controller_type as u8, ControllerType::Digital as u8);
    assert_eq!(controller.port1.button_state, 0xffff);
    assert!(controller.port1.analog_state.is_none());
    assert!(controller.port1.rumble_state.is_none());
    assert!(!controller.multitap[0]);
    assert!(!controller.multitap[1]);
}

#[test]
fn test_spu_state() {
    let spu = SpuState::default();
    
    assert_eq!(spu.voices.len(), 24);
    assert_eq!(spu.volume_left, 0x3fff);
    assert_eq!(spu.volume_right, 0x3fff);
    assert_eq!(spu.ram.len(), 512 * 1024);
    assert!(!spu.reverb_settings.enabled);
}

#[test]
fn test_timing_state() {
    let timing = TimingState::default();
    
    assert_eq!(timing.system_clock, 0);
    assert_eq!(timing.frame_counter, 0);
    assert_eq!(timing.timers.len(), 3);
    
    for timer in &timing.timers {
        assert_eq!(timer.counter, 0);
        assert!(!timer.irq_pending);
    }
}

#[test]
fn test_memory_card_data() {
    let card = MemoryCardData {
        data: vec![0; 128 * 1024], // 128KB memory card
        dirty: false,
        last_access: 0,
    };
    
    assert_eq!(card.data.len(), 128 * 1024);
    assert!(!card.dirty);
}

#[test]
fn test_analog_state() {
    let analog = AnalogState {
        left_x: 128,
        left_y: 128,
        right_x: 128,
        right_y: 128,
    };
    
    // Center position for analog sticks
    assert_eq!(analog.left_x, 128);
    assert_eq!(analog.left_y, 128);
}

#[test]
fn test_adsr_phases() {
    let adsr = AdsrState::default();
    
    assert!(matches!(adsr.current_phase, AdsrPhase::Off));
    assert_eq!(adsr.current_level, 0);
}

#[test]
fn test_save_state_checksum() {
    let state = SaveState::new();
    let checksum1 = state.calculate_checksum();
    
    // Same state should produce same checksum
    let checksum2 = state.calculate_checksum();
    assert_eq!(checksum1, checksum2);
    
    // Modified state should produce different checksum
    let mut modified = state;
    modified.cpu_state.pc = 0x80000000;
    let checksum3 = modified.calculate_checksum();
    assert_ne!(checksum1, checksum3);
}

#[test]
fn test_compressed_save_state() {
    let mut state = SaveState::new();
    
    // Add some data to make compression meaningful
    state.memory_state.main_ram = vec![0xAA; 2 * 1024 * 1024];
    state.gpu_state.vram = vec![0x5555; 1024 * 512];
    
    let compressed = state.to_bytes().unwrap();
    
    // Compressed should be smaller than raw data
    let raw_size = 2 * 1024 * 1024 + (1024 * 512 * 2);
    assert!(compressed.len() < raw_size);
    
    // Should decompress correctly
    let restored = SaveState::from_bytes(&compressed).unwrap();
    assert_eq!(restored.memory_state.main_ram[0], 0xAA);
    assert_eq!(restored.gpu_state.vram[0], 0x5555);
}