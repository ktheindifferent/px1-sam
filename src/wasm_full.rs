// Full PSX WASM implementation with proper module structure

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent,
    AudioContext, Gamepad
};
use std::cell::RefCell;

// Set up logging
use log::{info, error, debug};
use wasm_logger;

// Module structure for PSX emulator
pub mod psx {
    // Re-export required types at module level
    pub use super::sync::CycleCount;
    pub use super::cpu::AccessWidth;
    pub use super::Addressable;
    
    // Include all PSX modules with proper paths
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
    pub mod cd;
    
    // Stub for mdec (Motion Decoder) - not critical for basic emulation
    pub mod mdec {
        use super::*;
        
        pub struct Mdec;
        
        impl Mdec {
            pub fn new() -> Self { Mdec }
            pub fn sync(&mut self, _clock: &sync::SyncClock) {}
        }
        
        impl Addressable for Mdec {
            fn load<T: AccessWidth>(&mut self, _addr: u32) -> T {
                T::from_u32(0)
            }
            fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) {}
        }
    }
    
    // Map module for memory mapping
    pub mod map {
        pub const RAM_START: u32 = 0x00000000;
        pub const RAM_END: u32 = 0x001fffff;
        pub const BIOS_START: u32 = 0x1fc00000;
        pub const BIOS_END: u32 = 0x1fc7ffff;
    }
}

// Include module files with proper cfg for WASM
#[cfg(target_arch = "wasm32")]
#[path = "psx/cpu.rs"]
pub mod cpu;

#[cfg(target_arch = "wasm32")]
#[path = "psx/cop0.rs"]
pub mod cop0;

#[cfg(target_arch = "wasm32")]
#[path = "psx/gpu/mod.rs"]
pub mod gpu;

#[cfg(target_arch = "wasm32")]
#[path = "psx/gte/mod.rs"]
pub mod gte;

#[cfg(target_arch = "wasm32")]
#[path = "psx/spu/mod.rs"]
pub mod spu;

#[cfg(target_arch = "wasm32")]
#[path = "psx/dma.rs"]
pub mod dma;

#[cfg(target_arch = "wasm32")]
#[path = "psx/timers.rs"]
pub mod timers;

#[cfg(target_arch = "wasm32")]
#[path = "psx/irq.rs"]
pub mod irq;

#[cfg(target_arch = "wasm32")]
#[path = "psx/pad_memcard/mod.rs"]
pub mod pad_memcard;

#[cfg(target_arch = "wasm32")]
#[path = "psx/memory_control.rs"]
pub mod memory_control;

#[cfg(target_arch = "wasm32")]
#[path = "psx/cache.rs"]
pub mod cache;

#[cfg(target_arch = "wasm32")]
#[path = "psx/bios/mod.rs"]
pub mod bios;

#[cfg(target_arch = "wasm32")]
#[path = "psx/xmem.rs"]
pub mod xmem;

#[cfg(target_arch = "wasm32")]
#[path = "psx/sync.rs"]
pub mod sync;


// ISO9660 module - stub for WASM
#[cfg(target_arch = "wasm32")]
pub mod iso9660 {
    use thiserror::Error;
    use std::sync::Arc;
    
    // Stub CdCache for ISO9660
    pub mod disc {
        pub struct CdCache;
    }
    
    // Stub types for ISO9660
    pub struct Directory {
        entries: Vec<Entry>,
    }
    
    impl Directory {
        pub fn new(_image: &mut disc::CdCache, _entry: &Entry) -> Result<Directory, IsoError> {
            Ok(Directory { entries: Vec::new() })
        }
        
        pub fn entry_by_name(&self, _name: &[u8]) -> Result<&Entry, IsoError> {
            Err(IsoError::EntryNotFound("stub".to_string()))
        }
        
        pub fn cd(&self, _image: &mut disc::CdCache, _name: &[u8]) -> Result<Directory, IsoError> {
            Ok(Directory { entries: Vec::new() })
        }
        
        pub fn ls(&self) -> &[Entry] {
            &self.entries
        }
    }
    
    pub struct Entry(Vec<u8>);
    
    impl Entry {
        pub fn name(&self) -> &[u8] {
            b"stub"
        }
        
