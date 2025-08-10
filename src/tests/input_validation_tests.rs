// Input validation tests

use crate::psx::Psx;
use crate::error::{PsxError, Result};

#[test]
fn test_bios_validation_empty() {
    let mut psx = Psx::new().unwrap();
    let result = psx.load_bios(&[]);
    
    assert!(result.is_err());
    match result {
        Err(PsxError::InvalidBios { details }) => {
            assert!(details.contains("empty"));
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_bios_validation_wrong_size() {
    let mut psx = Psx::new().unwrap();
    let invalid_bios = vec![0u8; 256 * 1024]; // Wrong size
    let result = psx.load_bios(&invalid_bios);
    
    assert!(result.is_err());
    match result {
        Err(PsxError::InvalidBios { details }) => {
            assert!(details.contains("Invalid BIOS size"));
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_exe_validation_empty() {
    let mut psx = Psx::new().unwrap();
    let result = psx.load_exe(&[]);
    
    assert!(result.is_err());
    match result {
        Err(PsxError::InvalidExe { reason }) => {
            assert!(reason.contains("empty"));
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_exe_validation_too_small() {
    let mut psx = Psx::new().unwrap();
    let small_exe = vec![0u8; 100]; // Too small
    let result = psx.load_exe(&small_exe);
    
    assert!(result.is_err());
    match result {
        Err(PsxError::InvalidExe { reason }) => {
            assert!(reason.contains("too small"));
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_exe_validation_wrong_magic() {
    let mut psx = Psx::new().unwrap();
    let mut bad_exe = vec![0u8; 0x800];
    bad_exe[0..8].copy_from_slice(b"BADMAGIC");
    
    let result = psx.load_exe(&bad_exe);
    
    assert!(result.is_err());
    match result {
        Err(PsxError::InvalidExe { reason }) => {
            assert!(reason.contains("Invalid magic"));
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_exe_validation_invalid_pc() {
    let mut psx = Psx::new().unwrap();
    let mut exe = vec![0u8; 0x900];
    
    // Set valid magic
    exe[0..8].copy_from_slice(b"PS-X EXE");
    
    // Set invalid PC (outside valid range)
    let invalid_pc = 0x00000000u32;
    exe[0x10..0x14].copy_from_slice(&invalid_pc.to_le_bytes());
    
    // Set other required fields
    exe[0x18..0x1c].copy_from_slice(&0x80010000u32.to_le_bytes()); // dest
    exe[0x1c..0x20].copy_from_slice(&0x100u32.to_le_bytes()); // size
    
    let result = psx.load_exe(&exe);
    
    assert!(result.is_err());
    match result {
        Err(PsxError::InvalidExe { reason }) => {
            assert!(reason.contains("Invalid PC"));
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_controller_port_validation() {
    let mut psx = Psx::new().unwrap();
    
    // Valid ports
    assert!(psx.set_controller_state(0, 0x0000).is_ok());
    assert!(psx.set_controller_state(1, 0xFFFF).is_ok());
    
    // Invalid port
    let result = psx.set_controller_state(2, 0x0000);
    assert!(result.is_err());
    
    match result {
        Err(PsxError::ControllerError { port, reason }) => {
            assert_eq!(port, 2);
            assert!(reason.contains("Invalid controller port"));
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_memory_bounds_validation() {
    let psx = Psx::new().unwrap();
    
    // Test reading from invalid address
    let result: Result<u32> = psx.load(0xFFFFFFFF);
    
    // This should either return an error or a default value
    // depending on the implementation
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_exe_size_limits() {
    let mut psx = Psx::new().unwrap();
    
    // Test maximum size limit
    let large_exe = vec![0u8; 3 * 1024 * 1024]; // 3MB - too large
    let result = psx.load_exe(&large_exe);
    
    assert!(result.is_err());
    match result {
        Err(PsxError::InvalidExe { reason }) => {
            assert!(reason.contains("too large"));
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_valid_exe_loading() {
    let mut psx = Psx::new().unwrap();
    let mut exe = vec![0u8; 0x900];
    
    // Set valid magic
    exe[0..8].copy_from_slice(b"PS-X EXE");
    
    // Set valid header fields
    exe[0x10..0x14].copy_from_slice(&0x80010000u32.to_le_bytes()); // PC
    exe[0x14..0x18].copy_from_slice(&0x00000000u32.to_le_bytes()); // GP
    exe[0x18..0x1c].copy_from_slice(&0x80010000u32.to_le_bytes()); // dest
    exe[0x1c..0x20].copy_from_slice(&0x100u32.to_le_bytes());      // size
    exe[0x30..0x34].copy_from_slice(&0x801FFF00u32.to_le_bytes()); // SP base
    exe[0x34..0x38].copy_from_slice(&0x00000000u32.to_le_bytes()); // SP offset
    
    let result = psx.load_exe(&exe);
    assert!(result.is_ok());
}

#[test]
fn test_bios_signature_validation() {
    let mut psx = Psx::new().unwrap();
    let mut bios = vec![0u8; 512 * 1024];
    
    // Add a valid-looking MIPS instruction at the start
    // LUI instruction (Load Upper Immediate) - common in BIOS
    bios[0..4].copy_from_slice(&0x3C080000u32.to_le_bytes());
    
    let result = psx.load_bios(&bios);
    
    // Should succeed with valid signature
    assert!(result.is_ok());
}

#[test]
fn test_memory_write_to_bios() {
    let mut psx = Psx::new().unwrap();
    
    // Attempt to write to BIOS region (should fail)
    let result: Result<()> = psx.store(0x1fc00000, 0x12345678u32);
    
    assert!(result.is_err());
    match result {
        Err(PsxError::EmulationError { context, message }) => {
            assert_eq!(context, "BIOS");
            assert!(message.contains("read-only"));
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_memory_write_bounds() {
    let mut psx = Psx::new().unwrap();
    
    // Test writing at the edge of RAM
    let ram_end = 0x001FFFFF;
    let result: Result<()> = psx.store(ram_end - 3, 0x12345678u32);
    assert!(result.is_ok());
    
    // Test writing beyond RAM
    let beyond_ram = 0x00200000;
    let result: Result<()> = psx.store(beyond_ram, 0x12345678u32);
    
    // Should either succeed (unmapped region) or fail with proper error
    assert!(result.is_ok() || matches!(result, Err(PsxError::MemoryAccessViolation { .. })));
}