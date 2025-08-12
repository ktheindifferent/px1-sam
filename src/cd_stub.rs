// CD-ROM stub module for WASM build (replaces cdimage dependency)

use std::fmt;

// Stub types to replace cdimage crate types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Msf {
    pub m: u8,
    pub s: u8,
    pub f: u8,
}

impl Msf {
    pub fn new(m: u8, s: u8, f: u8) -> Self {
        Msf { m, s, f }
    }

    pub fn from_bcd(bcd: Bcd) -> Self {
        Msf {
            m: bcd.0,
            s: 0,
            f: 0,
        }
    }

    pub fn to_sector_index(&self) -> u32 {
        let m = self.m as u32;
        let s = self.s as u32;
        let f = self.f as u32;

        // MSF to sector: (M * 60 + S) * 75 + F - 150
        if m * 60 + s >= 2 {
            (m * 60 + s - 2) * 75 + f
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bcd(pub u8);

impl Bcd {
    pub fn new(val: u8) -> Self {
        Bcd(val)
    }

    pub fn from_binary(val: u8) -> Self {
        let tens = val / 10;
        let ones = val % 10;
        Bcd((tens << 4) | ones)
    }

    pub fn to_binary(&self) -> u8 {
        let tens = (self.0 >> 4) & 0xf;
        let ones = self.0 & 0xf;
        tens * 10 + ones
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiscPosition {
    pub track: u8,
    pub index: u8,
    pub relative_msf: Msf,
    pub absolute_msf: Msf,
}

impl Default for DiscPosition {
    fn default() -> Self {
        DiscPosition {
            track: 1,
            index: 1,
            relative_msf: Msf::new(0, 0, 0),
            absolute_msf: Msf::new(0, 2, 0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sector {
    pub data: Vec<u8>,
    pub mode: u8,
}

impl Sector {
    pub fn new() -> Self {
        Sector {
            data: vec![0; 2352],
            mode: 2,
        }
    }

    pub fn data_2048(&self) -> &[u8] {
        if self.data.len() >= 2048 {
            &self.data[0..2048]
        } else {
            &self.data
        }
    }
}

#[derive(Debug)]
pub struct CdError(String);

impl fmt::Display for CdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CD Error: {}", self.0)
    }
}

impl std::error::Error for CdError {}

pub type CdResult<T> = Result<T, CdError>;

// Stub Image trait for disc images
pub trait Image: Send {
    fn read_sector(&mut self, pos: &DiscPosition) -> CdResult<Sector>;
    fn track_count(&self) -> u8;
}

// Stub disc implementation
pub struct StubDisc {
    sectors: Vec<Sector>,
}

impl StubDisc {
    pub fn new() -> Self {
        StubDisc {
            sectors: vec![Sector::new(); 4000], // Minimal disc
        }
    }
}

impl Image for StubDisc {
    fn read_sector(&mut self, pos: &DiscPosition) -> CdResult<Sector> {
        let idx = pos.absolute_msf.to_sector_index() as usize;
        if idx < self.sectors.len() {
            Ok(self.sectors[idx].clone())
        } else {
            Err(CdError("Sector out of range".to_string()))
        }
    }

    fn track_count(&self) -> u8 {
        1
    }
}

// Stub TOC (Table of Contents)
#[derive(Debug, Clone)]
pub struct Toc {
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub number: u8,
    pub start: Msf,
    pub track_type: TrackType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    Data,
    Audio,
}

impl Default for Toc {
    fn default() -> Self {
        Toc {
            tracks: vec![Track {
                number: 1,
                start: Msf::new(0, 2, 0),
                track_type: TrackType::Data,
            }],
        }
    }
}

// XA Audio format stubs
#[derive(Debug, Clone, Copy)]
pub struct XaSamplingFreq(pub u8);

#[derive(Debug, Clone, Copy)]
pub struct XaBitsPerSample(pub u8);

#[derive(Debug, Clone, Copy)]
pub struct XaCodingAudio {
    pub stereo: bool,
    pub freq: XaSamplingFreq,
    pub bits: XaBitsPerSample,
}

pub mod sector {
    // Re-export sector-related types (currently unused but may be needed later)
    #[allow(unused_imports)]
    pub use super::{Sector, XaBitsPerSample, XaCodingAudio, XaSamplingFreq};
}
