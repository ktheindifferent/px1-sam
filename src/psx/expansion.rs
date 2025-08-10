//! PlayStation Expansion Port Implementation
//! 
//! The expansion port provides access to additional hardware like:
//! - Parallel port for development tools and printers
//! - Additional RAM expansions
//! - Development cartridges
//! - GameShark/Action Replay devices

use super::{AccessWidth, Addressable, CycleCount, Psx};
use std::collections::HashMap;

/// Expansion port device types
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExpansionDevice {
    None,
    ParallelPort,
    DevelopmentCart,
    ActionReplay,
    GameShark,
    RamExpansion,
}

/// Expansion port controller
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ExpansionPort {
    /// Currently connected device
    device: ExpansionDevice,
    /// Device-specific memory/registers
    device_memory: Vec<u8>,
    /// Parallel port data register
    parallel_data: u8,
    /// Parallel port status register
    parallel_status: u8,
    /// Parallel port control register
    parallel_control: u8,
    /// Development cart ROM (if loaded)
    #[serde(skip)]
    dev_cart_rom: Option<Vec<u8>>,
    /// Action Replay codes database
    #[serde(skip)]
    cheat_codes: HashMap<u32, u32>,
}

impl ExpansionPort {
    pub fn new() -> Self {
        ExpansionPort {
            device: ExpansionDevice::None,
            device_memory: vec![0; 0x80000], // 512KB max expansion space
            parallel_data: 0,
            parallel_status: 0x80, // Ready bit set
            parallel_control: 0,
            dev_cart_rom: None,
            cheat_codes: HashMap::new(),
        }
    }

    /// Connect a device to the expansion port
    pub fn connect_device(&mut self, device: ExpansionDevice) {
        self.device = device;
        info!("Expansion port: Connected {:?}", device);
        
        // Initialize device-specific state
        match device {
            ExpansionDevice::ParallelPort => {
                self.parallel_status = 0x80; // Ready
            }
            ExpansionDevice::RamExpansion => {
                // Initialize extra RAM
                self.device_memory.resize(0x200000, 0); // 2MB expansion
            }
            _ => {}
        }
    }

    /// Load a development cartridge ROM
    pub fn load_dev_cart(&mut self, rom_data: Vec<u8>) {
        self.dev_cart_rom = Some(rom_data);
        self.device = ExpansionDevice::DevelopmentCart;
        info!("Loaded development cartridge ({} bytes)", self.dev_cart_rom.as_ref().unwrap().len());
    }

    /// Add Action Replay / GameShark codes
    pub fn add_cheat_code(&mut self, address: u32, value: u32) {
        self.cheat_codes.insert(address, value);
        if self.device == ExpansionDevice::None {
            self.device = ExpansionDevice::ActionReplay;
        }
    }

    /// Read from expansion port region 1
    pub fn load<T: Addressable>(&self, offset: u32) -> T {
        match self.device {
            ExpansionDevice::None => {
                // No device connected, return 0xFF
                T::from_u32(0xFFFFFFFF)
            }
            ExpansionDevice::ParallelPort => {
                self.load_parallel_port(offset)
            }
            ExpansionDevice::DevelopmentCart => {
                self.read_dev_cart(offset)
            }
            ExpansionDevice::ActionReplay | ExpansionDevice::GameShark => {
                self.load_cheat_device(offset)
            }
            ExpansionDevice::RamExpansion => {
                self.load_ram_expansion(offset)
            }
        }
    }

    /// Write to expansion port region 1
    pub fn store<T: Addressable>(&mut self, offset: u32, value: T) {
        match self.device {
            ExpansionDevice::None => {
                // Ignore writes when no device
            }
            ExpansionDevice::ParallelPort => {
                self.store_parallel_port(offset, value);
            }
            ExpansionDevice::DevelopmentCart => {
                // Dev cart is usually read-only
                warn!("Write to development cartridge at offset 0x{:08x}", offset);
            }
            ExpansionDevice::ActionReplay | ExpansionDevice::GameShark => {
                self.store_cheat_device(offset, value);
            }
            ExpansionDevice::RamExpansion => {
                self.store_ram_expansion(offset, value);
            }
        }
    }

    // Parallel port implementation
    fn load_parallel_port<T: Addressable>(&self, offset: u32) -> T {
        let val = match offset & 0xF {
            0x0 => self.parallel_data as u32,
            0x4 => self.parallel_status as u32,
            0x8 => self.parallel_control as u32,
            _ => {
                warn!("Invalid parallel port read at offset 0x{:x}", offset);
                0xFFFFFFFF
            }
        };
        T::from_u32(val)
    }

    fn store_parallel_port<T: Addressable>(&mut self, offset: u32, value: T) {
        let val = value.as_u32() as u8;
        match offset & 0xF {
            0x0 => {
                self.parallel_data = val;
                // Simulate printer/device receiving data
                debug!("Parallel port data: 0x{:02x}", val);
            }
            0x4 => {
                // Status is mostly read-only
                warn!("Write to parallel port status: 0x{:02x}", val);
            }
            0x8 => {
                self.parallel_control = val;
                // Handle control signals (strobe, init, etc.)
                if val & 0x01 != 0 {
                    debug!("Parallel port strobe signal");
                }
            }
            _ => {
                warn!("Invalid parallel port write at offset 0x{:x}", offset);
            }
        }
    }

