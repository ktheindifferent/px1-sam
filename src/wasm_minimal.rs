use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent,
    AudioContext, Gamepad
};
use std::cell::RefCell;

// We can't use the full PSX modules due to dependencies, so we'll create stubs
// This file serves as the interface between JavaScript and the emulator

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

// Simplified internal PSX state for WASM
struct PsxCore {
    bios_loaded: bool,
    game_loaded: bool,
    ram: Vec<u8>,
    vram: Vec<u16>,
    frame_counter: u32,
}

impl PsxCore {
    fn new() -> Self {
        PsxCore {
            bios_loaded: false,
            game_loaded: false,
            ram: vec![0; 2 * 1024 * 1024], // 2MB RAM
            vram: vec![0; 1024 * 512],      // 1024x512 VRAM
            frame_counter: 0,
        }
    }
    
    fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), String> {
        if bios_data.len() != 512 * 1024 {
            return Err("Invalid BIOS size - must be 512KB".to_string());
        }
        self.bios_loaded = true;
        console_log!("BIOS loaded (size: {} bytes)", bios_data.len());
        Ok(())
    }
    
    fn load_game(&mut self, game_data: &[u8]) -> Result<(), String> {
        if !self.bios_loaded {
            return Err("BIOS must be loaded first".to_string());
        }
        
        // Check for PSX-EXE header
        if game_data.len() > 8 && &game_data[0..8] == b"PS-X EXE" {
            console_log!("PSX-EXE detected (size: {} bytes)", game_data.len());
            self.game_loaded = true;
            Ok(())
        } else {
            // Assume it's a raw binary or ISO
            console_log!("Game data loaded (size: {} bytes)", game_data.len());
            self.game_loaded = true;
            Ok(())
        }
    }
    
    fn run_frame(&mut self) -> Result<(), String> {
        if !self.bios_loaded {
            return Err("BIOS not loaded".to_string());
        }
        
        self.frame_counter += 1;
        
        // Generate a test pattern in VRAM for now
        // This creates a gradient that changes over time
        let pattern_offset = (self.frame_counter * 10) as u16;
        for y in 0..240 {
            for x in 0..320 {
                let idx = y * 1024 + x;
                // Create a moving gradient pattern
                let r = ((x as u16 + pattern_offset) / 10) & 0x1F;
                let g = ((y as u16 + pattern_offset / 2) / 8) & 0x1F;
                let b = ((self.frame_counter as u16) / 4) & 0x1F;
                self.vram[idx] = (b << 10) | (g << 5) | r;
            }
        }
        
        Ok(())
    }
    
    fn get_framebuffer(&self, buffer: &mut Vec<u8>) {
        buffer.clear();
        
        // Convert VRAM to RGBA for canvas
        // We'll display a 320x240 area for now
        for y in 0..240 {
            for x in 0..320 {
                let pixel = self.vram[y * 1024 + x];
                
                // Convert 15-bit RGB to 32-bit RGBA
                let r = ((pixel & 0x1F) << 3) as u8;
                let g = (((pixel >> 5) & 0x1F) << 3) as u8;
                let b = (((pixel >> 10) & 0x1F) << 3) as u8;
                
                buffer.push(r);
                buffer.push(g);
                buffer.push(b);
                buffer.push(255);
            }
        }
    }
}

#[wasm_bindgen]
pub struct PsxEmulator {
    psx: PsxCore,
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    audio_context: Option<AudioContext>,
    frame_buffer: Vec<u8>,
    audio_buffer: Vec<f32>,
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

        // Set canvas size
        canvas.set_width(320);
        canvas.set_height(240);

        let audio_context = AudioContext::new().ok();
        
        console_log!("PSX Emulator initialized");
        
        Ok(PsxEmulator {
            psx: PsxCore::new(),
            canvas,
            context,
            audio_context,
            frame_buffer: Vec::with_capacity(320 * 240 * 4),
            audio_buffer: Vec::with_capacity(2048),
            input_state: InputState::new(),
            running: RefCell::new(false),
        })
    }

    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), JsValue> {
        self.psx.load_bios(bios_data)
            .map_err(|e| JsValue::from_str(&e))?;
        console_log!("BIOS loaded successfully");
        Ok(())
    }

    pub fn load_game(&mut self, game_data: &[u8]) -> Result<(), JsValue> {
        self.psx.load_game(game_data)
            .map_err(|e| JsValue::from_str(&e))?;
        console_log!("Game loaded successfully");
        Ok(())
    }

    pub fn run_frame(&mut self) -> Result<(), JsValue> {
        if !self.is_running() {
            return Ok(());
        }

        // Run emulation
        self.psx.run_frame()
            .map_err(|e| JsValue::from_str(&e))?;
        
        // Get framebuffer and render
        self.psx.get_framebuffer(&mut self.frame_buffer);
        
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&self.frame_buffer),
            320,
            240,
        )?;
        
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        
        // Audio processing would go here
        // For now, we'll skip audio to avoid web-sys API issues
        
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
            console_log!("Key pressed: {}", keycode);
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