//! PlayStation CPU Cache Implementation
//! 
//! The R3000A has 4KB instruction cache and a scratchpad RAM area.
//! This module provides full cache control functionality.

use super::{AccessWidth, Addressable, Psx};

/// Cache control register bits
#[derive(Debug, Clone, Copy)]
pub struct CacheControl {
    /// Enable instruction cache
    pub icache_enable: bool,
    /// Enable data cache/scratchpad
    pub dcache_enable: bool,
    /// Tag test mode (for cache diagnostics)
    pub tag_test_mode: bool,
    /// Invalidate instruction cache
    pub invalidate_icache: bool,
    /// Load instruction cache
    pub load_icache: bool,
    /// Lock cache contents
    pub cache_lock: bool,
    /// Cache isolation mode
    pub cache_isolated: bool,
    /// Scratchpad enable
    pub scratchpad_enable: bool,
    /// Cache write-through mode
    pub write_through: bool,
    /// Cache burst mode
    pub burst_mode: bool,
    /// Raw register value
    raw: u32,
}

impl CacheControl {
    pub fn new() -> Self {
        CacheControl {
            icache_enable: false,
            dcache_enable: false,
            tag_test_mode: false,
            invalidate_icache: false,
            load_icache: false,
            cache_lock: false,
            cache_isolated: false,
            scratchpad_enable: true, // Usually enabled
            write_through: false,
            burst_mode: false,
            raw: 0,
        }
    }

    /// Parse control register value
    pub fn from_u32(value: u32) -> Self {
        CacheControl {
            icache_enable: (value & 0x0001) != 0,
            dcache_enable: (value & 0x0002) != 0,
            tag_test_mode: (value & 0x0004) != 0,
            invalidate_icache: (value & 0x0008) != 0,
            load_icache: (value & 0x0010) != 0,
            cache_lock: (value & 0x0020) != 0,
            cache_isolated: (value & 0x0040) != 0,
            scratchpad_enable: (value & 0x0080) != 0,
            write_through: (value & 0x0100) != 0,
            burst_mode: (value & 0x0200) != 0,
            raw: value,
        }
    }

    /// Convert to register value
    pub fn to_u32(&self) -> u32 {
        let mut value = 0u32;
        
        if self.icache_enable { value |= 0x0001; }
        if self.dcache_enable { value |= 0x0002; }
        if self.tag_test_mode { value |= 0x0004; }
        if self.invalidate_icache { value |= 0x0008; }
        if self.load_icache { value |= 0x0010; }
        if self.cache_lock { value |= 0x0020; }
        if self.cache_isolated { value |= 0x0040; }
        if self.scratchpad_enable { value |= 0x0080; }
        if self.write_through { value |= 0x0100; }
        if self.burst_mode { value |= 0x0200; }
        
        value | (self.raw & 0xFFFFFC00) // Preserve unknown bits
    }
}

/// Cache line structure
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct CacheLine {
    /// Tag address (physical address bits 31:12)
    pub tag: u32,
    /// Valid bit
    pub valid: bool,
    /// Dirty bit (for write-back cache)
    pub dirty: bool,
    /// LRU counter for replacement
    pub lru: u8,
    /// Cache line data (4 words = 16 bytes)
    pub data: [u32; 4],
}

impl CacheLine {
    pub fn new() -> Self {
        CacheLine {
            tag: 0,
            valid: false,
            dirty: false,
            lru: 0,
            data: [0; 4],
        }
    }

    /// Check if address matches this cache line
    pub fn matches(&self, addr: u32) -> bool {
        if !self.valid {
            return false;
        }
        
        // Compare tag (bits 31:12)
        let addr_tag = addr >> 12;
        self.tag == addr_tag
    }

    /// Invalidate the cache line
    pub fn invalidate(&mut self) {
        self.valid = false;
        self.dirty = false;
    }
}

/// Instruction cache implementation
#[derive(serde::Serialize, serde::Deserialize)]
pub struct InstructionCache {
    /// 256 cache lines of 16 bytes each = 4KB
    lines: [CacheLine; 256],
    /// Cache hit counter for statistics
    hits: u64,
    /// Cache miss counter
    misses: u64,
    /// Whether cache is enabled
    enabled: bool,
    /// Isolation mode (cache doesn't refill from memory)
    isolated: bool,
}

impl InstructionCache {
    pub fn new() -> Self {
        InstructionCache {
            lines: [CacheLine::new(); 256],
            hits: 0,
            misses: 0,
            enabled: true,
            isolated: false,
        }
    }

    /// Get cache line index for address
    fn line_index(addr: u32) -> usize {
        // Use bits 11:4 as line index (256 lines)
        ((addr >> 4) & 0xFF) as usize
    }

    /// Word offset within cache line
    fn word_offset(addr: u32) -> usize {
        // Use bits 3:2 as word offset (4 words per line)
        ((addr >> 2) & 0x3) as usize
    }

