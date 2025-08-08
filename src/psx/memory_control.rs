//! PlayStation Memory Control and Timing Implementation
//!
//! Manages memory timing parameters, wait states, and memory configuration

use super::{AccessWidth, Addressable, CycleCount};

/// Memory region timing parameters
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MemoryTiming {
    /// First access time (in CPU cycles)
    pub first_access_time: u8,
    /// Sequential access time (for burst reads)
    pub sequential_access_time: u8,
    /// Minimum cycles between accesses
    pub min_access_delay: u8,
    /// Recovery time after access
    pub recovery_time: u8,
    /// Hold time for data
    pub hold_time: u8,
    /// Float time (bus release)
    pub float_time: u8,
    /// Pre-strobe time
    pub pre_strobe: u8,
    /// Strobe width
    pub strobe_width: u8,
    /// Use COM delays
    pub use_com_delays: bool,
    /// Address/data bus width (8/16/32 bits)
    pub bus_width: BusWidth,
    /// Auto-increment for sequential access
    pub auto_increment: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BusWidth {
    Width8,
    Width16,
    Width32,
}

impl MemoryTiming {
    pub fn new() -> Self {
        MemoryTiming {
            first_access_time: 4,
            sequential_access_time: 2,
            min_access_delay: 1,
            recovery_time: 1,
            hold_time: 1,
            float_time: 1,
            pre_strobe: 1,
            strobe_width: 1,
            use_com_delays: false,
            bus_width: BusWidth::Width32,
            auto_increment: false,
        }
    }

    /// Parse from control register value
    pub fn from_u32(value: u32) -> Self {
        MemoryTiming {
            first_access_time: ((value >> 0) & 0x1F) as u8,
            sequential_access_time: ((value >> 5) & 0x1F) as u8,
            min_access_delay: ((value >> 10) & 0x07) as u8,
            recovery_time: ((value >> 13) & 0x03) as u8,
            hold_time: ((value >> 16) & 0x03) as u8,
            float_time: ((value >> 18) & 0x03) as u8,
            pre_strobe: ((value >> 20) & 0x01) as u8,
            strobe_width: ((value >> 21) & 0x03) as u8,
            use_com_delays: (value & 0x01000000) != 0,
            bus_width: match (value >> 25) & 0x03 {
                0 => BusWidth::Width8,
                1 => BusWidth::Width16,
                _ => BusWidth::Width32,
            },
            auto_increment: (value & 0x10000000) != 0,
        }
    }

    /// Convert to control register value
    pub fn to_u32(&self) -> u32 {
        let mut value = 0u32;
        
        value |= (self.first_access_time as u32) & 0x1F;
        value |= ((self.sequential_access_time as u32) & 0x1F) << 5;
        value |= ((self.min_access_delay as u32) & 0x07) << 10;
        value |= ((self.recovery_time as u32) & 0x03) << 13;
        value |= ((self.hold_time as u32) & 0x03) << 16;
        value |= ((self.float_time as u32) & 0x03) << 18;
        value |= ((self.pre_strobe as u32) & 0x01) << 20;
        value |= ((self.strobe_width as u32) & 0x03) << 21;
        
        if self.use_com_delays {
            value |= 0x01000000;
        }
        
        let width_bits = match self.bus_width {
            BusWidth::Width8 => 0,
            BusWidth::Width16 => 1,
            BusWidth::Width32 => 2,
        };
        value |= width_bits << 25;
        
        if self.auto_increment {
            value |= 0x10000000;
        }
        
        value
    }

    /// Calculate actual access time in cycles
    pub fn calculate_access_time(&self, sequential: bool) -> CycleCount {
        let base_time = if sequential {
            self.sequential_access_time
        } else {
            self.first_access_time
        };
        
        // Add delays
        let mut total = base_time as CycleCount;
        total += self.recovery_time as CycleCount;
        
        if self.use_com_delays {
            total += self.pre_strobe as CycleCount;
            total += self.strobe_width as CycleCount;
            total += self.hold_time as CycleCount;
            total += self.float_time as CycleCount;
        }
        
        total.max(self.min_access_delay as CycleCount)
    }
}

/// Memory control system
#[derive(serde::Serialize, serde::Deserialize)]
pub struct MemoryControl {
    /// Expansion region 1 base address
    pub exp1_base: u32,
    /// Expansion region 2 base address
    pub exp2_base: u32,
    /// Expansion region 3 base address
    pub exp3_base: u32,
    /// Expansion region 1 delay/size
    pub exp1_config: MemoryTiming,
    /// Expansion region 2 delay/size
    pub exp2_config: MemoryTiming,
    /// Expansion region 3 delay/size
    pub exp3_config: MemoryTiming,
    /// BIOS ROM delay/size
    pub bios_config: MemoryTiming,
    /// SPU delay
    pub spu_config: MemoryTiming,
    /// CDROM delay
    pub cdrom_config: MemoryTiming,
    /// Common delay
    pub common_delay: u32,
    /// RAM size configuration
    pub ram_size: RamSize,
    /// Memory control registers raw values
    registers: [u32; 9],
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum RamSize {
    /// 2MB RAM (standard)
    Ram2MB,
    /// 8MB RAM (development kit)
    Ram8MB,
}

impl MemoryControl {
    pub fn new() -> Self {
        // Default memory timings (from BIOS initialization)
        let mut ctrl = MemoryControl {
            exp1_base: 0x1F000000,
            exp2_base: 0x1F802000,
            exp3_base: 0x1FA00000,
            exp1_config: MemoryTiming::from_u32(0x0013243F),
            exp2_config: MemoryTiming::from_u32(0x00003022),
            exp3_config: MemoryTiming::from_u32(0x0013243F),
            bios_config: MemoryTiming::from_u32(0x0013243F),
            spu_config: MemoryTiming::from_u32(0x200931E1),
            cdrom_config: MemoryTiming::from_u32(0x00020843),
            common_delay: 0x00031125,
            ram_size: RamSize::Ram2MB,
            registers: [0; 9],
        };
        
        // Initialize registers with default values
        ctrl.registers[0] = ctrl.exp1_base;
        ctrl.registers[1] = ctrl.exp2_base;
        ctrl.registers[2] = ctrl.exp1_config.to_u32();
        ctrl.registers[3] = ctrl.exp3_config.to_u32();
        ctrl.registers[4] = ctrl.bios_config.to_u32();
        ctrl.registers[5] = ctrl.spu_config.to_u32();
        ctrl.registers[6] = ctrl.cdrom_config.to_u32();
        ctrl.registers[7] = ctrl.exp2_config.to_u32();
        ctrl.registers[8] = ctrl.common_delay;
        
        ctrl
    }

    /// Read memory control register
    pub fn read(&self, index: usize) -> u32 {
        if index < 9 {
            self.registers[index]
        } else {
            warn!("Invalid memory control register read: {}", index);
            0xFFFFFFFF
        }
    }

    /// Write memory control register
    pub fn write(&mut self, index: usize, value: u32) {
        if index >= 9 {
            warn!("Invalid memory control register write: {} = 0x{:08x}", index, value);
            return;
        }

        self.registers[index] = value;

        // Update configuration based on register
        match index {
            0 => self.exp1_base = value & 0xFFFFFF00,
            1 => self.exp2_base = value & 0xFFFFFF00,
            2 => self.exp1_config = MemoryTiming::from_u32(value),
            3 => self.exp3_config = MemoryTiming::from_u32(value),
            4 => self.bios_config = MemoryTiming::from_u32(value),
            5 => self.spu_config = MemoryTiming::from_u32(value),
            6 => self.cdrom_config = MemoryTiming::from_u32(value),
            7 => self.exp2_config = MemoryTiming::from_u32(value),
            8 => self.common_delay = value,
            _ => {}
        }

        debug!("Memory control[{}] = 0x{:08x}", index, value);
    }

    /// Get timing for a memory region
    pub fn get_timing(&self, address: u32) -> MemoryTiming {
        match address >> 24 {
            0x00..=0x7F => {
                // Main RAM - fastest timing
                let mut timing = MemoryTiming::new();
                timing.first_access_time = 1;
                timing.sequential_access_time = 1;
                timing
            }
            0x1F if (address & 0x00F00000) == 0x00000000 => {
                // Expansion region 1
                self.exp1_config
            }
            0x1F if (address & 0x00F00000) == 0x00800000 => {
                // Expansion region 2
                self.exp2_config
            }
            0x1F if (address & 0x00F00000) == 0x00A00000 => {
                // Expansion region 3
                self.exp3_config
            }
            0xBF => {
                // BIOS ROM
                self.bios_config
            }
            _ => {
                // Default timing
                MemoryTiming::new()
            }
        }
    }

    /// Calculate memory access penalty cycles
    pub fn calculate_access_penalty(&self, address: u32, width: AccessWidth, sequential: bool) -> CycleCount {
        let timing = self.get_timing(address);
        let mut penalty = timing.calculate_access_time(sequential);

        // Adjust for bus width mismatches
        match (timing.bus_width, width) {
            (BusWidth::Width8, AccessWidth::HalfWord) => penalty *= 2,
            (BusWidth::Width8, AccessWidth::Word) => penalty *= 4,
            (BusWidth::Width16, AccessWidth::Word) => penalty *= 2,
            _ => {}
        }

        penalty
    }

    /// Set RAM size configuration
    pub fn set_ram_size(&mut self, value: u32) {
        // Bit 9-11 indicate RAM size
        self.ram_size = match (value >> 9) & 0x7 {
            0x7 => RamSize::Ram8MB,
            _ => RamSize::Ram2MB,
        };

        debug!("RAM size set to {:?}", self.ram_size);
    }

    /// Get actual RAM size in bytes
    pub fn ram_size_bytes(&self) -> usize {
        match self.ram_size {
            RamSize::Ram2MB => 2 * 1024 * 1024,
            RamSize::Ram8MB => 8 * 1024 * 1024,
        }
    }
}