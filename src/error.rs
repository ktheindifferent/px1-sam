use crate::psx::cd::disc::SerialNumber;
use crate::psx::iso9660;
use cdimage::CdError;
use std::io;
use thiserror::Error;

pub type Result<T> = ::std::result::Result<T, PsxError>;

#[derive(Error, Debug)]
pub enum PsxError {
    #[error("Input output error: {0}")]
    IoError(#[from] io::Error),
    #[error("We couldn't find the disc's serial number")]
    NoSerialNumber,
    #[error("Bad CD serial number: expected {expected} got {got}")]
    BadSerialNumber {
        expected: SerialNumber,
        got: SerialNumber,
    },
    #[error("The provided BIOS is unknown")]
    UnknownBios,
    #[error("We couldn't find a suitable BIOS")]
    NoBiosFound,
    #[error("Invalid BIOS file `{0}`")]
    BadBios(String),
    #[error("Something went wrong while communicating with the frontend: `{0}`")]
    FrontendError(String),
    #[error("CD layer error: {0}")]
    CdError(#[from] CdError),
    #[error("The disc format was incorrect (i.e. probably not a valid PSX disc image): `{0}`")]
    BadDiscFormat(String),
    #[error("CD ISO filesystem error: `{0}`")]
    IsoError(#[from] iso9660::IsoError),
    #[error("Invalid or unknown CDC firmware")]
    BadCdcFirmware,
    #[error("We couldn't find a suitable CDC firmware image")]
    NoCdcFirmwareFound,
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Emulation error: {0}")]
    EmulationError(String),
    #[error("Save state error during {operation}: {reason}")]
    SaveStateError { operation: String, reason: String },
    #[error("Netplay error during {operation}: {reason}")]
    NetplayError { operation: String, reason: String },
    #[error("Cloud sync error during {operation}: {reason}")]
    CloudSyncError { operation: String, reason: String },
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Authentication error for {provider}: {reason}")]
    AuthenticationError { provider: String, reason: String },
    #[error("Security error in {module}: {reason}")]
    SecurityError {
        module: String,
        reason: String,
    },
    #[error("Invalid patch file")]
    InvalidPatch,
    #[error("Patch format not supported")]
    UnsupportedPatchFormat,
    #[error("Unknown patch format")]
    UnknownPatchFormat,
    #[error("Patch is too large")]
    PatchTooBig,
    #[error("Checksum mismatch in patch")]
    ChecksumMismatch,
    #[error("File not found")]
    FileNotFound,
    #[error("File write error")]
    FileWriteError,
    #[error("Patch already exists")]
    PatchAlreadyExists,
    #[error("Patch not found")]
    PatchNotFound,
    #[error("Profile already exists")]
    ProfileAlreadyExists,
    #[error("Profile not found")]
    ProfileNotFound,
    #[error("Serialization error")]
    SerializationError,
    #[error("Invalid input")]
    InvalidInput,
    #[error("Network error")]
    NetworkError,
    #[error("Web environment error")]
    WebEnvironmentError,
    #[error("Memory forensics error: {0}")]
    ForensicsError(String),
}
