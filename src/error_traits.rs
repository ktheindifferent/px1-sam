//! Error Handling Traits and Common Patterns
//!
//! This module provides unified error handling traits to eliminate repetitive
//! error conversion code throughout the codebase, particularly in WASM implementations.

use std::fmt;
use wasm_bindgen::JsValue;

// ============================================================================
// Core Error Trait
// ============================================================================

/// Common trait for all emulator errors
pub trait EmulatorError: fmt::Debug + fmt::Display {
    /// Convert to JavaScript error value for WASM
    fn to_js_error(&self) -> JsValue {
        JsValue::from_str(&self.to_string())
    }
    
    /// Get error category for logging/metrics
    fn category(&self) -> ErrorCategory {
        ErrorCategory::General
    }
    
    /// Check if error is recoverable
    fn is_recoverable(&self) -> bool {
        true
    }
    
    /// Get suggested user action if any
    fn user_action(&self) -> Option<&str> {
        None
    }
}

/// Error categories for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    General,
    Memory,
    Cpu,
    Gpu,
    Audio,
    Input,
    Disc,
    Network,
    Configuration,
    Fatal,
}

// ============================================================================
// Result Type Aliases
// ============================================================================

/// Standard result type for emulator operations
pub type EmulatorResult<T> = Result<T, Box<dyn EmulatorError>>;

/// JavaScript-compatible result type
pub type JsResult<T> = Result<T, JsValue>;

// ============================================================================
// Error Conversion Traits
// ============================================================================

/// Trait for converting errors to EmulatorError
pub trait ToEmulatorError {
    fn to_emulator_error(self) -> Box<dyn EmulatorError>;
}

/// Trait for converting results to JavaScript results
pub trait ToJsResult<T> {
    fn to_js_result(self) -> JsResult<T>;
}

impl<T, E: EmulatorError + 'static> ToJsResult<T> for Result<T, E> {
    fn to_js_result(self) -> JsResult<T> {
        self.map_err(|e| e.to_js_error())
    }
}

impl<T> ToJsResult<T> for Result<T, Box<dyn EmulatorError>> {
    fn to_js_result(self) -> JsResult<T> {
        self.map_err(|e| e.to_js_error())
    }
}

// ============================================================================
// Common Error Types
// ============================================================================

/// Memory access error
#[derive(Debug)]
pub struct MemoryError {
    pub address: u32,
    pub kind: MemoryErrorKind,
}

#[derive(Debug)]
pub enum MemoryErrorKind {
    InvalidAddress,
    UnalignedAccess,
    ReadOnly,
    OutOfBounds,
    SegmentationFault,
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Memory error at {:#010x}: {:?}", self.address, self.kind)
    }
}

impl EmulatorError for MemoryError {
    fn category(&self) -> ErrorCategory {
        ErrorCategory::Memory
    }
    
    fn is_recoverable(&self) -> bool {
        !matches!(self.kind, MemoryErrorKind::SegmentationFault)
    }
}

/// CPU execution error
#[derive(Debug)]
pub struct CpuError {
    pub pc: u32,
    pub instruction: u32,
    pub kind: CpuErrorKind,
}

#[derive(Debug)]
pub enum CpuErrorKind {
    InvalidInstruction,
    UnalignedJump,
    PrivilegeViolation,
    CoprocessorUnavailable,
    ArithmeticOverflow,
    DivisionByZero,
}

impl fmt::Display for CpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CPU error at PC {:#010x} (instruction {:#010x}): {:?}", 
               self.pc, self.instruction, self.kind)
    }
}

impl EmulatorError for CpuError {
    fn category(&self) -> ErrorCategory {
        ErrorCategory::Cpu
    }
    
    fn is_recoverable(&self) -> bool {
        !matches!(self.kind, CpuErrorKind::PrivilegeViolation)
    }
}

/// GPU rendering error
#[derive(Debug)]
pub struct GpuError {
    pub command: u32,
    pub kind: GpuErrorKind,
}

#[derive(Debug)]
pub enum GpuErrorKind {
    InvalidCommand,
    FifoOverflow,
    VramOutOfBounds,
    TextureNotFound,
    InvalidResolution,
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GPU error with command {:#010x}: {:?}", self.command, self.kind)
    }
}

impl EmulatorError for GpuError {
    fn category(&self) -> ErrorCategory {
        ErrorCategory::Gpu
    }
}

/// Disc/CDROM error
#[derive(Debug)]
pub struct DiscError {
    pub kind: DiscErrorKind,
}

#[derive(Debug)]
pub enum DiscErrorKind {
    NoDiscInserted,
    InvalidFormat,
    ReadError(String),
    SeekError,
    CrcMismatch,
    UnsupportedFormat(String),
}

impl fmt::Display for DiscError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DiscErrorKind::NoDiscInserted => write!(f, "No disc inserted"),
            DiscErrorKind::InvalidFormat => write!(f, "Invalid disc format"),
            DiscErrorKind::ReadError(msg) => write!(f, "Disc read error: {}", msg),
            DiscErrorKind::SeekError => write!(f, "Disc seek error"),
            DiscErrorKind::CrcMismatch => write!(f, "Disc CRC mismatch"),
            DiscErrorKind::UnsupportedFormat(fmt) => write!(f, "Unsupported disc format: {}", fmt),
        }
    }
}

impl EmulatorError for DiscError {
    fn category(&self) -> ErrorCategory {
        ErrorCategory::Disc
    }
    
    fn user_action(&self) -> Option<&str> {
        match self.kind {
            DiscErrorKind::NoDiscInserted => Some("Please load a game disc or executable"),
            DiscErrorKind::UnsupportedFormat(_) => Some("This disc format is not supported. Try converting to BIN/CUE format"),
            _ => None,
        }
    }
}

