// Simplified PSX emulator stub for WASM build
use super::error::{PsxError, Result};

// CPU stub
pub struct Cpu {
    pub pc: u32,
    pub regs: [u32; 32],
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            pc: 0xbfc00000,
            regs: [0; 32],
        }
    }
    
    pub fn step(&mut self) -> Result<()> {
        // Basic CPU step simulation
        self.pc += 4;
        Ok(())
    }
}

// GPU stub
pub struct Gpu {
    pub vram: Vec<u16>,
    pub display_width: u32,
    pub display_height: u32,
}

impl Gpu {
    pub fn new() -> Self {
        Gpu {
            vram: vec![0; 1024 * 512],
            display_width: 640,
            display_height: 480,
        }
    }
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> T {
        T::from_u32(0)
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) {
        // Stub implementation
    }
}

// SPU (Sound Processing Unit) stub
pub struct Spu {
    audio_buffer: Vec<f32>,
}

impl Spu {
    pub fn new() -> Self {
        Spu {
            audio_buffer: Vec::new(),
        }
    }
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> T {
        T::from_u32(0)
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) {
        // Stub implementation
    }
}

// DMA stub
pub struct Dma {
    channels: [DmaChannel; 7],
}

impl Dma {
    pub fn new() -> Self {
        Dma {
            channels: [DmaChannel::new(); 7],
        }
    }
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> T {
        T::from_u32(0)
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) {
        // Stub implementation
    }
}

#[derive(Clone, Copy)]
struct DmaChannel {
    enabled: bool,
}

impl DmaChannel {
    fn new() -> Self {
        DmaChannel { enabled: false }
    }
}

// Timers stub
pub struct Timers {
    timers: [Timer; 3],
}

impl Timers {
    pub fn new() -> Self {
        Timers {
            timers: [Timer::new(); 3],
        }
    }
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> T {
        T::from_u32(0)
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) {
        // Stub implementation
    }
}

#[derive(Clone, Copy)]
struct Timer {
    counter: u16,
}

impl Timer {
    fn new() -> Self {
        Timer { counter: 0 }
    }
}

// IRQ (Interrupt Request) stub
pub struct Irq {
    status: u32,
    mask: u32,
}

impl Irq {
    pub fn new() -> Self {
        Irq {
            status: 0,
            mask: 0,
        }
    }
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> T {
        T::from_u32(0)
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) {
        // Stub implementation
    }
}

// Pad/Memory Card stub
pub struct PadMemCard {
    controller_state: [u16; 2],
}

impl PadMemCard {
    pub fn new() -> Self {
        PadMemCard {
            controller_state: [0; 2],
        }
    }
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> T {
        T::from_u32(0)
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) {
        // Stub implementation
    }
}

// Memory access trait
pub trait AccessWidth {
    fn from_u32(val: u32) -> Self;
    fn to_u32(&self) -> u32;
    fn store(&self, buf: &mut [u8]);
}

impl AccessWidth for u8 {
    fn from_u32(val: u32) -> Self {
        val as u8
    }
    
    fn to_u32(&self) -> u32 {
        *self as u32
    }
    
    fn store(&self, buf: &mut [u8]) {
        if !buf.is_empty() {
            buf[0] = *self;
        }
    }
}

impl AccessWidth for u16 {
    fn from_u32(val: u32) -> Self {
        val as u16
    }
    
    fn to_u32(&self) -> u32 {
        *self as u32
    }
    
    fn store(&self, buf: &mut [u8]) {
        if buf.len() >= 2 {
            buf[0] = (*self & 0xff) as u8;
            buf[1] = ((*self >> 8) & 0xff) as u8;
        }
    }
}

impl AccessWidth for u32 {
    fn from_u32(val: u32) -> Self {
        val
    }
    
    fn to_u32(&self) -> u32 {
        *self
    }
    
    fn store(&self, buf: &mut [u8]) {
        if buf.len() >= 4 {
            buf[0] = (*self & 0xff) as u8;
            buf[1] = ((*self >> 8) & 0xff) as u8;
            buf[2] = ((*self >> 16) & 0xff) as u8;
            buf[3] = ((*self >> 24) & 0xff) as u8;
        }
    }
}

// Memory management
pub struct XMem {
    ram: Vec<u8>,
}

impl XMem {
    pub fn new() -> Self {
        XMem {
            ram: vec![0; 2 * 1024 * 1024], // 2MB RAM
        }
    }
    
    pub fn ram(&self) -> &[u8] {
        &self.ram
    }
    
    pub fn ram_mut(&mut self) -> &mut [u8] {
        &mut self.ram
    }
}

// Main PSX structure
pub struct Psx {
    pub cpu: Cpu,
    pub gpu: Gpu,
    pub spu: Spu,
    pub dma: Dma,
    pub timers: Timers,
    pub irq: Irq,
    pub pad_memcard: PadMemCard,
    pub xmem: XMem,
    pub mem_control: [u32; 9],
    pub bios: Vec<u8>,
    pub display_width: u32,
    pub display_height: u32,
}

