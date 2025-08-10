// Modified PSX module for WASM build without cdimage dependency

use crate::cd_stub as cdimage;
use std::fmt;

// Re-export modules that we'll use
pub mod cpu;
pub mod cop0;
pub mod gpu;
pub mod gte;
pub mod spu;
pub mod dma;
pub mod timers;
pub mod irq;
pub mod pad_memcard;
pub mod memory_control;
pub mod cache;
pub mod bios;
pub mod xmem;
pub mod sync;

// Stub CD modules
pub mod cd {
    use super::cdimage;
    
    pub const CDC_ROM_SIZE: usize = 16 * 1024; // 16KB CDC ROM
    
    pub mod disc {
        use super::super::cdimage;
        
        #[derive(Clone)]
        pub struct Disc {
            image: Box<dyn DiscImage>,
        }
        
        trait DiscImage: Send {
            fn read_sector(&mut self, msf: cdimage::Msf) -> cdimage::CdResult<cdimage::Sector>;
            fn clone_box(&self) -> Box<dyn DiscImage>;
        }
        
        impl Clone for Box<dyn DiscImage> {
            fn clone(&self) -> Self {
                self.clone_box()
            }
        }
        
        struct StubDiscImage;
        
        impl DiscImage for StubDiscImage {
            fn read_sector(&mut self, _msf: cdimage::Msf) -> cdimage::CdResult<cdimage::Sector> {
                Ok(cdimage::Sector::new())
            }
            
            fn clone_box(&self) -> Box<dyn DiscImage> {
                Box::new(StubDiscImage)
            }
        }
        
        impl Disc {
            pub fn new() -> Self {
                Disc {
                    image: Box::new(StubDiscImage),
                }
            }
            
            pub fn region(&self) -> Region {
                Region::NorthAmerica
            }
        }
        
        #[derive(Debug, Clone, Copy)]
        pub enum Region {
            Japan,
            NorthAmerica,
            Europe,
        }
        
        impl Region {
            pub fn video_standard(&self) -> crate::gpu::VideoStandard {
                match self {
                    Region::Japan | Region::NorthAmerica => crate::gpu::VideoStandard::Ntsc,
                    Region::Europe => crate::gpu::VideoStandard::Pal,
                }
            }
        }
    }
    
    pub struct CdInterface {
        cdc: Cdc,
    }
    
    pub struct Cdc {
        disc: Option<disc::Disc>,
        rom: Vec<u8>,
    }
    
    impl Cdc {
        pub fn new(_disc: Option<disc::Disc>, rom: Vec<u8>) -> Self {
            Cdc {
                disc: None,
                rom,
            }
        }
        
        pub fn take_disc(&mut self) -> Option<disc::Disc> {
            self.disc.take()
        }
        
        pub fn set_disc(&mut self, disc: Option<disc::Disc>) -> Result<(), (PsxError, Option<disc::Disc>)> {
            self.disc = disc;
            Ok(())
        }
        
        pub fn copy_rom(&mut self, _other: &Cdc) {
            // Stub
        }
        
        pub fn sync(&mut self, _sync_clock: &crate::sync::SyncClock) {
            // Stub
        }
    }
    
    impl CdInterface {
        pub fn new(disc: Option<disc::Disc>, cdc_firmware: [u8; CDC_ROM_SIZE]) -> Result<Self, PsxError> {
            Ok(CdInterface {
                cdc: Cdc::new(disc, cdc_firmware.to_vec()),
            })
        }
        
        pub fn sync(&mut self, sync_clock: &crate::sync::SyncClock) {
            self.cdc.sync(sync_clock);
        }
    }
    
    use super::PsxError;
}

// Error types
#[derive(Debug)]
pub enum PsxError {
    InvalidBios(String),
    InvalidDisc(String),
    DeserializationError(String),
    CdError(String),
}

impl fmt::Display for PsxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PsxError::InvalidBios(s) => write!(f, "Invalid BIOS: {}", s),
            PsxError::InvalidDisc(s) => write!(f, "Invalid Disc: {}", s),
            PsxError::DeserializationError(s) => write!(f, "Deserialization error: {}", s),
            PsxError::CdError(s) => write!(f, "CD error: {}", s),
        }
    }
}

impl std::error::Error for PsxError {}

pub type Result<T> = std::result::Result<T, PsxError>;

// Re-export common types
pub use sync::CycleCount;
pub use cpu::AccessWidth;

pub trait Addressable {
    fn load<T: AccessWidth>(&mut self, addr: u32) -> T;
    fn store<T: AccessWidth>(&mut self, addr: u32, val: T);
}

