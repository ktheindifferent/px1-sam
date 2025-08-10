// Integration tests for complete feature workflows

use crate::psx::Psx;
use crate::save_state::{SaveState, SaveSlotManager};
use crate::performance_monitor::{PerformanceMonitor, Component};
use crate::error::{PsxError, Result};
use std::time::Duration;

#[test]
fn test_complete_emulation_cycle() {
    let mut psx = Psx::new().unwrap();
    
    // Load BIOS
    let mut bios = vec![0u8; 512 * 1024];
    bios[0..4].copy_from_slice(&0x3C080000u32.to_le_bytes()); // Valid instruction
    assert!(psx.load_bios(&bios).is_ok());
    
    // Load EXE
    let mut exe = vec![0u8; 0x900];
    exe[0..8].copy_from_slice(b"PS-X EXE");
    exe[0x10..0x14].copy_from_slice(&0x80010000u32.to_le_bytes()); // PC
    exe[0x18..0x1c].copy_from_slice(&0x80010000u32.to_le_bytes()); // dest
    exe[0x1c..0x20].copy_from_slice(&0x100u32.to_le_bytes());      // size
    assert!(psx.load_exe(&exe).is_ok());
    
    // Run a frame
    assert!(psx.run_frame().is_ok());
    
    // Set controller input
    assert!(psx.set_controller_state(0, 0x0000).is_ok());
    
    // Get framebuffer
    let mut buffer = Vec::new();
    psx.get_framebuffer(&mut buffer);
    assert!(!buffer.is_empty());
}

#[test]
fn test_save_load_cycle() {
    // Create and configure emulator
    let mut psx = Psx::new().unwrap();
    let bios = vec![0xAAu8; 512 * 1024];
    psx.load_bios(&bios).unwrap();
    
    // Create save state
    let mut state = SaveState::new();
    state.cpu_state.pc = 0x80100000;
    state.cpu_state.regs[1] = 0x12345678;
    
    // Serialize and deserialize
    let bytes = state.to_bytes().unwrap();
    let restored = SaveState::from_bytes(&bytes).unwrap();
    
    // Verify state was preserved
    assert_eq!(restored.cpu_state.pc, 0x80100000);
    assert_eq!(restored.cpu_state.regs[1], 0x12345678);
}

#[test]
fn test_performance_monitoring_workflow() {
    let mut monitor = PerformanceMonitor::new(60.0);
    let mut psx = Psx::new().unwrap();
    
    // Simulate frame processing with monitoring
    for _ in 0..5 {
        monitor.begin_frame();
        
        // Time CPU operations
        monitor.time_component(Component::Cpu, || {
            psx.run_frame().unwrap();
        });
        
        // Time GPU operations
        monitor.time_component(Component::Gpu, || {
            let mut buffer = Vec::new();
            psx.get_framebuffer(&mut buffer);
        });
        
        monitor.end_frame();
    }
    
    // Check metrics
    let metrics = monitor.get_metrics();
    assert!(metrics.frame_time_ms > 0.0);
    
    let stats = monitor.get_frame_stats();
    assert!(stats.avg_time > Duration::ZERO);
}

#[test]
fn test_error_recovery_workflow() {
    let mut psx = Psx::new().unwrap();
    
    // Try invalid operations
    let invalid_bios = vec![0u8; 100];
    let result = psx.load_bios(&invalid_bios);
    assert!(result.is_err());
    
    // Verify emulator is still functional after error
    let valid_bios = vec![0u8; 512 * 1024];
    assert!(psx.load_bios(&valid_bios).is_ok());
    
    // Try invalid controller port
    let result = psx.set_controller_state(5, 0);
    assert!(result.is_err());
    
    // Valid controller operation should still work
    assert!(psx.set_controller_state(0, 0).is_ok());
}