impl Psx {
    pub fn new() -> Result<Self> {
        Ok(Psx {
            cpu: Cpu::new(),
            gpu: Gpu::new(),
            spu: Spu::new(),
            dma: Dma::new(),
            timers: Timers::new(),
            irq: Irq::new(),
            pad_memcard: PadMemCard::new(),
            xmem: XMem::new(),
            mem_control: [0; 9],
            bios: vec![0; 512 * 1024], // 512KB BIOS
            display_width: 640,
            display_height: 480,
        })
    }
    
    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<()> {
        if bios_data.len() != 512 * 1024 {
            return Err(PsxError::InvalidBios);
        }
        self.bios.copy_from_slice(bios_data);
        Ok(())
    }
    
    pub fn load_exe(&mut self, exe_data: &[u8]) -> Result<()> {
        // Parse PSX-EXE header
        if exe_data.len() < 0x800 {
            return Err(PsxError::InvalidExe);
        }
        
        // Check magic
        if &exe_data[0..8] != b"PS-X EXE" {
            return Err(PsxError::InvalidExe);
        }
        
        // Read header fields (little-endian)
        let pc = u32::from_le_bytes([exe_data[0x10], exe_data[0x11], exe_data[0x12], exe_data[0x13]]);
        let gp = u32::from_le_bytes([exe_data[0x14], exe_data[0x15], exe_data[0x16], exe_data[0x17]]);
        let dest = u32::from_le_bytes([exe_data[0x18], exe_data[0x19], exe_data[0x1a], exe_data[0x1b]]);
        let size = u32::from_le_bytes([exe_data[0x1c], exe_data[0x1d], exe_data[0x1e], exe_data[0x1f]]);
        
        // Copy executable to RAM
        let dest_offset = (dest & 0x1fffff) as usize;
        let exe_size = size.min((exe_data.len() - 0x800) as u32) as usize;
        
        if dest_offset + exe_size <= self.xmem.ram.len() {
            self.xmem.ram[dest_offset..dest_offset + exe_size]
                .copy_from_slice(&exe_data[0x800..0x800 + exe_size]);
        }
        
        // Set PC and GP
        self.cpu.pc = pc;
        self.cpu.regs[28] = gp; // GP register
        
        Ok(())
    }
    
    pub fn init_with_disc(&mut self) -> Result<()> {
        // Initialize PSX with a disc loaded
        self.reset()
    }
    
    pub fn reset(&mut self) -> Result<()> {
        self.cpu = Cpu::new();
        self.gpu = Gpu::new();
        self.spu = Spu::new();
        self.dma = Dma::new();
        self.timers = Timers::new();
        self.irq = Irq::new();
        self.pad_memcard = PadMemCard::new();
        
        self.cpu.pc = 0xbfc00000;
        Ok(())
    }
    
    pub fn run_frame(&mut self) -> Result<()> {
        // Run approximately one frame worth of CPU cycles
        // PSX runs at ~33.8688 MHz, 60 FPS = ~564,480 cycles per frame
        for _ in 0..10000 {
            self.cpu.step()?;
        }
        Ok(())
    }
    
    pub fn set_controller_state(&mut self, controller: usize, state: u16) {
        if controller < 2 {
            self.pad_memcard.controller_state[controller] = state;
        }
    }
    
    pub fn get_framebuffer(&self, buffer: &mut Vec<u8>) {
        // Generate a simple test pattern for now
        let width = self.display_width as usize;
        let height = self.display_height as usize;
        
        buffer.resize(width * height * 4, 0);
        
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 4;
                // Simple gradient pattern
                buffer[idx] = (x * 255 / width) as u8;     // R
                buffer[idx + 1] = (y * 255 / height) as u8; // G
                buffer[idx + 2] = 128;                      // B
                buffer[idx + 3] = 255;                      // A
            }
        }
    }
    
    pub fn get_audio_samples(&self, buffer: &mut Vec<f32>) {
        // Generate silence for now
        buffer.clear();
    }
    
    fn load<T: AccessWidth>(&self, addr: u32) -> T {
        let masked_addr = addr & 0x1fffffff;
        
        match masked_addr {
            0x00000000..=0x001fffff => {
                // RAM
                let offset = (masked_addr & 0x1fffff) as usize;
                if offset < self.xmem.ram.len() {
                    T::from_u32(self.xmem.ram[offset] as u32)
                } else {
                    T::from_u32(0)
                }
            }
            0x1fc00000..=0x1fc7ffff => {
                // BIOS
                let offset = (masked_addr & 0x7ffff) as usize;
                if offset < self.bios.len() {
                    T::from_u32(self.bios[offset] as u32)
                } else {
                    T::from_u32(0)
                }
            }
            _ => T::from_u32(0),
        }
    }
    
    fn store<T: AccessWidth>(&mut self, addr: u32, val: T) {
        let masked_addr = addr & 0x1fffffff;
        
        match masked_addr {
            0x00000000..=0x001fffff => {
                // RAM
                let offset = (masked_addr & 0x1fffff) as usize;
                val.store(&mut self.xmem.ram[offset..]);
            }
            _ => {
                // Ignore writes to other areas for now
            }
        }
    }
}