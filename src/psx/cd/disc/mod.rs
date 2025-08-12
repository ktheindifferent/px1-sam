use super::iso9660;
use super::chd;
use super::libcrypt::{LibCrypt, SubchannelQ};
use super::compression;
use crate::error::{PsxError, Result};
use crate::psx::gpu::VideoStandard;
pub use cache::Cache as CdCache;
use cache::CachedResult;
use crate::cdimage::{Bcd, CdResult, DiscPosition, Image, Msf, Sector, Toc};
use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};
use std::fmt;
use std::path::Path;

// Simple logging macro
macro_rules! disc_log {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        eprintln!("[DISC] {}", format!($($arg)*));
    };
}

mod cache;
pub mod advanced_cache;
pub mod advanced_cache_wasm;
pub mod wasm_integration;
pub mod patching_integration;

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod statistics_validation;
#[cfg(test)]
mod regression_tests;

/// PlayStation disc.
///
/// XXX: add support for CD-DA? Not really useful but shouldn't be very hard either. We need to
/// support audio tracks anyway...
pub struct Disc {
    /// Disc image
    cache: CdCache,
    /// Disc serial number
    serial: SerialNumber,
    /// LibCrypt protection handler
    libcrypt: LibCrypt,
}

impl Disc {
    /// Reify a disc using `image` as a backend.
    pub fn new(image: Box<dyn Image + Send>) -> Result<Disc> {
        let mut cache = CdCache::new(image);

        let serial = extract_serial_number(&mut cache)?;
        let libcrypt = LibCrypt::new();

        let disc = Disc { cache, serial, libcrypt };

        Ok(disc)
    }

    /// Load a disc from a CHD file
    pub fn from_chd<P: AsRef<Path>>(path: P) -> Result<Disc> {
        let path_ref = path.as_ref();
        let image = chd::ChdImage::open(path_ref)?;
        let mut disc = Self::new(Box::new(image))?;
        
        // Try to auto-load SBI file
        if let Err(e) = disc.libcrypt.auto_load_sbi(path_ref) {
            disc_log!("Failed to auto-load SBI file: {:?}", e);
        }
        
        Ok(disc)
    }

    /// Load a disc with automatic format detection and support for all compression formats
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Disc> {
        let path_ref = path.as_ref();
        
        // Use automatic format detection
        let format = compression::detect_format(path_ref)?;
        disc_log!("Detected format: {:?}", format);
        
        // Open the image using the compression module
        let image = compression::open_image(path_ref)?;
        
        // Create disc from the image
        let mut disc = Self::new(image)?;
        
        // Try to auto-load SBI file for LibCrypt protection
        if let Err(e) = disc.libcrypt.auto_load_sbi(path_ref) {
            disc_log!("No SBI file found or failed to load: {:?}", e);
        }
        
        Ok(disc)
    }
    
    /// Find a fallback BIN/CUE file for a corrupted CHD
    fn find_fallback_image(chd_path: &Path) -> Option<std::path::PathBuf> {
        // Look for BIN/CUE files with the same base name
        if let Some(stem) = chd_path.file_stem() {
            if let Some(parent) = chd_path.parent() {
                // Check for .cue file
                let cue_path = parent.join(stem).with_extension("cue");
                if cue_path.exists() {
                    return Some(cue_path);
                }
                
                // Check for .bin file
                let bin_path = parent.join(stem).with_extension("bin");
                if bin_path.exists() {
                    return Some(bin_path);
                }
            }
        }
        None
    }
    
    /// Load a fallback image (BIN/CUE or other format)
    fn load_fallback(path: std::path::PathBuf) -> Result<Disc> {
        // In a real implementation, this would use the cdimage crate
        // to load BIN/CUE files. For now, return an error.
        Err(PsxError::BadDiscFormat(
            format!("Fallback loading not yet implemented for: {:?}", path)
        ))
    }

    /// Instantiate a placeholder disc that will generate errors when used
    fn new_placeholder(serial: SerialNumber, toc: Toc) -> Disc {
        Disc {
            cache: CdCache::new_with_toc(Box::new(DummyImage), toc),
            serial,
            libcrypt: LibCrypt::new(),
        }
    }

    pub fn read_sector(&mut self, dp: DiscPosition) -> CachedResult<Sector> {
        self.cache.read_sector(dp)
    }

    pub fn region(&self) -> Region {
        // For now I prefer to panic to catch potential issues with the serial number handling
        // code, alternatively we could fallback on `extract_system_region`
        match self.serial.region() {
            Some(r) => r,
            None => panic!("Can't establish the region of {}", self.serial),
        }
    }

    pub fn serial_number(&self) -> SerialNumber {
        self.serial
    }