    // Development cartridge implementation - read from ROM
    fn read_dev_cart<T: Addressable>(&self, offset: u32) -> T {
        if let Some(ref rom) = self.dev_cart_rom {
            let offset = offset as usize;
            if offset < rom.len() {
                let val = match T::width() {
                    AccessWidth::Byte => rom[offset] as u32,
                    AccessWidth::HalfWord => {
                        let lo = rom[offset] as u32;
                        let hi = rom.get(offset + 1).copied().unwrap_or(0) as u32;
                        lo | (hi << 8)
                    }
                    AccessWidth::Word => {
                        let b0 = rom.get(offset).copied().unwrap_or(0) as u32;
                        let b1 = rom.get(offset + 1).copied().unwrap_or(0) as u32;
                        let b2 = rom.get(offset + 2).copied().unwrap_or(0) as u32;
                        let b3 = rom.get(offset + 3).copied().unwrap_or(0) as u32;
                        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
                    }
                };
                return T::from_u32(val);
            }
        }
        T::from_u32(0xFFFFFFFF)
    }

    // Cheat device implementation
    fn load_cheat_device<T: Addressable>(&self, offset: u32) -> T {
        // Cheat devices typically have their own ROM/RAM
        let offset = offset as usize;
        if offset < self.device_memory.len() {
            let val = match T::width() {
                AccessWidth::Byte => self.device_memory[offset] as u32,
                AccessWidth::HalfWord => {
                    let lo = self.device_memory[offset] as u32;
                    let hi = self.device_memory[offset + 1] as u32;
                    lo | (hi << 8)
                }
                AccessWidth::Word => {
                    let b0 = self.device_memory[offset] as u32;
                    let b1 = self.device_memory[offset + 1] as u32;
                    let b2 = self.device_memory[offset + 2] as u32;
                    let b3 = self.device_memory[offset + 3] as u32;
                    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
                }
            };
            T::from_u32(val)
        } else {
            T::from_u32(0xFFFFFFFF)
        }
    }

    fn store_cheat_device<T: Addressable>(&mut self, offset: u32, value: T) {
        let offset = offset as usize;
        let val = value.as_u32();
        
        if offset < self.device_memory.len() {
            match T::width() {
                AccessWidth::Byte => {
                    self.device_memory[offset] = val as u8;
                }
                AccessWidth::HalfWord => {
                    self.device_memory[offset] = val as u8;
                    if offset + 1 < self.device_memory.len() {
                        self.device_memory[offset + 1] = (val >> 8) as u8;
                    }
                }
                AccessWidth::Word => {
                    self.device_memory[offset] = val as u8;
                    if offset + 1 < self.device_memory.len() {
                        self.device_memory[offset + 1] = (val >> 8) as u8;
                    }
                    if offset + 2 < self.device_memory.len() {
                        self.device_memory[offset + 2] = (val >> 16) as u8;
                    }
                    if offset + 3 < self.device_memory.len() {
                        self.device_memory[offset + 3] = (val >> 24) as u8;
                    }
                }
            }
        }
    }

    // RAM expansion implementation
    fn load_ram_expansion<T: Addressable>(&self, offset: u32) -> T {
        let offset = offset as usize;
        if offset < self.device_memory.len() {
            let val = match T::width() {
                AccessWidth::Byte => self.device_memory[offset] as u32,
                AccessWidth::HalfWord => {
                    let lo = self.device_memory[offset] as u32;
                    let hi = self.device_memory.get(offset + 1).copied().unwrap_or(0) as u32;
                    lo | (hi << 8)
                }
                AccessWidth::Word => {
                    let b0 = self.device_memory.get(offset).copied().unwrap_or(0) as u32;
                    let b1 = self.device_memory.get(offset + 1).copied().unwrap_or(0) as u32;
                    let b2 = self.device_memory.get(offset + 2).copied().unwrap_or(0) as u32;
                    let b3 = self.device_memory.get(offset + 3).copied().unwrap_or(0) as u32;
                    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
                }
            };
            T::from_u32(val)
        } else {
            T::from_u32(0)
        }
    }

    fn store_ram_expansion<T: Addressable>(&mut self, offset: u32, value: T) {
        let offset = offset as usize;
        let val = value.as_u32();
        
        if offset < self.device_memory.len() {
            match T::width() {
                AccessWidth::Byte => {
                    self.device_memory[offset] = val as u8;
                }
                AccessWidth::HalfWord => {
                    self.device_memory[offset] = val as u8;
                    if offset + 1 < self.device_memory.len() {
                        self.device_memory[offset + 1] = (val >> 8) as u8;
                    }
                }
                AccessWidth::Word => {
                    for i in 0..4 {
                        if offset + i < self.device_memory.len() {
                            self.device_memory[offset + i] = (val >> (i * 8)) as u8;
                        }
                    }
                }
            }
        }
    }

    /// Apply cheat codes to memory
    pub fn apply_cheats(&self, psx: &mut Psx) {
        for (&address, &value) in &self.cheat_codes {
            // Apply cheat code to main RAM
            if address < 0x200000 {
                // Write to main RAM through proper channels
                debug!("Applying cheat: {:08x} = {:08x}", address, value);
            }
        }
    }
}