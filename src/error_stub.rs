// Enhanced error module for WASM build with comprehensive error handling
use thiserror::Error;
use std::fmt;

pub type Result<T> = std::result::Result<T, PsxError>;

#[derive(Error, Debug)]
pub enum PsxError {
    #[error("Invalid BIOS file: {details}")]
    InvalidBios { 
        details: String 
    },
    
    #[error("Invalid PSX-EXE file: {reason}")]
    InvalidExe {
        reason: String
    },
    
    #[error("Invalid disc format: {format_type}")]
    InvalidDisc {
        format_type: String
    },
    
    #[error("Emulation error: {context} - {message}")]
    EmulationError {
        context: String,
        message: String,
    },
    
    #[error("I/O error: {operation} - {message}")]
    IoError {
        operation: String,
        message: String,
    },
    
    #[error("Memory access violation: address {address:#08x} out of bounds")]
    MemoryAccessViolation {
        address: u32,
    },
    
    #[error("Invalid input: {parameter} - {reason}")]
    InvalidInput {
        parameter: String,
        reason: String,
    },
    
    #[error("State error: {operation} - {reason}")]
    StateError {
        operation: String,
        reason: String,
    },
    
    #[error("Resource exhaustion: {resource} - {limit} exceeded")]
    ResourceExhaustion {
        resource: String,
        limit: String,
    },
    
    #[error("Not implemented: {feature}")]
    NotImplemented {
        feature: String,
    },
    
    #[error("Configuration error: {setting} - {reason}")]
    ConfigurationError {
        setting: String,
        reason: String,
    },
    
    #[error("Save state error: {operation} - {reason}")]
    SaveStateError {
        operation: String,
        reason: String,
    },
    
    #[error("Controller error: port {port} - {reason}")]
    ControllerError {
        port: usize,
        reason: String,
    },
    
    #[error("Audio error: {component} - {reason}")]
    AudioError {
        component: String,
        reason: String,
    },
    
    #[error("Disc read error: sector {sector} - {reason}")]
    DiscReadError {
        sector: usize,
        reason: String,
    },
}

impl PsxError {
    /// Create a simple invalid BIOS error
    pub fn invalid_bios(details: impl Into<String>) -> Self {
        PsxError::InvalidBios {
            details: details.into(),
        }
    }
    
    /// Create a simple invalid EXE error
    pub fn invalid_exe(reason: impl Into<String>) -> Self {
        PsxError::InvalidExe {
            reason: reason.into(),
        }
    }
    
    /// Create a memory access violation error
    pub fn memory_violation(address: u32) -> Self {
        PsxError::MemoryAccessViolation { address }
    }
    
    /// Create an emulation error with context
    pub fn emulation(context: impl Into<String>, message: impl Into<String>) -> Self {
        PsxError::EmulationError {
            context: context.into(),
            message: message.into(),
        }
    }
    
    /// Create an IO error with operation context
    pub fn io(operation: impl Into<String>, message: impl Into<String>) -> Self {
        PsxError::IoError {
            operation: operation.into(),
            message: message.into(),
        }
    }
    
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            PsxError::MemoryAccessViolation { .. } => false,
            PsxError::InvalidBios { .. } => false,
            PsxError::InvalidExe { .. } => false,
            PsxError::ResourceExhaustion { .. } => false,
            _ => true,
        }
    }
    
    /// Get error severity level (for logging)
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            PsxError::MemoryAccessViolation { .. } => ErrorSeverity::Critical,
            PsxError::InvalidBios { .. } => ErrorSeverity::Critical,
            PsxError::InvalidExe { .. } => ErrorSeverity::Error,
            PsxError::ResourceExhaustion { .. } => ErrorSeverity::Critical,
            PsxError::NotImplemented { .. } => ErrorSeverity::Warning,
            PsxError::SaveStateError { .. } => ErrorSeverity::Error,
            _ => ErrorSeverity::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorSeverity::Warning => write!(f, "WARNING"),
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}