    /// Get subchannel Q data for a specific sector
    pub fn get_subchannel_q(&self, msf: [u8; 3]) -> SubchannelQ {
        // Create default subchannel Q data
        let mut default = SubchannelQ::new();
        default.absolute_msf = msf;
        default.crc = default.calculate_crc();
        
        // Get modified data from LibCrypt if available
        self.libcrypt.get_subchannel_q(msf, default)
    }

    /// Check if the disc has LibCrypt protection
    pub fn has_libcrypt(&self) -> bool {
        self.libcrypt.protection_type() != super::libcrypt::ProtectionType::None
    }

    /// Load SBI file for LibCrypt protection
    pub fn load_sbi<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        self.libcrypt.load_sbi(path)
    }

    /// Get LibCrypt protection type
    pub fn libcrypt_type(&self) -> super::libcrypt::ProtectionType {
        self.libcrypt.protection_type()
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SerializedDisc {
    serial: SerialNumber,
    toc: Toc,
}

impl Serialize for Disc {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = SerializedDisc {
            serial: self.serial,
            toc: self.cache.toc().clone(),
        };

        s.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Disc {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = SerializedDisc::deserialize(deserializer)?;

        Ok(Disc::new_placeholder(s.serial, s.toc))
    }
}

/// Disc region
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// Japan (NTSC): SCEI
    Japan,
    /// North America (NTSC): SCEA
    NorthAmerica,
    /// Europe (PAL): SCEE
    Europe,
}

impl Region {
    /// Returns the video standard normally used for the given region
    pub fn video_standard(self) -> VideoStandard {
        match self {
            Region::Japan => VideoStandard::Ntsc,
            Region::NorthAmerica => VideoStandard::Ntsc,
            Region::Europe => VideoStandard::Pal,
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Region::Japan => "Japan",
            Region::NorthAmerica => "NorthAmerica",
            Region::Europe => "Europe",
        };
        write!(f, "{}", s)
    }
}

/// Disc serial number
#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct SerialNumber([u8; 10]);

impl SerialNumber {
    /// Extract a serial number from a standard PlayStation binary
    /// name of the form "aaaa_ddd.dd"
    fn from_bin_name(bin: &[u8]) -> Result<SerialNumber> {
        if bin.len() != 11 {
            return Err(PsxError::NoSerialNumber);
        }

        // Ridge Racer (NA) uses "SCUS-943.00"
        if bin[4] != b'_' && bin[4] != b'-' {
            // This will fail for the few "lightspan educational" discs since they have a serial
            // number looking like "LSP-123456". Those games are fairly obscure and some of them
            // seem to have weird and nonstandards SYSTEM.CNF anyway.
            return Err(PsxError::NoSerialNumber);
        }

        let mut serial = [0u8; 10];

        fn to_upper(b: u8) -> u8 {
            if b.is_ascii_lowercase() {
                b - b'a' + b'A'
            } else {
                b
            }
        }

        serial[0] = to_upper(bin[0]);
        serial[1] = to_upper(bin[1]);
        serial[2] = to_upper(bin[2]);
        serial[3] = to_upper(bin[3]);
        serial[4] = b'-';
        serial[5] = bin[5];
        serial[6] = bin[6];
        serial[7] = bin[7];
        serial[8] = bin[9];
        serial[9] = bin[10];

        Ok(SerialNumber(serial))
    }

    pub fn region(&self) -> Option<Region> {
        match &self.0[0..4] {
            b"SCPS" | b"SLPS" | b"SLPM" | b"PAPX" => Some(Region::Japan),
            b"SCUS" | b"SLUS" | b"LSP-" => Some(Region::NorthAmerica),
            b"SCES" | b"SCED" | b"SLES" | b"SLED" => Some(Region::Europe),
            _ => None,
        }
    }
}

impl fmt::Display for SerialNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

impl ToString for SerialNumber {
    fn to_string(&self) -> String {
        String::from_utf8_lossy(&self.0).to_string()
    }
}

impl fmt::Debug for SerialNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// Attempt to discover the region of the disc using the license string stored in the system area
/// of the official PlayStation ISO filesystem.
#[allow(dead_code)]
pub fn extract_system_region(image: &mut dyn Image) -> Result<Region> {
    // In order to identify the type of disc we're going to use sector 00:00:04 from Track01 which
    // should contain the "Licensed by..."  string.
    let toc = image.toc();
    let track = toc.track(Bcd::ONE)?;
    let msf = track.disc_position(Msf::from_bcd(0x00, 0x00, 0x04).unwrap())?;

    let sector = image.read_sector(msf)?;

    // On the discs I've tried we always have an ASCII license string in the first 76 data bytes.
    // We'll see if it holds true for all the discs out there...
    let license_blob = &sector.mode2_xa_payload()?[0..76];

    // There are spaces everywhere in the license string (including in the middle of some words),
    // let's clean it up and convert to a canonical string
    let license: String = license_blob
        .iter()
        .filter_map(|&b| match b {
            b'A'..=b'z' => Some(b as char),
            _ => None,
        })
        .collect();

    let region = match license.as_ref() {
        "LicensedbySonyComputerEntertainmentInc" => Region::Japan,
        "LicensedbySonyComputerEntertainmentAmerica" => Region::NorthAmerica,
        "LicensedbySonyComputerEntertainmentofAmerica" => Region::NorthAmerica,
        "LicensedbySonyComputerEntertainmentEurope" => Region::Europe,
        _ => {
            let m = format!("Couldn't identify disc region string: {}", license);
            return Err(PsxError::BadDiscFormat(m));
        }
    };

    Ok(region)
}