        pub fn is_dir(&self) -> bool {
            false
        }
        
        pub fn extent_location(&self) -> u32 {
            0
        }
        
        pub fn extent_len(&self) -> u32 {
            0
        }
        
        pub fn read_file(&self, _image: &mut disc::CdCache) -> Result<Vec<u8>, IsoError> {
            Ok(Vec::new())
        }
    }
    
    #[derive(Error, Debug)]
    pub enum IsoError {
        #[error("Cdimage access error: stub")]
        CdError,
        #[error("Cdimage access error: stub")]
        CachedCdError,
        #[error("Couldn't find the ISO9660 magic `CD0001`")]
        BadMagic,
        #[error("Couldn't find the Primary Volume Descriptor")]
        MissingPrimaryVolumeDescriptor,
        #[error("Unexpected Volume Descriptor version")]
        BadVolumDescriptorVersion,
        #[error("Encountered an invalid extent location")]
        BadExtent(u32),
        #[error("ISO9660 directory entry is too short")]
        InvalidDirectoryEntryLen(usize),
        #[error("ISO9660 entry name is longer than the directory")]
        EntryNameTooLong,
        #[error("The requested entry could not be found")]
        EntryNotFound(String),
        #[error("We expected a directory and got a file")]
        NotADirectory,
        #[error("We expected a file and got a directory")]
        NotAFile,
    }
    
    pub fn open_image(_image: &mut disc::CdCache) -> Result<Directory, IsoError> {
        Ok(Directory { entries: Vec::new() })
    }
}

// CD module with stubs for WASM  
#[cfg(target_arch = "wasm32")]
pub mod cd {
    use super::*;
    use crate::sync::{SyncClock, CycleCount};
    
    pub const CDC_ROM_SIZE: usize = 16 * 1024;
    pub const CDC_ROM_SHA256: &str = "stub";
    
    pub mod disc {
        use super::*;
        
        #[derive(Clone, Debug)]
        pub struct Disc;
        
        impl Disc {
            pub fn new() -> Self { Disc }
            pub fn region(&self) -> Region { Region::NorthAmerica }
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
        
        #[derive(Debug, Clone)]
        pub struct SerialNumber(pub String);
    }
    
    pub mod cdc {
        use super::*;
        
        pub struct Cdc {
            rom: Vec<u8>,
        }
        
        impl Cdc {
            pub fn new(_disc: Option<disc::Disc>, rom: Vec<u8>) -> Self {
                Cdc { rom }
            }
            
            pub fn sync(&mut self, _clock: &SyncClock) {}
            
            pub fn take_disc(&mut self) -> Option<disc::Disc> { None }
            
            pub fn set_disc(&mut self, _disc: Option<disc::Disc>) -> Result<(), (String, Option<disc::Disc>)> {
                Ok(())
            }
            
            pub fn copy_rom(&mut self, _other: &Cdc) {}
        }
    }
    
    pub struct CdInterface {
        pub cdc: cdc::Cdc,
    }
    
    impl CdInterface {
        pub fn new(disc: Option<disc::Disc>, cdc_firmware: [u8; CDC_ROM_SIZE]) -> Result<Self, String> {
            Ok(CdInterface {
                cdc: cdc::Cdc::new(disc, cdc_firmware.to_vec()),
            })
        }
        
        pub fn sync(&mut self, clock: &SyncClock) {
            self.cdc.sync(clock);
        }
    }
    
    impl Addressable for CdInterface {
        fn load<T: AccessWidth>(&mut self, _addr: u32) -> T {
            T::from_u32(0)
        }
        
        fn store<T: AccessWidth>(&mut self, _addr: u32, _val: T) {}
    }
}

// Helper modules
#[cfg(target_arch = "wasm32")]
#[path = "memory_card.rs"]
pub mod memory_card;

#[cfg(target_arch = "wasm32")]
#[path = "error.rs"]
pub mod error;

#[cfg(target_arch = "wasm32")]
#[path = "bitwise.rs"]
pub mod bitwise;

#[cfg(target_arch = "wasm32")]
#[path = "box_array.rs"]
pub mod box_array;

// SHA stub for WASM
#[cfg(target_arch = "wasm32")]
pub mod sha {
    pub fn sha256(_data: &[u8]) -> String {
        "0000000000000000000000000000000000000000000000000000000000000000".to_string()
    }
}

// Assembler stub for WASM
#[cfg(target_arch = "wasm32")]
pub mod assembler {
    pub mod syntax {
        pub const AT: u32 = 1;
        pub const GP: u32 = 28;
        