    /// Read instruction from cache
    pub fn read(&mut self, addr: u32) -> Option<u32> {
        if !self.enabled {
            return None;
        }

        let index = Self::line_index(addr);
        let offset = Self::word_offset(addr);
        let line = &mut self.lines[index];

        if line.matches(addr) {
            self.hits += 1;
            line.lru = 0; // Reset LRU
            Some(line.data[offset])
        } else {
            self.misses += 1;
            None
        }
    }

    /// Fill cache line from memory
    pub fn fill(&mut self, addr: u32, data: [u32; 4]) {
        if !self.enabled || self.isolated {
            return;
        }

        let index = Self::line_index(addr);
        let line = &mut self.lines[index];

        line.tag = addr >> 12;
        line.valid = true;
        line.dirty = false;
        line.lru = 0;
        line.data = data;
    }

    /// Invalidate entire cache
    pub fn invalidate(&mut self) {
        for line in &mut self.lines {
            line.invalidate();
        }
        debug!("I-cache invalidated (hits: {}, misses: {})", self.hits, self.misses);
    }

    /// Set cache enable state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.invalidate();
        }
    }

    /// Set isolation mode
    pub fn set_isolated(&mut self, isolated: bool) {
        self.isolated = isolated;
    }

    /// Get cache statistics
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

/// Data cache / Scratchpad RAM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DataCache {
    /// 1KB scratchpad RAM
    scratchpad: [u8; 1024],
    /// Whether scratchpad is enabled
    enabled: bool,
}

impl DataCache {
    pub fn new() -> Self {
        DataCache {
            scratchpad: [0; 1024],
            enabled: true,
        }
    }

    /// Read from scratchpad
    pub fn read<T: Addressable>(&self, offset: u32) -> T {
        if !self.enabled {
            return T::from_u32(0xFFFFFFFF);
        }

        let offset = (offset & 0x3FF) as usize; // 1KB range
        
        let value = match T::width() {
            AccessWidth::Byte => {
                self.scratchpad[offset] as u32
            }
            AccessWidth::HalfWord => {
                let lo = self.scratchpad[offset] as u32;
                let hi = self.scratchpad.get(offset + 1).copied().unwrap_or(0) as u32;
                lo | (hi << 8)
            }
            AccessWidth::Word => {
                let b0 = self.scratchpad.get(offset).copied().unwrap_or(0) as u32;
                let b1 = self.scratchpad.get(offset + 1).copied().unwrap_or(0) as u32;
                let b2 = self.scratchpad.get(offset + 2).copied().unwrap_or(0) as u32;
                let b3 = self.scratchpad.get(offset + 3).copied().unwrap_or(0) as u32;
                b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
            }
        };

        T::from_u32(value)
    }

    /// Write to scratchpad
    pub fn write<T: Addressable>(&mut self, offset: u32, value: T) {
        if !self.enabled {
            return;
        }

        let offset = (offset & 0x3FF) as usize; // 1KB range
        let val = value.as_u32();

        match T::width() {
            AccessWidth::Byte => {
                self.scratchpad[offset] = val as u8;
            }
            AccessWidth::HalfWord => {
                self.scratchpad[offset] = val as u8;
                if offset + 1 < 1024 {
                    self.scratchpad[offset + 1] = (val >> 8) as u8;
                }
            }
            AccessWidth::Word => {
                for i in 0..4 {
                    if offset + i < 1024 {
                        self.scratchpad[offset + i] = (val >> (i * 8)) as u8;
                    }
                }
            }
        }
    }

    /// Set enable state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// Complete cache system
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CacheSystem {
    /// Instruction cache
    pub icache: InstructionCache,
    /// Data cache/scratchpad
    pub dcache: DataCache,
    /// Cache control register
    pub control: CacheControl,
}

impl CacheSystem {
    pub fn new() -> Self {
        CacheSystem {
            icache: InstructionCache::new(),
            dcache: DataCache::new(),
            control: CacheControl::new(),
        }
    }

    /// Update cache control
    pub fn set_control(&mut self, value: u32) {
        let new_control = CacheControl::from_u32(value);
        
        // Handle cache operations
        if new_control.invalidate_icache && !self.control.invalidate_icache {
            self.icache.invalidate();
        }
        
        // Update enable states
        self.icache.set_enabled(new_control.icache_enable);
        self.icache.set_isolated(new_control.cache_isolated);
        self.dcache.set_enabled(new_control.scratchpad_enable);
        
        self.control = new_control;
    }

    /// Get cache control value
    pub fn control(&self) -> u32 {
        self.control.to_u32()
    }

    /// Perform cache tag test
    pub fn tag_test(&self, addr: u32) -> u32 {
        if !self.control.tag_test_mode {
            return 0;
        }

        let index = InstructionCache::line_index(addr);
        let line = &self.icache.lines[index];
        
        // Return tag and valid bit
        let mut result = line.tag << 12;
        if line.valid {
            result |= 0x1;
        }
        result
    }
}