/// Configuration error
#[derive(Debug)]
pub struct ConfigError {
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Configuration error: {}", self.message)
    }
}

impl EmulatorError for ConfigError {
    fn category(&self) -> ErrorCategory {
        ErrorCategory::Configuration
    }
    
    fn user_action(&self) -> Option<&str> {
        Some("Please check your emulator configuration settings")
    }
}

// ============================================================================
// Error Builder Pattern
// ============================================================================

/// Builder for constructing detailed errors
pub struct ErrorBuilder<E> {
    error: E,
    context: Vec<String>,
    recoverable: bool,
}

impl<E: EmulatorError + 'static> ErrorBuilder<E> {
    pub fn new(error: E) -> Self {
        Self {
            error,
            context: Vec::new(),
            recoverable: true,
        }
    }
    
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context.push(context.into());
        self
    }
    
    pub fn non_recoverable(mut self) -> Self {
        self.recoverable = false;
        self
    }
    
    pub fn build(self) -> Box<dyn EmulatorError> {
        Box::new(DetailedError {
            inner: Box::new(self.error),
            context: self.context,
            recoverable: self.recoverable,
        })
    }
}

struct DetailedError {
    inner: Box<dyn EmulatorError>,
    context: Vec<String>,
    recoverable: bool,
}

impl fmt::Debug for DetailedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DetailedError")
            .field("inner", &self.inner)
            .field("context", &self.context)
            .field("recoverable", &self.recoverable)
            .finish()
    }
}

impl fmt::Display for DetailedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)?;
        for ctx in &self.context {
            write!(f, "\n  Context: {}", ctx)?;
        }
        Ok(())
    }
}

impl EmulatorError for DetailedError {
    fn category(&self) -> ErrorCategory {
        self.inner.category()
    }
    
    fn is_recoverable(&self) -> bool {
        self.recoverable && self.inner.is_recoverable()
    }
    
    fn user_action(&self) -> Option<&str> {
        self.inner.user_action()
    }
}

// ============================================================================
// Convenience Macros
// ============================================================================

/// Create a memory error with context
#[macro_export]
macro_rules! memory_error {
    ($addr:expr, $kind:expr) => {
        $crate::error_traits::ErrorBuilder::new(
            $crate::error_traits::MemoryError {
                address: $addr,
                kind: $kind,
            }
        )
    };
    ($addr:expr, $kind:expr, $($context:expr),+) => {
        {
            let mut builder = $crate::error_traits::ErrorBuilder::new(
                $crate::error_traits::MemoryError {
                    address: $addr,
                    kind: $kind,
                }
            );
            $(
                builder = builder.with_context($context);
            )+
            builder.build()
        }
    };
}

/// Create a CPU error with context
#[macro_export]
macro_rules! cpu_error {
    ($pc:expr, $instr:expr, $kind:expr) => {
        $crate::error_traits::ErrorBuilder::new(
            $crate::error_traits::CpuError {
                pc: $pc,
                instruction: $instr,
                kind: $kind,
            }
        )
    };
    ($pc:expr, $instr:expr, $kind:expr, $($context:expr),+) => {
        {
            let mut builder = $crate::error_traits::ErrorBuilder::new(
                $crate::error_traits::CpuError {
                    pc: $pc,
                    instruction: $instr,
                    kind: $kind,
                }
            );
            $(
                builder = builder.with_context($context);
            )+
            builder.build()
        }
    };
}

/// Convert any error to JsValue with logging
#[macro_export]
macro_rules! js_error {
    ($error:expr) => {
        {
            #[cfg(feature = "console_error")]
            web_sys::console::error_1(&JsValue::from_str(&format!("{:?}", $error)));
            JsValue::from_str(&$error.to_string())
        }
    };
}

/// Chain error handling with context
#[macro_export]
macro_rules! chain_error {
    ($result:expr, $context:expr) => {
        $result.map_err(|e| {
            $crate::error_traits::ErrorBuilder::new(e)
                .with_context($context)
                .build()
        })
    };
}

// ============================================================================
// Standard Library Error Implementations
// ============================================================================

impl EmulatorError for std::io::Error {
    fn category(&self) -> ErrorCategory {
        ErrorCategory::General
    }
}

impl<T: EmulatorError + 'static> From<T> for Box<dyn EmulatorError> {
    fn from(error: T) -> Self {
        Box::new(error)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_error_creation() {
        let error = MemoryError {
            address: 0xdeadbeef,
            kind: MemoryErrorKind::InvalidAddress,
        };
        
        assert_eq!(error.category(), ErrorCategory::Memory);
        assert!(error.is_recoverable());
        
        let display = format!("{}", error);
        assert!(display.contains("0xdeadbeef"));
    }
    
    #[test]
    fn test_error_builder() {
        let error = ErrorBuilder::new(CpuError {
            pc: 0x1000,
            instruction: 0x12345678,
            kind: CpuErrorKind::InvalidInstruction,
        })
        .with_context("During game boot")
        .with_context("Instruction fetch phase")
        .non_recoverable()
        .build();
        
        assert_eq!(error.category(), ErrorCategory::Cpu);
        assert!(!error.is_recoverable());
        
        let display = format!("{}", error);
        assert!(display.contains("0x00001000"));
        assert!(display.contains("During game boot"));
    }
    
    #[test]
    fn test_js_result_conversion() {
        fn test_function() -> Result<u32, MemoryError> {
            Err(MemoryError {
                address: 0x1234,
                kind: MemoryErrorKind::OutOfBounds,
            })
        }
        
        let result = test_function().to_js_result();
        assert!(result.is_err());
    }
}