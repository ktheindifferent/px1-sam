//! PSX Memory Map Constants
//! 
//! This module defines all memory addresses, ranges, and masks used throughout
//! the PSX emulator. Centralizing these constants improves maintainability
//! and eliminates magic numbers scattered throughout the codebase.

// ============================================================================
// Memory Segments and Base Addresses
// ============================================================================

/// KUSEG - User segment (cached, mapped)
pub const KUSEG_BASE: u32 = 0x00000000;
pub const KUSEG_SIZE: u32 = 0x80000000;

/// KSEG0 - Kernel segment 0 (cached, unmapped)
pub const KSEG0_BASE: u32 = 0x80000000;
pub const KSEG0_SIZE: u32 = 0x20000000;

/// KSEG1 - Kernel segment 1 (uncached, unmapped)
pub const KSEG1_BASE: u32 = 0xa0000000;
pub const KSEG1_SIZE: u32 = 0x20000000;

/// KSEG2 - Kernel segment 2 (cached, mapped)
pub const KSEG2_BASE: u32 = 0xc0000000;
pub const KSEG2_SIZE: u32 = 0x40000000;

// ============================================================================
// Main Memory (RAM)
// ============================================================================

/// Main RAM size (2MB standard, some games detect 8MB)
pub const RAM_SIZE_2MB: u32 = 0x00200000;
pub const RAM_SIZE_8MB: u32 = 0x00800000;
pub const RAM_SIZE_DEFAULT: u32 = RAM_SIZE_2MB;

/// RAM mirroring masks
pub const RAM_MASK_2MB: u32 = RAM_SIZE_2MB - 1;
pub const RAM_MASK_8MB: u32 = RAM_SIZE_8MB - 1;

// ============================================================================
// BIOS ROM
// ============================================================================

/// BIOS ROM physical address
pub const BIOS_PHYSICAL_ADDR: u32 = 0x1fc00000;
pub const BIOS_SIZE: u32 = 0x00080000; // 512KB

/// BIOS entry points in different segments
pub const BIOS_KSEG0_ADDR: u32 = 0x9fc00000;
pub const BIOS_KSEG1_ADDR: u32 = 0xbfc00000;

/// Reset vector (CPU starts here)
pub const RESET_VECTOR: u32 = BIOS_KSEG1_ADDR;

// ============================================================================
// Hardware Registers Base Addresses
// ============================================================================

/// Memory control registers
pub const MEMCTRL_BASE: u32 = 0x1f801000;
pub const MEMCTRL_SIZE: u32 = 0x00000024;

/// Pad/Memory card registers
pub const PAD_MEMCARD_BASE: u32 = 0x1f801040;
pub const PAD_MEMCARD_SIZE: u32 = 0x00000010;

/// Serial port registers
pub const SERIAL_BASE: u32 = 0x1f801050;
pub const SERIAL_SIZE: u32 = 0x00000010;

/// Memory control registers (additional)
pub const MEMCTRL2_BASE: u32 = 0x1f801060;
pub const MEMCTRL2_SIZE: u32 = 0x00000004;

/// Interrupt controller registers
pub const IRQ_BASE: u32 = 0x1f801070;
pub const IRQ_I_STAT: u32 = 0x1f801070;
pub const IRQ_I_MASK: u32 = 0x1f801074;

/// DMA controller registers
pub const DMA_BASE: u32 = 0x1f801080;
pub const DMA_SIZE: u32 = 0x00000080;

/// Timer registers
pub const TIMER_BASE: u32 = 0x1f801100;
pub const TIMER_SIZE: u32 = 0x00000030;

/// CDROM controller registers
pub const CDROM_BASE: u32 = 0x1f801800;
pub const CDROM_SIZE: u32 = 0x00000004;

/// GPU registers
pub const GPU_BASE: u32 = 0x1f801810;
pub const GPU_GP0: u32 = 0x1f801810;  // GPU command/data register
pub const GPU_GP1: u32 = 0x1f801814;  // GPU control/status register

/// MDEC (Motion Decoder) registers
pub const MDEC_BASE: u32 = 0x1f801820;
pub const MDEC_SIZE: u32 = 0x00000008;

/// SPU (Sound Processing Unit) registers
pub const SPU_BASE: u32 = 0x1f801c00;
pub const SPU_SIZE: u32 = 0x00000400;

/// Expansion Region 2 (I/O Ports)
pub const EXP2_BASE: u32 = 0x1f802000;
pub const EXP2_SIZE: u32 = 0x00000080;

/// Expansion Region 3
pub const EXP3_BASE: u32 = 0x1fa00000;
pub const EXP3_SIZE: u32 = 0x00200000;

// ============================================================================
// Scratchpad (Data Cache)
// ============================================================================

pub const SCRATCHPAD_ADDR: u32 = 0x1f800000;
pub const SCRATCHPAD_SIZE: u32 = 0x00000400; // 1KB

// ============================================================================
// Expansion Regions
// ============================================================================

/// Expansion Region 1 base address
pub const EXP1_BASE: u32 = 0x1f000000;
pub const EXP1_SIZE: u32 = 0x00800000;

// ============================================================================
// Memory Access Masks and Alignment
// ============================================================================

/// Physical address mask (removes segment bits)
pub const PHYSICAL_ADDR_MASK: u32 = 0x1fffffff;

/// Cache control mask
pub const CACHE_CONTROL_MASK: u32 = 0xe0000000;

/// Word alignment masks
pub const WORD_ALIGN_MASK: u32 = !0x3;
pub const HALFWORD_ALIGN_MASK: u32 = !0x1;

