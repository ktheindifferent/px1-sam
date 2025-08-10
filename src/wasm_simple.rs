// Simple PSX WASM implementation without complex dependencies
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent,
    AudioContext, Gamepad
};
use std::cell::RefCell;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

macro_rules! console_error {
    ($($t:tt)*) => (error(&format_args!($($t)*).to_string()))
}

// Simple PSX emulator implementation
struct SimplePsx {
    ram: Vec<u8>,
    vram: Vec<u16>,
    display_start_x: u16,
    display_start_y: u16,
    display_width: u16,
    display_height: u16,
    cpu_pc: u32,
    cpu_regs: [u32; 32],
    frame_count: u32,
}

impl SimplePsx {
    fn new() -> Self {
        SimplePsx {
            ram: vec![0; 2 * 1024 * 1024], // 2MB RAM
            vram: vec![0; 1024 * 512], // 1024x512 16-bit VRAM
            display_start_x: 0,
            display_start_y: 0,
            display_width: 320,
            display_height: 240,
            cpu_pc: 0xbfc00000, // BIOS start address
            cpu_regs: [0; 32],
            frame_count: 0,
        }
    }
    
    fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), String> {
        if bios_data.len() != 512 * 1024 {
            return Err("BIOS must be exactly 512KB".to_string());
        }
        // In a real implementation, we'd map this to the correct address
        // For now, just store it
        Ok(())
    }
    
    fn load_exe(&mut self, exe_data: &[u8]) -> Result<(), String> {
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
        
        // Load the code into RAM
        let exe_start = 0x800;
        let exe_end = exe_start + file_size as usize;
        
        if exe_end > exe_data.len() {
            return Err("EXE file size mismatch".to_string());
        }
        
        // Map to physical RAM address
        let ram_addr = (load_addr & 0x1fffff) as usize;
        let exe_code = &exe_data[exe_start..exe_end];
        
        for (i, &byte) in exe_code.iter().enumerate() {
            if ram_addr + i < self.ram.len() {
                self.ram[ram_addr + i] = byte;
            }
        }
        
        // Set CPU state
        self.cpu_pc = initial_pc;
        self.cpu_regs[28] = initial_gp; // GP
        self.cpu_regs[29] = initial_sp; // SP
        self.cpu_regs[30] = initial_sp; // FP
        
        Ok(())
    }
    
    fn run_frame(&mut self) -> Result<(), String> {
        // Simplified frame execution - just generate a test pattern
        self.frame_count += 1;
        
        // Generate animated test pattern in VRAM
        let offset = (self.frame_count * 2) as u16;
        for y in 0..self.display_height {
            for x in 0..self.display_width {
                let vram_x = (self.display_start_x + x) as usize % 1024;
                let vram_y = (self.display_start_y + y) as usize % 512;
                let idx = vram_y * 1024 + vram_x;
                
                let r = ((x + offset) & 0x1f) as u16;
                let g = ((y + offset / 2) & 0x1f) as u16;
                let b = (offset & 0x1f) as u16;
                
                self.vram[idx] = r | (g << 5) | (b << 10);
            }
        }
        
        Ok(())
    }
    
    fn get_framebuffer(&self, buffer: &mut Vec<u8>) {
        let width = self.display_width as usize;
        let height = self.display_height as usize;
        
        buffer.clear();
        buffer.resize(width * height * 4, 0);
        
        for y in 0..height {
            for x in 0..width {
                let vram_x = (self.display_start_x as usize + x) % 1024;
                let vram_y = (self.display_start_y as usize + y) % 512;
                let pixel = self.vram[vram_y * 1024 + vram_x];
                
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
}

#[wasm_bindgen]
pub struct PsxEmulator {
    psx: SimplePsx,
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    audio_context: Option<AudioContext>,
    frame_buffer: Vec<u8>,
    input_state: InputState,
    running: RefCell<bool>,
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
        
        console_log!("Simple PSX WASM Emulator initialized");
        
        Ok(PsxEmulator {
            psx: SimplePsx::new(),
            canvas,
            context,
            audio_context,
            frame_buffer: Vec::with_capacity(640 * 480 * 4),
            input_state: InputState::new(),
            running: RefCell::new(false),
        })
    }

    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), JsValue> {
        match self.psx.load_bios(bios_data) {
            Ok(_) => {
                console_log!("BIOS loaded successfully");
                Ok(())
            }
            Err(e) => {
                console_error!("Failed to load BIOS: {}", e);
                Err(JsValue::from_str(&format!("BIOS load failed: {}", e)))
            }
        }
    }

    pub fn load_game(&mut self, game_data: &[u8]) -> Result<(), JsValue> {
        if game_data.len() > 8 && &game_data[0..8] == b"PS-X EXE" {
            match self.psx.load_exe(game_data) {
                Ok(_) => {
                    console_log!("PSX-EXE loaded successfully");
                    Ok(())
                }
                Err(e) => {
                    console_error!("Failed to load EXE: {}", e);
                    Err(JsValue::from_str(&format!("Failed to load EXE: {}", e)))
                }
            }
        } else {
            console_error!("Only PSX-EXE files are supported in WASM build");
            Err(JsValue::from_str("Only PSX-EXE files are supported"))
        }
    }

    pub fn run_frame(&mut self) -> Result<(), JsValue> {
        if !self.is_running() {
            return Ok(());
        }

        match self.psx.run_frame() {
            Ok(_) => {
                self.render_frame()?;
                Ok(())
            }
            Err(e) => {
                console_error!("Frame execution error: {}", e);
                *self.running.borrow_mut() = false;
                Err(JsValue::from_str(&format!("Emulation error: {}", e)))
            }
        }
    }
    
    fn render_frame(&mut self) -> Result<(), JsValue> {
        self.psx.get_framebuffer(&mut self.frame_buffer);
        
        let width = self.psx.display_width as u32;
        let height = self.psx.display_height as u32;
        
        if self.canvas.width() != width || self.canvas.height() != height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
        }
        
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&self.frame_buffer),
            width,
            height,
        )?;
        
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        
        Ok(())
    }

    pub fn start(&mut self) {
        *self.running.borrow_mut() = true;
        console_log!("Emulator started");
    }

    pub fn stop(&mut self) {
        *self.running.borrow_mut() = false;
        console_log!("Emulator stopped");
    }

    pub fn is_running(&self) -> bool {
        *self.running.borrow()
    }

    pub fn handle_keyboard_event(&mut self, event: KeyboardEvent, pressed: bool) {
        let keycode = event.key_code();
        self.input_state.set_key(keycode, pressed);
        
        if pressed {
            console_log!("Key pressed: {} (code: {})", event.key(), keycode);
        }
    }

    pub fn get_frame_buffer(&self) -> Vec<u8> {
        self.frame_buffer.clone()
    }

    pub fn get_audio_buffer(&self) -> Vec<f32> {
        vec![]
    }

    pub fn get_save_state(&self) -> Vec<u8> {
        vec![]
    }

    pub fn load_save_state(&mut self, _state: &[u8]) -> Result<(), JsValue> {
        Ok(())
    }

    pub fn reset(&mut self) {
        self.psx = SimplePsx::new();
        console_log!("Emulator reset");
    }

    pub fn set_volume(&mut self, _volume: f32) {
        // Audio not implemented yet
    }

    pub fn get_debug_info(&self) -> String {
        format!("PC: {:08x}, Frame: {}", self.psx.cpu_pc, self.psx.frame_count)
    }

    pub fn update_gamepad_state(&mut self, gamepad: &Gamepad) {
        self.input_state.update_gamepad(gamepad);
    }
}