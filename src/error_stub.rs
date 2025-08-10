// Simplified error module for WASM build
use thiserror::Error;

pub type Result<T> = std::result::Result<T, PsxError>;

#[derive(Error, Debug)]
pub enum PsxError {
    #[error("Invalid BIOS file")]
    InvalidBios,
    
    #[error("Invalid PSX-EXE file")]
    InvalidExe,
    
    #[error("Invalid disc format")]
    InvalidDisc,
    
    #[error("Emulation error: {0}")]
    EmulationError(String),
    
    #[error("I/O error: {0}")]
    IoError(String),
}