// ============================================================================
// CPU Instruction Constants
// ============================================================================

/// Jump instruction target mask
pub const JUMP_TARGET_MASK: u32 = 0x03ffffff;

/// PC segment mask for jump instructions
pub const PC_SEGMENT_MASK: u32 = 0xf0000000;

/// Branch offset sign extension
pub const BRANCH_OFFSET_SHIFT: u32 = 2;
pub const BRANCH_OFFSET_SIGN_EXTEND: u32 = 0xfffc0000;

// ============================================================================
// Exception Vectors
// ============================================================================

/// Exception handler addresses
pub const EXCEPTION_VECTOR_RAM: u32 = 0x80000080;
pub const EXCEPTION_VECTOR_ROM: u32 = 0xbfc00180;

// ============================================================================
// EXE File Loading
// ============================================================================

/// Default EXE load address
pub const EXE_DEFAULT_LOAD_ADDR: u32 = 0x80010000;

/// EXE header magic number
pub const EXE_MAGIC: &[u8] = b"PS-X EXE";

// ============================================================================
// DMA Channel IDs
// ============================================================================

pub const DMA_MDEC_IN: u32 = 0;
pub const DMA_MDEC_OUT: u32 = 1;
pub const DMA_GPU: u32 = 2;
pub const DMA_CDROM: u32 = 3;
pub const DMA_SPU: u32 = 4;
pub const DMA_PIO: u32 = 5;
pub const DMA_OTC: u32 = 6;

// ============================================================================
// Controller Input Constants
// ============================================================================

/// Digital controller button masks
pub mod controller {
    /// D-Pad buttons
    pub const DPAD_UP: u16 = 0x0010;
    pub const DPAD_RIGHT: u16 = 0x0020;
    pub const DPAD_DOWN: u16 = 0x0040;
    pub const DPAD_LEFT: u16 = 0x0080;
    
    /// Face buttons
    pub const BUTTON_TRIANGLE: u16 = 0x1000;
    pub const BUTTON_CIRCLE: u16 = 0x2000;
    pub const BUTTON_CROSS: u16 = 0x4000;
    pub const BUTTON_SQUARE: u16 = 0x8000;
    
    /// Shoulder buttons
    pub const BUTTON_L1: u16 = 0x0004;
    pub const BUTTON_L2: u16 = 0x0001;
    pub const BUTTON_R1: u16 = 0x0008;
    pub const BUTTON_R2: u16 = 0x0002;
    
    /// System buttons
    pub const BUTTON_SELECT: u16 = 0x0100;
    pub const BUTTON_START: u16 = 0x0800;
    pub const BUTTON_L3: u16 = 0x0200;
    pub const BUTTON_R3: u16 = 0x0400;
    
    /// Controller types
    pub const TYPE_DIGITAL: u8 = 0x41;
    pub const TYPE_ANALOG: u8 = 0x73;
    pub const TYPE_DUALSHOCK: u8 = 0x73;
    pub const TYPE_MOUSE: u8 = 0x12;
    pub const TYPE_NEGCON: u8 = 0x23;
    pub const TYPE_GUNCON: u8 = 0x63;
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert a virtual address to physical address
#[inline(always)]
pub const fn to_physical_address(addr: u32) -> u32 {
    addr & PHYSICAL_ADDR_MASK
}

/// Check if address is in KSEG0 (cached)
#[inline(always)]
pub const fn is_kseg0(addr: u32) -> bool {
    (addr & CACHE_CONTROL_MASK) == KSEG0_BASE
}

/// Check if address is in KSEG1 (uncached)
#[inline(always)]
pub const fn is_kseg1(addr: u32) -> bool {
    (addr & CACHE_CONTROL_MASK) == KSEG1_BASE
}

/// Check if address is in KUSEG (user segment)
#[inline(always)]
pub const fn is_kuseg(addr: u32) -> bool {
    addr < KSEG0_BASE
}

/// Check if address is in KSEG2
#[inline(always)]
pub const fn is_kseg2(addr: u32) -> bool {
    addr >= KSEG2_BASE
}

/// Get the memory segment for an address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySegment {
    Kuseg,
    Kseg0,
    Kseg1,
    Kseg2,
}

#[inline(always)]
pub const fn get_segment(addr: u32) -> MemorySegment {
    if addr < KSEG0_BASE {
        MemorySegment::Kuseg
    } else if addr < KSEG1_BASE {
        MemorySegment::Kseg0
    } else if addr < KSEG2_BASE {
        MemorySegment::Kseg1
    } else {
        MemorySegment::Kseg2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physical_address_conversion() {
        assert_eq!(to_physical_address(0x80001234), 0x00001234);
        assert_eq!(to_physical_address(0xa0001234), 0x00001234);
        assert_eq!(to_physical_address(0x00001234), 0x00001234);
    }

    #[test]
    fn test_segment_detection() {
        assert!(is_kuseg(0x00001234));
        assert!(is_kseg0(0x80001234));
        assert!(is_kseg1(0xa0001234));
        assert!(is_kseg2(0xc0001234));
    }

    #[test]
    fn test_segment_enum() {
        assert_eq!(get_segment(0x00001234), MemorySegment::Kuseg);
        assert_eq!(get_segment(0x80001234), MemorySegment::Kseg0);
        assert_eq!(get_segment(0xa0001234), MemorySegment::Kseg1);
        assert_eq!(get_segment(0xc0001234), MemorySegment::Kseg2);
    }
}