/// Attempt to extract the serial number of the disc. All officially
/// licensed PlayStation game should have a serial number.
fn extract_serial_number(image: &mut CdCache) -> Result<SerialNumber> {
    let system_cnf = read_system_cnf(image)?;

    parse_serial_number_from_system_cnf(&system_cnf)
}

fn parse_serial_number_from_system_cnf(system_cnf: &[u8]) -> Result<SerialNumber> {
    // Now we need to parse the SYSTEM.CNF file to get the content of the "BOOT" line
    let mut boot_path = None;

    for line in system_cnf.split(|&b| b == b'\n' || b == b'\r') {
        let words: Vec<_> = line
            .split(|&b| b == b' ' || b == b'\t' || b == b'=')
            .filter(|w| !w.is_empty())
            .collect();

        if words.len() == 2 && words[0] == b"BOOT" {
            boot_path = Some(words[1]);
            break;
        }
    }

    let boot_path = match boot_path {
        Some(b) => b,
        None => {
            disc_log!("Couldn't find BOOT line in SYSTEM.CNF");
            return Err(PsxError::NoSerialNumber);
        }
    };

    // boot_path should look like "cdrom:\FOO\BAR\...\aaaa_ddd.dd;1"
    //
    // Most (but not all) paths ends with a ";1", so get rid of it here
    let boot_path = boot_path.split(|&b| b == b';').next().unwrap();

    // boot_path should look like "cdrom:\FOO\BAR\...\aaaa_ddd.dd"
    let bin_name = boot_path
        .split(|&b| b == b':' || b == b'\\')
        .last()
        .unwrap();

    let serial = SerialNumber::from_bin_name(bin_name);

    if serial.is_err() {
        disc_log!("Unexpected bin name: {}", String::from_utf8_lossy(bin_name));
    }

    serial
}

fn read_system_cnf(image: &mut CdCache) -> Result<Vec<u8>> {
    let dir = iso9660::open_image(image)?;

    let system_cnf = dir.entry_by_name(b"SYSTEM.CNF;1")?;

    // SYSTEM.CNF should be a small text file, 1MB should be way more
    // than necessary
    let len = system_cnf.extent_len();

    if len > 1024 * 1024 {
        let desc = format!("SYSTEM.CNF is too big: {}B", len);
        return Err(PsxError::BadDiscFormat(desc));
    }

    let system_cnf = system_cnf.read_file(image)?;

    Ok(system_cnf)
}

/// A placeholder disc image that crashes if we attempt to use it
struct DummyImage;

impl Image for DummyImage {
    fn image_format(&self) -> String {
        "Placeholder disc".to_string()
    }

    fn read_sector(&mut self, _position: DiscPosition) -> CdResult<Sector> {
        unimplemented!("Attempted to read a placeholder disc image")
    }

    fn toc(&self) -> &Toc {
        unimplemented!("Attempted to read the ToC of a placeholder disc image");
    }
}

#[test]
fn parse_sn() {
    fn tst(conf: &[u8], expected: SerialNumber) {
        assert_eq!(parse_serial_number_from_system_cnf(conf).unwrap(), expected);
    }

    // Final Fantasy Tactics (Japan) (Rev 1)
    tst(
        br#"BOOT = cdrom:\SLPS_007.70;1
TCB = 4
EVENT = 16
STACK = 801fff00
"#,
        SerialNumber(*b"SLPS-00770"),
    );

    // Legaia Densetsu (Japan)
    tst(
        br#"BOOT= cdrom:\SCPS_100.59
TCB= 4
EVENT= 10
STACK= 801FFFFC
        "#,
        SerialNumber(*b"SCPS-10059"),
    );

    // Final Fantasy IX (USA) (Disc 1) (Rev 1)
    tst(
        br#"BOOT=cdrom:\SLUS_012.51;1
TCB=4
EVENT=16
STACK=801fff00"#,
        SerialNumber(*b"SLUS-01251"),
    );
}
