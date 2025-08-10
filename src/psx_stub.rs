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
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> Result<T> {
        Ok(T::from_u32(0))
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) -> Result<()> {
        // Stub implementation
        Ok(())
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
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> Result<T> {
        Ok(T::from_u32(0))
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) -> Result<()> {
        // Stub implementation
        Ok(())
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
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> Result<T> {
        Ok(T::from_u32(0))
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) -> Result<()> {
        // Stub implementation
        Ok(())
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
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> Result<T> {
        Ok(T::from_u32(0))
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) -> Result<()> {
        // Stub implementation
        Ok(())
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
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> Result<T> {
        Ok(T::from_u32(0))
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) -> Result<()> {
        // Stub implementation
        Ok(())
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
    
    pub fn load<T: AccessWidth>(&self, _addr: u32) -> Result<T> {
        Ok(T::from_u32(0))
    }
    
    pub fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) -> Result<()> {
        // Stub implementation
        Ok(())
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
        // Enhanced BIOS validation
        const EXPECTED_BIOS_SIZE: usize = 512 * 1024;
        
        if bios_data.is_empty() {
            return Err(PsxError::invalid_bios("BIOS data is empty"));
        }
        
        if bios_data.len() != EXPECTED_BIOS_SIZE {
            return Err(PsxError::invalid_bios(format!(
                "Invalid BIOS size: expected {} bytes, got {} bytes",
                EXPECTED_BIOS_SIZE,
                bios_data.len()
            )));
        }
        
        // Validate BIOS signature (PlayStation BIOS typically starts with specific patterns)
        // Check for common BIOS signatures
        let has_valid_signature = self.validate_bios_signature(bios_data);
        if !has_valid_signature {
            return Err(PsxError::invalid_bios(
                "BIOS signature validation failed - may not be a valid PlayStation BIOS"
            ));
        }
        
        self.bios.copy_from_slice(bios_data);
        Ok(())
    }
    
    fn validate_bios_signature(&self, bios_data: &[u8]) -> bool {
        // Check for common BIOS entry point patterns
        if bios_data.len() < 4 {
            return false;
        }
        
        // PlayStation BIOS typically has specific patterns at the reset vector
        // This is a simplified check - real validation would be more comprehensive
        let first_word = u32::from_le_bytes([bios_data[0], bios_data[1], bios_data[2], bios_data[3]]);
        
        // Check if it looks like MIPS code (rough heuristic)
        // Most BIOS start with a jump or branch instruction
        let is_jump = (first_word >> 26) == 0x02 || (first_word >> 26) == 0x03; // J or JAL
        let is_branch = (first_word >> 26) == 0x04 || (first_word >> 26) == 0x05; // BEQ or BNE
        
        is_jump || is_branch || first_word == 0x3C080000 // LUI instruction common in BIOS
    }
    
    pub fn load_exe(&mut self, exe_data: &[u8]) -> Result<()> {
        // Enhanced PSX-EXE loading with comprehensive validation
        const MIN_EXE_SIZE: usize = 0x800;
        const MAX_EXE_SIZE: usize = 2 * 1024 * 1024; // 2MB max
        
        if exe_data.is_empty() {
            return Err(PsxError::invalid_exe("EXE data is empty"));
        }
        
        if exe_data.len() < MIN_EXE_SIZE {
            return Err(PsxError::invalid_exe(format!(
                "EXE file too small: {} bytes (minimum: {} bytes)",
                exe_data.len(),
                MIN_EXE_SIZE
            )));
        }
        
        if exe_data.len() > MAX_EXE_SIZE {
            return Err(PsxError::invalid_exe(format!(
                "EXE file too large: {} bytes (maximum: {} bytes)",
                exe_data.len(),
                MAX_EXE_SIZE
            )));
        }
        
        // Check magic
        if &exe_data[0..8] != b"PS-X EXE" {
            return Err(PsxError::invalid_exe(
                "Invalid magic signature - not a PSX-EXE file"
            ));
        }
        
        // Safely read header fields with bounds checking
        let pc = self.read_u32_safe(exe_data, 0x10)?;
        let gp = self.read_u32_safe(exe_data, 0x14)?;
        let dest = self.read_u32_safe(exe_data, 0x18)?;
        let size = self.read_u32_safe(exe_data, 0x1c)?;
        let sp_base = self.read_u32_safe(exe_data, 0x30)?;
        let sp_offset = self.read_u32_safe(exe_data, 0x34)?;
        
        // Validate addresses
        if pc < 0x80000000 || pc >= 0x80200000 {
            return Err(PsxError::invalid_exe(format!(
                "Invalid PC address: {:#010x}",
                pc
            )));
        }
        
        if dest < 0x80000000 || dest >= 0x80200000 {
            return Err(PsxError::invalid_exe(format!(
                "Invalid destination address: {:#010x}",
                dest
            )));
        }
        
        // Calculate and validate sizes
        let available_data = exe_data.len().saturating_sub(MIN_EXE_SIZE);
        let exe_size = (size as usize).min(available_data);
        
        if exe_size == 0 {
            return Err(PsxError::invalid_exe("No executable data to load"));
        }
        
        // Calculate destination offset in RAM
        let dest_offset = (dest & 0x1fffff) as usize;
        
        // Validate that the executable fits in RAM
        if dest_offset.saturating_add(exe_size) > self.xmem.ram.len() {
            return Err(PsxError::invalid_exe(format!(
                "Executable would overflow RAM: dest={:#x}, size={:#x}, RAM size={:#x}",
                dest_offset,
                exe_size,
                self.xmem.ram.len()
            )));
        }
        
        // Copy executable to RAM
        let exe_start = MIN_EXE_SIZE;
        let exe_end = exe_start + exe_size;
        
        self.xmem.ram[dest_offset..dest_offset + exe_size]
            .copy_from_slice(&exe_data[exe_start..exe_end]);
        
        // Set CPU registers
        self.cpu.pc = pc;
        self.cpu.regs[28] = gp; // GP register
        self.cpu.regs[29] = sp_base.wrapping_add(sp_offset); // SP register
        self.cpu.regs[30] = sp_base.wrapping_add(sp_offset); // FP register
        
        Ok(())
    }
    
    fn read_u32_safe(&self, data: &[u8], offset: usize) -> Result<u32> {
        if offset + 4 > data.len() {
            return Err(PsxError::invalid_exe(format!(
                "Cannot read u32 at offset {:#x}: out of bounds",
                offset
            )));
        }
        
        Ok(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]))
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
        const CYCLES_PER_FRAME: u32 = 564_480;
        const CYCLES_PER_ITERATION: u32 = 100;
        
        for cycle in 0..(CYCLES_PER_FRAME / CYCLES_PER_ITERATION) {
            self.cpu.step().map_err(|e| {
                PsxError::emulation(
                    "CPU",
                    format!("CPU step failed at cycle {}: {:?}", cycle * CYCLES_PER_ITERATION, e),
                )
            })?;
            
            // Update other components periodically
            if cycle % 10 == 0 {
                self.update_timers()?;
                self.check_interrupts()?;
            }
        }
        Ok(())
    }
    
    fn update_timers(&mut self) -> Result<()> {
        // Update timer counters
        for timer in &mut self.timers.timers {
            timer.counter = timer.counter.wrapping_add(1);
        }
        Ok(())
    }
    
    fn check_interrupts(&mut self) -> Result<()> {
        // Check and handle interrupts
        if self.irq.status & self.irq.mask != 0 {
            // Interrupt pending
            // Would trigger CPU interrupt here
        }
        Ok(())
    }
    
    pub fn set_controller_state(&mut self, controller: usize, state: u16) -> Result<()> {
        if controller >= 2 {
            return Err(PsxError::ControllerError {
                port: controller,
                reason: format!("Invalid controller port: {} (valid: 0-1)", controller),
            });
        }
        
        self.pad_memcard.controller_state[controller] = state;
        Ok(())
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
    
    fn load<T: AccessWidth>(&self, addr: u32) -> Result<T> {
        let masked_addr = addr & 0x1fffffff;
        
        match masked_addr {
            0x00000000..=0x001fffff => {
                // RAM
                let offset = (masked_addr & 0x1fffff) as usize;
                if offset >= self.xmem.ram.len() {
                    return Err(PsxError::memory_violation(addr));
                }
                Ok(T::from_u32(self.xmem.ram[offset] as u32))
            }
            0x1fc00000..=0x1fc7ffff => {
                // BIOS
                let offset = (masked_addr & 0x7ffff) as usize;
                if offset >= self.bios.len() {
                    return Err(PsxError::memory_violation(addr));
                }
                Ok(T::from_u32(self.bios[offset] as u32))
            }
            0x1f801000..=0x1f801fff => {
                // Hardware registers
                self.load_hardware_register(masked_addr)
            }
            _ => {
                // Unknown memory region - log and return default
                Ok(T::from_u32(0))
            }
        }
    }
    
    fn load_hardware_register<T: AccessWidth>(&self, addr: u32) -> Result<T> {
        // Handle hardware register reads
        match addr {
            0x1f801070..=0x1f801077 => {
                // IRQ registers
                self.irq.load(addr)
            }
            0x1f801080..=0x1f8010ff => {
                // DMA registers
                self.dma.load(addr)
            }
            0x1f801100..=0x1f80112f => {
                // Timer registers
                self.timers.load(addr)
            }
            _ => Ok(T::from_u32(0)),
        }
    }
    
    fn store<T: AccessWidth>(&mut self, addr: u32, val: T) -> Result<()> {
        let masked_addr = addr & 0x1fffffff;
        
        match masked_addr {
            0x00000000..=0x001fffff => {
                // RAM - validate bounds
                let offset = (masked_addr & 0x1fffff) as usize;
                let bytes_needed = std::mem::size_of::<T>();
                
                if offset + bytes_needed > self.xmem.ram.len() {
                    return Err(PsxError::memory_violation(addr));
                }
                
                val.store(&mut self.xmem.ram[offset..]);
                Ok(())
            }
            0x1fc00000..=0x1fc7ffff => {
                // BIOS - read-only
                Err(PsxError::emulation(
                    "BIOS",
                    format!("Attempted write to read-only BIOS at {:#010x}", addr),
                ))
            }
            0x1f801000..=0x1f801fff => {
                // Hardware registers
                self.store_hardware_register(addr, val)
            }
            _ => {
                // Ignore writes to unmapped areas
                Ok(())
            }
        }
    }
    
    fn store_hardware_register<T: AccessWidth>(&mut self, addr: u32, val: T) -> Result<()> {
        // Handle hardware register writes
        match addr {
            0x1f801070..=0x1f801077 => {
                // IRQ registers
                self.irq.store(addr, val)
            }
            0x1f801080..=0x1f8010ff => {
                // DMA registers
                self.dma.store(addr, val)
            }
            0x1f801100..=0x1f80112f => {
                // Timer registers
                self.timers.store(addr, val)
            }
            _ => Ok(()),
        }
    }
}