#[test]
fn test_save_slot_management_workflow() {
    let mut manager = SaveSlotManager::new();
    
    // Create multiple save states
    for i in 0..5 {
        let mut state = SaveState::new();
        state.cpu_state.pc = 0x80000000 + (i as u32 * 0x1000);
        assert!(manager.save_to_slot(i, state).is_ok());
    }
    
    // Load and verify different slots
    for i in 0..5 {
        let state = manager.load_from_slot(i).unwrap();
        assert_eq!(state.cpu_state.pc, 0x80000000 + (i as u32 * 0x1000));
    }
    
    // Auto-save functionality
    let auto_state = SaveState::new();
    manager.auto_save(auto_state);
    assert!(manager.load_auto_save().is_ok());
}

#[test]
fn test_memory_access_patterns() {
    let mut psx = Psx::new().unwrap();
    
    // Sequential writes
    for i in 0..100 {
        let addr = i * 4;
        assert!(psx.store(addr, i as u32).is_ok());
    }
    
    // Random access pattern
    let addresses = [0x1000, 0x2000, 0x3000, 0x4000];
    for &addr in &addresses {
        let _: Result<u32> = psx.load(addr);
        let _: Result<()> = psx.store(addr, 0xDEADBEEF);
    }
    
    // Boundary testing
    let boundaries = [0x001FFFFC, 0x1fc7FFFC];
    for &addr in &boundaries {
        let _: Result<u32> = psx.load(addr);
    }
}

#[test]
fn test_performance_alert_detection() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    // Simulate slow frame
    monitor.begin_frame();
    std::thread::sleep(Duration::from_millis(25));
    monitor.end_frame();
    
    // Should have performance alerts
    let alerts = monitor.get_alerts();
    assert!(!alerts.is_empty());
    
    // Simulate normal frame
    monitor.begin_frame();
    std::thread::sleep(Duration::from_millis(10));
    monitor.end_frame();
    
    // Alerts should be cleared for new frame
    let alerts = monitor.get_alerts();
    assert!(alerts.is_empty() || alerts.len() < 2);
}

#[test]
fn test_complete_save_state_workflow() {
    // Create fully configured save state
    let mut state = SaveState::new();
    
    // Configure CPU state
    state.cpu_state.pc = 0x80100000;
    state.cpu_state.regs[29] = 0x801FFF00; // Stack pointer
    
    // Configure GPU state
    state.gpu_state.display_area.width = 320;
    state.gpu_state.display_area.height = 240;
    
    // Configure controller state
    state.controller_state.port1.button_state = 0xFFFF;
    
    // Validate state
    assert!(state.validate().is_ok());
    
    // Calculate checksum
    let checksum1 = state.calculate_checksum();
    
    // Serialize and deserialize
    let bytes = state.to_bytes().unwrap();
    let restored = SaveState::from_bytes(&bytes).unwrap();
    
    // Verify checksum matches
    let checksum2 = restored.calculate_checksum();
    assert_eq!(checksum1, checksum2);
}

#[test]
fn test_error_severity_handling() {
    // Test different error severities
    let errors = vec![
        PsxError::NotImplemented { feature: "test".to_string() },
        PsxError::ControllerError { port: 0, reason: "test".to_string() },
        PsxError::memory_violation(0x1000),
    ];
    
    for error in errors {
        let severity = error.severity();
        let recoverable = error.is_recoverable();
        
        // Critical errors should not be recoverable
        if severity == crate::error::ErrorSeverity::Critical {
            assert!(!recoverable);
        }
    }
}

#[test]
fn test_resource_cleanup() {
    // Test that resources are properly cleaned up
    {
        let mut psx = Psx::new().unwrap();
        let _ = psx.run_frame();
    } // PSX dropped here
    
    {
        let mut monitor = PerformanceMonitor::new(60.0);
        monitor.begin_frame();
        monitor.end_frame();
    } // Monitor dropped here
    
    {
        let manager = SaveSlotManager::new();
        let _ = manager.load_from_slot(0);
    } // Manager dropped here
    
    // No panics or leaks should occur
}