        pub enum Label {
            Absolute(u32),
        }
        
        pub fn Li(_reg: u32, _val: u32) {}
        pub fn Jal(_label: Label) {}
        pub fn Sw(_rt: u32, _rs: u32, _offset: i32) {}
    }
    
    pub struct Assembler;
}

// libretro stub for WASM
#[cfg(target_arch = "wasm32")]
pub mod libretro {
    pub fn save_memory_card(_slot: usize, _data: &[u8]) {}
    pub fn load_memory_card(_slot: usize) -> Option<Vec<u8>> { None }
}

// VRamDisplayMode for GPU
#[derive(Debug, Clone, Copy)]
pub enum VRamDisplayMode {
    Disabled,
    Enabled,
}

// Core trait for memory access
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
    pub mdec: psx::mdec::Mdec,
    pub cache_system: cache::CacheSystem,
    pub memory_ctrl: memory_control::MemoryControl,
    pub mem_control: [u32; 9],
    pub ram_size: u32,
    pub cache_control: u32,
    pub dma_timing_penalty: sync::CycleCount,
    pub cpu_stalled_for_dma: bool,
    pub sync_clock: sync::SyncClock,
}

impl Psx {
    pub fn new_without_disc(bios: bios::Bios) -> Result<Self, String> {
        let standard = gpu::VideoStandard::Ntsc;
        let cdc_firmware = [0u8; cd::CDC_ROM_SIZE];
        
        Self::new_with_bios(None, bios, standard, cdc_firmware)
    }
    
