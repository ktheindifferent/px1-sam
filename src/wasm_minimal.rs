use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent,
    AudioContext, Gamepad
};
use std::cell::RefCell;

// Include our stub modules
mod cd_stub;
mod psx_wasm;

// Include the actual PSX modules
#[path = "psx/cpu.rs"]
mod cpu;
#[path = "psx/cop0.rs"]
mod cop0;
#[path = "psx/gpu/mod.rs"]
mod gpu;
#[path = "psx/gte/mod.rs"]
mod gte;
#[path = "psx/spu/mod.rs"]
mod spu;
#[path = "psx/dma.rs"]
mod dma;
#[path = "psx/timers.rs"]
mod timers;
#[path = "psx/irq.rs"]
mod irq;
#[path = "psx/pad_memcard/mod.rs"]
mod pad_memcard;
#[path = "psx/memory_control.rs"]
mod memory_control;
#[path = "psx/cache.rs"]
mod cache;
#[path = "psx/bios/mod.rs"]
mod bios;
#[path = "psx/xmem.rs"]
mod xmem;
#[path = "psx/sync.rs"]
mod sync;

// Helper modules
#[path = "memory_card.rs"]
mod memory_card;
#[path = "error.rs"]
mod error;
#[path = "bitwise.rs"]
mod bitwise;
#[path = "box_array.rs"]
mod box_array;

use psx_wasm::{Psx, PsxError};

#[cfg(feature = "console_error_panic_hook")]
use console_error_panic_hook;

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

        // Set initial canvas size
        canvas.set_width(640);
        canvas.set_height(480);

        let audio_context = AudioContext::new().ok();
        
        console_log!("PSX WASM Emulator initialized");
        
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
        // Validate and create BIOS
        let bios = match bios::Bios::new(bios_data) {
            Ok(b) => b,
            Err(e) => {
                console_error!("Failed to load BIOS: {:?}", e);
                return Err(JsValue::from_str(&format!("Invalid BIOS: {:?}", e)));
            }
        };
        
        // Create PSX instance
        match Psx::new_without_disc(bios) {
            Ok(psx) => {
                self.psx = Some(psx);
                console_log!("PSX initialized with BIOS");
                Ok(())
            }
            Err(e) => {
                console_error!("Failed to initialize PSX: {:?}", e);
                Err(JsValue::from_str(&format!("PSX init failed: {:?}", e)))
            }
        }
    }

    pub fn load_game(&mut self, game_data: &[u8]) -> Result<(), JsValue> {
        if let Some(ref mut psx) = self.psx {
            // Try to load as PSX-EXE
            if game_data.len() > 8 && &game_data[0..8] == b"PS-X EXE" {
                match psx.load_exe(game_data) {
                    Ok(_) => {
                        console_log!("PSX-EXE loaded successfully");
                        Ok(())
                    }
                    Err(e) => {
                        console_error!("Failed to load EXE: {:?}", e);
                        Err(JsValue::from_str(&format!("Failed to load EXE: {:?}", e)))
                    }
                }
            } else {
                // For now, we only support PSX-EXE files
                console_error!("Only PSX-EXE files are supported in WASM build");
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
            // Update input
            self.update_input(psx);
            
            // Run emulation for one frame
            match psx.run_frame() {
                Ok(_) => {
                    self.frame_count += 1;
                    
                    // Render frame
                    self.render_frame(psx)?;
                    
                    // Process audio
                    self.process_audio(psx)?;
                    
                    // Log frame count every second
                    if self.frame_count % 60 == 0 {
                        console_log!("Frame {}: Emulation running", self.frame_count);
                    }
                    
                    Ok(())
                }
                Err(e) => {
                    console_error!("Frame execution error: {:?}", e);
                    *self.running.borrow_mut() = false;
                    Err(JsValue::from_str(&format!("Emulation error: {:?}", e)))
                }
            }
        } else {
            // No PSX loaded - show test pattern
            self.render_test_pattern()?;
            Ok(())
        }
    }
    
    fn update_input(&self, psx: &mut Psx) {
        let keys = self.input_state.keys.borrow();
        let gamepad_buttons = self.input_state.gamepad_buttons.borrow();
        
        let mut pad_state = 0u16;
        
        // D-Pad
        if keys[38] || gamepad_buttons[12] { pad_state |= 0x0010; } // Up
        if keys[40] || gamepad_buttons[13] { pad_state |= 0x0040; } // Down
        if keys[37] || gamepad_buttons[14] { pad_state |= 0x0080; } // Left
        if keys[39] || gamepad_buttons[15] { pad_state |= 0x0020; } // Right
        
        // Face buttons
        if keys[88] || gamepad_buttons[0] { pad_state |= 0x4000; } // X (Cross)
        if keys[90] || gamepad_buttons[1] { pad_state |= 0x2000; } // Z (Circle)
        if keys[83] || gamepad_buttons[2] { pad_state |= 0x8000; } // S (Square)
        if keys[65] || gamepad_buttons[3] { pad_state |= 0x1000; } // A (Triangle)
        
        // Shoulder buttons
        if keys[81] || gamepad_buttons[4] { pad_state |= 0x0004; } // Q (L1)
        if keys[87] || gamepad_buttons[5] { pad_state |= 0x0008; } // W (R1)
        if keys[69] || gamepad_buttons[6] { pad_state |= 0x0001; } // E (L2)
        if keys[82] || gamepad_buttons[7] { pad_state |= 0x0002; } // R (R2)
        
        // Start/Select
        if keys[13] || gamepad_buttons[9] { pad_state |= 0x0800; } // Enter (Start)
        if keys[16] || gamepad_buttons[8] { pad_state |= 0x0100; } // Shift (Select)
        
        psx.set_controller_state(0, pad_state);
    }

    fn render_frame(&mut self, psx: &mut Psx) -> Result<(), JsValue> {
        let (width, height) = psx.get_display_size();
        
        // Ensure canvas matches display size
        if self.canvas.width() != width || self.canvas.height() != height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
        }
        
        // Get framebuffer from PSX
        psx.get_framebuffer(&mut self.frame_buffer);
        
        // Create ImageData and render to canvas
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&self.frame_buffer),
            width,
            height,
        )?;
        
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        
        Ok(())
    }
    
    fn render_test_pattern(&mut self) -> Result<(), JsValue> {
        // Generate a test pattern when no PSX is loaded
        let width = 320u32;
        let height = 240u32;
        
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        
        self.frame_buffer.clear();
        self.frame_buffer.resize((width * height * 4) as usize, 0);
        
        // Create animated gradient
        let offset = (self.frame_count * 2) as u8;
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                self.frame_buffer[idx] = ((x as u8).wrapping_add(offset)); // R
                self.frame_buffer[idx + 1] = ((y as u8).wrapping_add(offset / 2)); // G
                self.frame_buffer[idx + 2] = offset; // B
                self.frame_buffer[idx + 3] = 255; // A
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
        // Get audio samples from SPU
        let samples = psx.get_audio_samples();
        
        if !samples.is_empty() && self.audio_context.is_some() {
            // Convert i16 samples to f32
            self.audio_buffer.clear();
            for &sample in samples.iter() {
                let normalized = (sample as f32) / 32768.0;
                self.audio_buffer.push(normalized);
            }
            
            // Audio playback would go here
            // Skipping for now to avoid web-sys API complexity
        }
        
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
        
        // Log key events for debugging
        if pressed {
            console_log!("Key pressed: {} (code: {})", event.key(), keycode);
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