// Main PSX struct
pub struct Psx {
    pub xmem: xmem::XMemory,
    pub cpu: cpu::Cpu,
    pub gpu: gpu::Gpu,
    pub spu: spu::Spu,
    pub dma: dma::Dma,
    pub timers: timers::Timers,
    pub irq: irq::Irq,
    pub pad_memcard: pad_memcard::PadMemCard,
    pub cd: cd::CdInterface,
    pub cache_system: cache::CacheSystem,
    pub memory_ctrl: memory_control::MemoryControl,
    pub mem_control: [u32; 9],
    pub ram_size: u32,
    pub cache_control: u32,
    pub dma_timing_penalty: CycleCount,
    pub cpu_stalled_for_dma: bool,
    pub sync_clock: sync::SyncClock,
}

impl Psx {
    pub fn new_without_disc(bios: bios::Bios) -> Result<Self> {
        let standard = gpu::VideoStandard::Ntsc;
        let cdc_firmware = [0u8; cd::CDC_ROM_SIZE];
        
        Self::new_with_bios(None, bios, standard, cdc_firmware)
    }
    
    pub fn new_with_bios(
        disc: Option<cd::disc::Disc>,
        bios: bios::Bios,
        standard: gpu::VideoStandard,
        cdc_firmware: [u8; cd::CDC_ROM_SIZE],
    ) -> Result<Self> {
        let mut xmem = xmem::XMemory::new();
        xmem.set_bios(bios.get_rom());
        
        let cd = cd::CdInterface::new(disc, cdc_firmware)?;
        
        Ok(Psx {
            xmem,
            cpu: cpu::Cpu::new(),
            gpu: gpu::Gpu::new(standard),
            spu: spu::Spu::new(),
            dma: dma::Dma::new(),
            timers: timers::Timers::new(),
            irq: irq::Irq::new(),
            pad_memcard: pad_memcard::PadMemCard::new(),
            cd,
            cache_system: cache::CacheSystem::new(),
            memory_ctrl: memory_control::MemoryControl::new(),
            mem_control: [
                0x1f000000, 0x1f802000, 0x0013243f,
                0x00003022, 0x0013243f, 0x200931e1,
                0x00020843, 0x00070777, 0x00031125,
            ],
            ram_size: 0x00000b88,
            cache_control: 0,
            dma_timing_penalty: CycleCount(0),
            cpu_stalled_for_dma: false,
            sync_clock: sync::SyncClock::new(),
        })
    }
    
    pub fn run_frame(&mut self) -> Result<()> {
        // Run approximately one frame worth of cycles
        // NTSC: ~33.8688 MHz / 60 fps = ~564480 cycles per frame
        let cycles_per_frame = 564480;
        let mut cycles_run = 0;
        
        while cycles_run < cycles_per_frame {
            // Execute CPU instruction
            if !self.cpu_stalled_for_dma {
                let cycles = self.cpu.run_next_instruction(self)?;
                cycles_run += cycles.0;
                self.sync_clock.tick(cycles);
            }
            
            // Update components
            self.gpu.sync(&self.sync_clock);
            self.spu.sync(&self.sync_clock);
            self.timers.sync(&self.sync_clock, &mut self.irq);
            self.dma.sync(self);
            self.cd.sync(&self.sync_clock);
            
            // Handle interrupts
            if self.irq.pending() && self.cpu.interrupts_enabled() {
                self.cpu.trigger_interrupt();
            }
        }
        
        Ok(())
    }
    
    pub fn get_display_size(&self) -> (u32, u32) {
        (self.gpu.display_width() as u32, self.gpu.display_height() as u32)
    }
    
    pub fn get_framebuffer(&self, buffer: &mut Vec<u8>) {
        let width = self.gpu.display_width() as usize;
        let height = self.gpu.display_height() as usize;
        
        buffer.clear();
        buffer.resize(width * height * 4, 0);
        
        // Get the framebuffer from GPU VRAM
        let vram = self.gpu.vram();
        let display_start = self.gpu.display_start();
        
        for y in 0..height {
            for x in 0..width {
                let vram_x = (display_start.0 as usize + x) % 1024;
                let vram_y = (display_start.1 as usize + y) % 512;
                let pixel = vram[vram_y * 1024 + vram_x];
                
                // Convert 15-bit RGB to 32-bit RGBA
                let r = ((pixel & 0x1F) << 3) as u8;
                let g = (((pixel >> 5) & 0x1F) << 3) as u8;
                let b = (((pixel >> 10) & 0x1F) << 3) as u8;
                
                let offset = (y * width + x) * 4;
                buffer[offset] = r;
                buffer[offset + 1] = g;
                buffer[offset + 2] = b;
                buffer[offset + 3] = 255;
            }
        }
    }
    
    pub fn get_audio_samples(&mut self) -> Vec<i16> {
        self.spu.get_samples()
    }
    
    pub fn set_controller_state(&mut self, port: usize, state: u16) {
        if port < 2 {
            self.pad_memcard.set_gamepad_state(port, state);
        }
    }
    
    pub fn serialize_state(&self) -> Vec<u8> {
        // Simplified serialization
        vec![0; 1024]
    }
    
    pub fn deserialize_state(&mut self, _data: &[u8]) -> Result<()> {
        // Simplified deserialization
        Ok(())
    }
    