    pub fn new_with_bios(
        disc: Option<cd::disc::Disc>,
        bios: bios::Bios,
        standard: gpu::VideoStandard,
        cdc_firmware: [u8; cd::CDC_ROM_SIZE],
    ) -> Result<Self, String> {
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
            mdec: psx::mdec::Mdec::new(),
            cache_system: cache::CacheSystem::new(),
            memory_ctrl: memory_control::MemoryControl::new(),
            mem_control: [
                0x1f000000, 0x1f802000, 0x0013243f,
                0x00003022, 0x0013243f, 0x200931e1,
                0x00020843, 0x00070777, 0x00031125,
            ],
            ram_size: 0x00000b88,
            cache_control: 0,
            dma_timing_penalty: sync::CycleCount(0),
            cpu_stalled_for_dma: false,
            sync_clock: sync::SyncClock::new(),
        })
    }
    
    pub fn run_frame(&mut self) -> Result<(), String> {
        // Run approximately one frame worth of cycles
        let cycles_per_frame = 564480;
        let mut cycles_run = 0;
        
        while cycles_run < cycles_per_frame {
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
            self.mdec.sync(&self.sync_clock);
            
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
        
        let vram = self.gpu.vram();
        let display_start = self.gpu.display_start();
        
        for y in 0..height {
            for x in 0..width {
                let vram_x = (display_start.0 as usize + x) % 1024;
                let vram_y = (display_start.1 as usize + y) % 512;
                let pixel = vram[vram_y * 1024 + vram_x];
                
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
    
    pub fn load_exe(&mut self, exe_data: &[u8]) -> Result<(), String> {
        if exe_data.len() < 0x800 {
            return Err("EXE file too small".to_string());
        }
        
        if &exe_data[0..8] != b"PS-X EXE" {
            return Err("Invalid PS-X EXE header".to_string());
        }
        
        let initial_pc = u32::from_le_bytes([exe_data[0x10], exe_data[0x11], exe_data[0x12], exe_data[0x13]]);
        let initial_gp = u32::from_le_bytes([exe_data[0x14], exe_data[0x15], exe_data[0x16], exe_data[0x17]]);
        let load_addr = u32::from_le_bytes([exe_data[0x18], exe_data[0x19], exe_data[0x1a], exe_data[0x1b]]);
        let file_size = u32::from_le_bytes([exe_data[0x1c], exe_data[0x1d], exe_data[0x1e], exe_data[0x1f]]);
        let initial_sp = u32::from_le_bytes([exe_data[0x30], exe_data[0x31], exe_data[0x32], exe_data[0x33]]);
        
        let exe_start = 0x800;
        let exe_end = exe_start + file_size as usize;
        
        if exe_end > exe_data.len() {
            return Err("EXE file size mismatch".to_string());
        }
        
        let exe_code = &exe_data[exe_start..exe_end];
        for (i, &byte) in exe_code.iter().enumerate() {
            let addr = load_addr + i as u32;
            self.store::<u8>(addr, byte);
        }
        
        self.cpu.set_pc(initial_pc);
        self.cpu.set_reg(28, initial_gp);
        self.cpu.set_reg(29, initial_sp);
        self.cpu.set_reg(30, initial_sp);
        
        Ok(())
    }
}

// Implement Addressable for PSX
impl Addressable for Psx {
    fn load<T: AccessWidth>(&mut self, addr: u32) -> T {
        let masked_addr = addr & 0x1fffffff;
        
        match masked_addr {
            0x00000000..=0x001fffff => {
                let offset = (masked_addr & 0x1fffff) as usize;
                T::load(&self.xmem.ram()[offset..])
            }
            0x1fc00000..=0x1fc7ffff => {
                let offset = (masked_addr & 0x7ffff) as usize;
                T::load(&self.xmem.bios()[offset..])
            }
            0x1f801040..=0x1f80104f => self.pad_memcard.load(masked_addr),
            0x1f801070..=0x1f801077 => self.irq.load(masked_addr),
            0x1f801080..=0x1f8010ff => self.dma.load(masked_addr),
            0x1f801100..=0x1f80112f => self.timers.load(masked_addr),
            0x1f801810..=0x1f801817 => self.gpu.load(masked_addr),
            0x1f801800..=0x1f801803 => self.cd.load(masked_addr),
            0x1f801820..=0x1f801827 => self.mdec.load(masked_addr),
            0x1f801c00..=0x1f801fff => self.spu.load(masked_addr),
            _ => T::from_u32(0xffffffff),
        }
    }
    
    fn store<T: AccessWidth>(&mut self, addr: u32, val: T) {
        let masked_addr = addr & 0x1fffffff;
        
        match masked_addr {
            0x00000000..=0x001fffff => {
                let offset = (masked_addr & 0x1fffff) as usize;
                val.store(&mut self.xmem.ram_mut()[offset..]);
            }
            0x1f801040..=0x1f80104f => self.pad_memcard.store(masked_addr, val),
            0x1f801070..=0x1f801077 => self.irq.store(masked_addr, val),
            0x1f801080..=0x1f8010ff => self.dma.store(masked_addr, val),
            0x1f801100..=0x1f80112f => self.timers.store(masked_addr, val),
            0x1f801810..=0x1f801817 => self.gpu.store(masked_addr, val),
            0x1f801800..=0x1f801803 => self.cd.store(masked_addr, val),
            0x1f801820..=0x1f801827 => self.mdec.store(masked_addr, val),
            0x1f801c00..=0x1f801fff => self.spu.store(masked_addr, val),
            _ => {}
        }
    }
}

// WASM bindings
#[cfg(feature = "console_error_panic_hook")]
use console_error_panic_hook;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

#[wasm_bindgen]
pub struct PsxEmulator {
    psx: Option<Psx>,
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    audio_context: Option<AudioContext>,
    frame_buffer: Vec<u8>,
    audio_buffer: Vec<f32>,
    input_state: InputState,
    running: RefCell<bool>,
    frame_count: u32,
}

#[wasm_bindgen]
pub struct InputState {
    keys: RefCell<[bool; 256]>,
    gamepad_buttons: RefCell<[bool; 16]>,
    gamepad_axes: RefCell<[f32; 4]>,
}

#[wasm_bindgen]
impl InputState {
    pub fn new() -> Self {
        InputState {
            keys: RefCell::new([false; 256]),
            gamepad_buttons: RefCell::new([false; 16]),
            gamepad_axes: RefCell::new([0.0; 4]),
        }
    }

    pub fn set_key(&self, keycode: u32, pressed: bool) {
        if keycode < 256 {
            self.keys.borrow_mut()[keycode as usize] = pressed;
        }
    }

    pub fn update_gamepad(&self, gamepad: &Gamepad) {
        let buttons = gamepad.buttons();
        let mut gamepad_buttons = self.gamepad_buttons.borrow_mut();
        
        for (i, button) in buttons.iter().enumerate() {
            if i >= 16 { break; }
            if let Ok(button) = button.dyn_into::<web_sys::GamepadButton>() {
                gamepad_buttons[i] = button.pressed();
            }
        }

        let axes = gamepad.axes();
        let mut gamepad_axes = self.gamepad_axes.borrow_mut();
        
        for (i, axis) in axes.iter().enumerate() {
            if i >= 4 { break; }
            if let Some(val) = axis.as_f64() {
                gamepad_axes[i] = val as f32;
            }
        }
    }
}

#[wasm_bindgen]
impl PsxEmulator {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<PsxEmulator, JsValue> {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();
        
        // Initialize logger
        wasm_logger::init(wasm_logger::Config::default());
        
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document.get_element_by_id(canvas_id).unwrap();
        let canvas: HtmlCanvasElement = canvas
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| JsValue::from_str("Failed to get canvas element"))?;

        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();

        canvas.set_width(640);
        canvas.set_height(480);

        let audio_context = AudioContext::new().ok();
        
        info!("PSX WASM Emulator initialized");
        
        Ok(PsxEmulator {
            psx: None,
            canvas,
            context,
            audio_context,
            frame_buffer: Vec::with_capacity(640 * 480 * 4),
            audio_buffer: Vec::with_capacity(4096),
            input_state: InputState::new(),
            running: RefCell::new(false),
            frame_count: 0,
        })
    }

    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), JsValue> {
        let bios = match bios::Bios::new(bios_data) {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to load BIOS: {:?}", e);
                return Err(JsValue::from_str(&format!("Invalid BIOS: {:?}", e)));
            }
        };
        
        match Psx::new_without_disc(bios) {
            Ok(psx) => {
                self.psx = Some(psx);
                info!("PSX initialized with BIOS");
                Ok(())
            }
            Err(e) => {
                error!("Failed to initialize PSX: {:?}", e);
                Err(JsValue::from_str(&format!("PSX init failed: {:?}", e)))
            }
        }
    }

    pub fn load_game(&mut self, game_data: &[u8]) -> Result<(), JsValue> {
        if let Some(ref mut psx) = self.psx {
            if game_data.len() > 8 && &game_data[0..8] == b"PS-X EXE" {
                match psx.load_exe(game_data) {
                    Ok(_) => {
                        info!("PSX-EXE loaded successfully");
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to load EXE: {}", e);
                        Err(JsValue::from_str(&format!("Failed to load EXE: {}", e)))
                    }
                }
            } else {
                error!("Only PSX-EXE files are supported in WASM build");
                Err(JsValue::from_str("Only PSX-EXE files are supported"))
            }
        } else {
            Err(JsValue::from_str("BIOS must be loaded first"))
        }
    }

    pub fn run_frame(&mut self) -> Result<(), JsValue> {
        if !self.is_running() {
            return Ok(());
        }

        if let Some(ref mut psx) = self.psx {
            self.update_input(psx);
            
            match psx.run_frame() {
                Ok(_) => {
                    self.frame_count += 1;
                    self.render_frame(psx)?;
                    self.process_audio(psx)?;
                    
                    if self.frame_count % 60 == 0 {
                        debug!("Frame {}: Emulation running", self.frame_count);
                    }
                    
                    Ok(())
                }
                Err(e) => {
                    error!("Frame execution error: {}", e);
                    *self.running.borrow_mut() = false;
                    Err(JsValue::from_str(&format!("Emulation error: {}", e)))
                }
            }
        } else {
            self.render_test_pattern()?;
            Ok(())
        }
    }
    
    fn update_input(&self, psx: &mut Psx) {
        let keys = self.input_state.keys.borrow();
        let gamepad_buttons = self.input_state.gamepad_buttons.borrow();
        
        let mut pad_state = 0u16;
        
        if keys[38] || gamepad_buttons[12] { pad_state |= 0x0010; } // Up
        if keys[40] || gamepad_buttons[13] { pad_state |= 0x0040; } // Down
        if keys[37] || gamepad_buttons[14] { pad_state |= 0x0080; } // Left
        if keys[39] || gamepad_buttons[15] { pad_state |= 0x0020; } // Right
        if keys[88] || gamepad_buttons[0] { pad_state |= 0x4000; } // X
        if keys[90] || gamepad_buttons[1] { pad_state |= 0x2000; } // Z
        if keys[83] || gamepad_buttons[2] { pad_state |= 0x8000; } // S
        if keys[65] || gamepad_buttons[3] { pad_state |= 0x1000; } // A
        if keys[81] || gamepad_buttons[4] { pad_state |= 0x0004; } // Q
        if keys[87] || gamepad_buttons[5] { pad_state |= 0x0008; } // W
        if keys[69] || gamepad_buttons[6] { pad_state |= 0x0001; } // E
        if keys[82] || gamepad_buttons[7] { pad_state |= 0x0002; } // R
        if keys[13] || gamepad_buttons[9] { pad_state |= 0x0800; } // Enter
        if keys[16] || gamepad_buttons[8] { pad_state |= 0x0100; } // Shift
        
        psx.set_controller_state(0, pad_state);
    }

    fn render_frame(&mut self, psx: &mut Psx) -> Result<(), JsValue> {
        let (width, height) = psx.get_display_size();
        
        if self.canvas.width() != width || self.canvas.height() != height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
        }
        
        psx.get_framebuffer(&mut self.frame_buffer);
        
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&self.frame_buffer),
            width,
            height,
        )?;
        
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        
        Ok(())
    }
    
    fn render_test_pattern(&mut self) -> Result<(), JsValue> {
        let width = 320u32;
        let height = 240u32;
        
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        
        self.frame_buffer.clear();
        self.frame_buffer.resize((width * height * 4) as usize, 0);
        
        let offset = (self.frame_count * 2) as u8;
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                self.frame_buffer[idx] = ((x as u8).wrapping_add(offset));
                self.frame_buffer[idx + 1] = ((y as u8).wrapping_add(offset / 2));
                self.frame_buffer[idx + 2] = offset;
                self.frame_buffer[idx + 3] = 255;
            }
        }
        
        self.frame_count += 1;
        
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&self.frame_buffer),
            width,
            height,
        )?;
        
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        
        Ok(())
    }

    fn process_audio(&mut self, psx: &mut Psx) -> Result<(), JsValue> {
        let samples = psx.get_audio_samples();
        
        if !samples.is_empty() && self.audio_context.is_some() {
            self.audio_buffer.clear();
            for &sample in samples.iter() {
                let normalized = (sample as f32) / 32768.0;
                self.audio_buffer.push(normalized);
            }
        }
        
        Ok(())
    }

    pub fn start(&mut self) {
        *self.running.borrow_mut() = true;
        info!("Emulator started");
    }

    pub fn stop(&mut self) {
        *self.running.borrow_mut() = false;
        info!("Emulator stopped");
    }

    pub fn is_running(&self) -> bool {
        *self.running.borrow()
    }

    pub fn handle_keyboard_event(&mut self, event: KeyboardEvent, pressed: bool) {
        let keycode = event.key_code();
        self.input_state.set_key(keycode, pressed);
        
        if pressed {
            debug!("Key pressed: {} (code: {})", event.key(), keycode);
        }
    }

    pub fn get_frame_buffer(&self) -> Vec<u8> {
        self.frame_buffer.clone()
    }

    pub fn get_audio_buffer(&self) -> Vec<f32> {
        self.audio_buffer.clone()
    }
}

// Stub for JsError
#[wasm_bindgen]
pub struct JsError {
    message: String,
}

#[wasm_bindgen]
impl JsError {
    pub fn new(message: String) -> Self {
        JsError { message }
    }
    
    pub fn message(&self) -> String {
        self.message.clone()
    }
}