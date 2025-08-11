mod db;
mod hle;
mod selector;

use crate::box_array::BoxArray;
use crate::error::{PsxError, Result};
use log::warn;
pub use db::Metadata;
pub use hle::HleBios;
pub use selector::BiosSelector;

pub struct Bios {
    rom: BoxArray<u8, BIOS_SIZE>,
    metadata: &'static Metadata,
    /// Optional patches to apply for enhanced compatibility
    patches: BiosPatches,
}

/// Configuration for BIOS patches
#[derive(Debug, Clone, Copy)]
pub struct BiosPatches {
    /// Skip copyright check for faster boot
    pub skip_copyright: bool,
    /// Skip boot logo animation
    pub skip_logo: bool,
    /// Enable region-free patching
    pub region_free: bool,
    /// Enable debug UART output
    pub debug_uart: bool,
}

impl Default for BiosPatches {
    fn default() -> Self {
        BiosPatches {
            skip_copyright: false,
            skip_logo: false,
            region_free: false,
            debug_uart: false,
        }
    }
}

impl Bios {
    /// Create a BIOS image from `binary` and attempt to match it with an entry in the database. If
    /// no match can be found return an error.
    pub fn new(binary: BoxArray<u8, BIOS_SIZE>) -> Result<Bios> {
        Self::new_with_patches(binary, BiosPatches::default())
    }
    
    /// Create a BIOS with custom patches
    pub fn new_with_patches(mut binary: BoxArray<u8, BIOS_SIZE>, patches: BiosPatches) -> Result<Bios> {
        match db::lookup_blob(&binary) {
            Some(metadata) => {
                let mut bios = Bios {
                    rom: binary,
                    metadata,
                    patches,
                };
                
                // Apply requested patches
                bios.apply_patches();
                
                Ok(bios)
            },
            None => {
                // Try to identify BIOS by version bytes if SHA256 doesn't match
                // This allows for patched/modified BIOS files
                if let Some(metadata) = db::identify_by_version(&binary) {
                    warn!("BIOS SHA256 doesn't match database, but version identified");
                    let mut bios = Bios {
                        rom: binary,
                        metadata,
                        patches,
                    };
                    bios.apply_patches();
                    Ok(bios)
                } else {
                    Err(PsxError::UnknownBios)
                }
            }
        }
    }
    
    /// Apply configured patches to the BIOS
    fn apply_patches(&mut self) {
        if self.patches.skip_copyright {
            // Copyright check is typically at a fixed offset per version
            let offset = match self.metadata.version_major {
                1 | 2 => Some(0x6c88),
                3 => Some(0x6cf0),
                4 => Some(0x6d58),
                _ => None,
            };
            
            if let Some(off) = offset {
                db::patch_skip_copyright_check(self, off);
            }
        }
        
        if self.patches.skip_logo {
            if let Some(hook) = self.metadata.animation_jump_hook {
                db::patch_skip_logo_animation(self, hook);
            }
        }
        
        if self.patches.region_free {
            db::patch_region_free(self, self.metadata.version_major);
        }
        
        if self.patches.debug_uart {
            if let Some(patch_fn) = self.metadata.patch_debug_uart {
                patch_fn(self);
            }
        }
    }

    /// Return a static pointer to the BIOS's Metadata
    pub fn metadata(&self) -> &'static Metadata {
        self.metadata
    }

    /// Creates a BIOS instance with content set to all 0s.
    #[allow(dead_code)]
    pub fn new_dummy() -> Bios {
        let rom = BoxArray::from_vec(vec![0; BIOS_SIZE]);

        Bios {
            rom,
            metadata: &db::DATABASE[0],
            patches: BiosPatches::default(),
        }
    }
    
    /// Get the configured patches
    pub fn patches(&self) -> &BiosPatches {
        &self.patches
    }
    
    /// Check if this BIOS supports a specific region
    pub fn supports_region(&self, region: crate::psx::cd::disc::Region) -> bool {
        self.patches.region_free || self.metadata.region == region
    }

    /// Return the raw BIOS ROM
    pub fn get_rom(&self) -> &[u8; BIOS_SIZE] {
        &self.rom
    }
}

/// BIOS images are always 512KB in length
pub const BIOS_SIZE: usize = 512 * 1024;