    pub fn load_exe(&mut self, exe_data: &[u8]) -> Result<()> {
        // Parse PSX-EXE header
        if exe_data.len() < 0x800 {
            return Err(PsxError::InvalidDisc("EXE file too small".to_string()));
        }
        
        // Check magic number
        if &exe_data[0..8] != b"PS-X EXE" {
            return Err(PsxError::InvalidDisc("Invalid PS-X EXE header".to_string()));
        }
        
        // Read header values (little-endian)
        let initial_pc = u32::from_le_bytes([exe_data[0x10], exe_data[0x11], exe_data[0x12], exe_data[0x13]]);
        let initial_gp = u32::from_le_bytes([exe_data[0x14], exe_data[0x15], exe_data[0x16], exe_data[0x17]]);
        let load_addr = u32::from_le_bytes([exe_data[0x18], exe_data[0x19], exe_data[0x1a], exe_data[0x1b]]);
        let file_size = u32::from_le_bytes([exe_data[0x1c], exe_data[0x1d], exe_data[0x1e], exe_data[0x1f]]);
        let initial_sp = u32::from_le_bytes([exe_data[0x30], exe_data[0x31], exe_data[0x32], exe_data[0x33]]);
        
        // Load executable into RAM
        let exe_start = 0x800;
        let exe_end = exe_start + file_size as usize;
        
        if exe_end > exe_data.len() {
            return Err(PsxError::InvalidDisc("EXE file size mismatch".to_string()));
        }
        
        // Copy executable to RAM
        let exe_code = &exe_data[exe_start..exe_end];
        for (i, &byte) in exe_code.iter().enumerate() {
            let addr = load_addr + i as u32;
            self.store::<u8>(addr, byte);
        }
        
        // Set up CPU registers
        self.cpu.set_pc(initial_pc);
        self.cpu.set_reg(28, initial_gp); // GP register
        self.cpu.set_reg(29, initial_sp); // SP register
        self.cpu.set_reg(30, initial_sp); // FP register
        
        Ok(())
    }
}

// Implement Addressable for PSX memory access
impl Addressable for Psx {
    fn load<T: AccessWidth>(&mut self, addr: u32) -> T {
        let masked_addr = addr & 0x1fffffff;
        
        match masked_addr {
            // RAM
            0x00000000..=0x001fffff => {
                let offset = (masked_addr & 0x1fffff) as usize;
                T::load(&self.xmem.ram()[offset..])
            }
            // BIOS
            0x1fc00000..=0x1fc7ffff => {
                let offset = (masked_addr & 0x7ffff) as usize;
                T::load(&self.xmem.bios()[offset..])
            }
            // Hardware registers
            0x1f801000..=0x1f801023 => {
                // Memory control
                T::from_u32(self.mem_control[((masked_addr & 0xff) / 4) as usize])
            }
            0x1f801040..=0x1f80104f => {
                // Pad/Memory card
                self.pad_memcard.load(masked_addr)
            }
            0x1f801070..=0x1f801077 => {
                // IRQ
                self.irq.load(masked_addr)
            }
            0x1f801080..=0x1f8010ff => {
                // DMA
                self.dma.load(masked_addr)
            }
            0x1f801100..=0x1f80112f => {
                // Timers
                self.timers.load(masked_addr)
            }
            0x1f801810..=0x1f801817 => {
                // GPU
                self.gpu.load(masked_addr)
            }
            0x1f801c00..=0x1f801fff => {
                // SPU
                self.spu.load(masked_addr)
            }
            _ => {
                // Unknown address
                T::from_u32(0xffffffff)
            }
        }
    }
    
    fn store<T: AccessWidth>(&mut self, addr: u32, val: T) {
        let masked_addr = addr & 0x1fffffff;
        
        match masked_addr {
            // RAM
            0x00000000..=0x001fffff => {
                let offset = (masked_addr & 0x1fffff) as usize;
                val.store(&mut self.xmem.ram_mut()[offset..]);
            }
            // Hardware registers
            0x1f801000..=0x1f801023 => {
                // Memory control
                self.mem_control[((masked_addr & 0xff) / 4) as usize] = val.to_u32();
            }
            0x1f801040..=0x1f80104f => {
                // Pad/Memory card
                self.pad_memcard.store(masked_addr, val);
            }
            0x1f801070..=0x1f801077 => {
                // IRQ
                self.irq.store(masked_addr, val);
            }
            0x1f801080..=0x1f8010ff => {
                // DMA
                self.dma.store(masked_addr, val);
            }
            0x1f801100..=0x1f80112f => {
                // Timers
                self.timers.store(masked_addr, val);
            }
            0x1f801810..=0x1f801817 => {
                // GPU
                self.gpu.store(masked_addr, val);
            }
            0x1f801c00..=0x1f801fff => {
                // SPU
                self.spu.store(masked_addr, val);
            }
            _ => {
                // Unknown address - ignore write
            }
        }
    }
}