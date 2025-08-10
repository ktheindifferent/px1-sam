// Error handling tests

use crate::error::{PsxError, ErrorSeverity};

#[test]
fn test_error_creation() {
    let err = PsxError::invalid_bios("Test BIOS error");
    match err {
        PsxError::InvalidBios { details } => {
            assert_eq!(details, "Test BIOS error");
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_memory_violation_error() {
    let err = PsxError::memory_violation(0xDEADBEEF);
    match err {
        PsxError::MemoryAccessViolation { address } => {
            assert_eq!(address, 0xDEADBEEF);
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_error_severity() {
    let critical_err = PsxError::memory_violation(0x1000);
    assert_eq!(critical_err.severity(), ErrorSeverity::Critical);
    
    let warning_err = PsxError::NotImplemented {
        feature: "test".to_string(),
    };
    assert_eq!(warning_err.severity(), ErrorSeverity::Warning);
    
    let normal_err = PsxError::ControllerError {
        port: 0,
        reason: "test".to_string(),
    };
    assert_eq!(normal_err.severity(), ErrorSeverity::Error);
}

#[test]
fn test_error_recoverability() {
    let recoverable = PsxError::ControllerError {
        port: 0,
        reason: "test".to_string(),
    };
    assert!(recoverable.is_recoverable());
    
    let non_recoverable = PsxError::memory_violation(0x1000);
    assert!(!non_recoverable.is_recoverable());
}

#[test]
fn test_emulation_error_context() {
    let err = PsxError::emulation("CPU", "Invalid instruction");
    match err {
        PsxError::EmulationError { context, message } => {
            assert_eq!(context, "CPU");
            assert_eq!(message, "Invalid instruction");
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_io_error_with_operation() {
    let err = PsxError::io("file_read", "Permission denied");
    match err {
        PsxError::IoError { operation, message } => {
            assert_eq!(operation, "file_read");
            assert_eq!(message, "Permission denied");
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_save_state_error() {
    let err = PsxError::SaveStateError {
        operation: "serialize".to_string(),
        reason: "Out of memory".to_string(),
    };
    
    match err {
        PsxError::SaveStateError { operation, reason } => {
            assert_eq!(operation, "serialize");
            assert_eq!(reason, "Out of memory");
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_disc_read_error() {
    let err = PsxError::DiscReadError {
        sector: 1234,
        reason: "Sector not found".to_string(),
    };
    
    match err {
        PsxError::DiscReadError { sector, reason } => {
            assert_eq!(sector, 1234);
            assert_eq!(reason, "Sector not found");
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_resource_exhaustion() {
    let err = PsxError::ResourceExhaustion {
        resource: "Memory".to_string(),
        limit: "2MB".to_string(),
    };
    
    assert!(!err.is_recoverable());
    assert_eq!(err.severity(), ErrorSeverity::Critical);
}

#[test]
fn test_configuration_error() {
    let err = PsxError::ConfigurationError {
        setting: "video_mode".to_string(),
        reason: "Invalid resolution".to_string(),
    };
    
    match err {
        PsxError::ConfigurationError { setting, reason } => {
            assert_eq!(setting, "video_mode");
            assert_eq!(reason, "Invalid resolution");
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_audio_error() {
    let err = PsxError::AudioError {
        component: "SPU".to_string(),
        reason: "Buffer overflow".to_string(),
    };
    
    match err {
        PsxError::AudioError { component, reason } => {
            assert_eq!(component, "SPU");
            assert_eq!(reason, "Buffer overflow");
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_invalid_input_error() {
    let err = PsxError::InvalidInput {
        parameter: "sector_size".to_string(),
        reason: "Must be 2048 or 2352".to_string(),
    };
    
    match err {
        PsxError::InvalidInput { parameter, reason } => {
            assert_eq!(parameter, "sector_size");
            assert_eq!(reason, "Must be 2048 or 2352");
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_state_error() {
    let err = PsxError::StateError {
        operation: "resume".to_string(),
        reason: "Emulator not initialized".to_string(),
    };
    
    match err {
        PsxError::StateError { operation, reason } => {
            assert_eq!(operation, "resume");
            assert_eq!(reason, "Emulator not initialized");
        }
        _ => panic!("Wrong error type"),
    }
}