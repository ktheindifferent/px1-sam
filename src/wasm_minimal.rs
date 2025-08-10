// Minimal WASM build that compiles without cdimage dependency
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, 
    KeyboardEvent, Gamepad
};
use std::cell::RefCell;

#[cfg(feature = "console_error_panic_hook")]
use console_error_panic_hook;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

#[allow(unused_macros)]
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub struct PsxEmulator {
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

        // Get canvas element
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document.get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str("Canvas not found"))?;
        
        let _canvas: HtmlCanvasElement = canvas
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| JsValue::from_str("Element is not a canvas"))?;
            
        let _context = _canvas
            .get_context("2d")
            .map_err(|_| JsValue::from_str("Failed to get 2D context"))?
            .ok_or_else(|| JsValue::from_str("2D context is null"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| JsValue::from_str("Failed to cast to 2D context"))?;

        Ok(PsxEmulator {
            frame_buffer: vec![0; 1024 * 512 * 4], // RGBA buffer
            audio_buffer: vec![0.0; 44100 * 2], // Stereo audio buffer
            input_state: InputState::new(),
            running: RefCell::new(false),
        })
    }

    pub fn load_bios(&mut self, _bios_data: &[u8]) -> Result<(), JsValue> {
        // Stub: Would load BIOS data
        Ok(())
    }

    pub fn load_game(&mut self, _game_data: &[u8]) -> Result<(), JsValue> {
        // Stub: Would load game data
        Ok(())
    }

    pub fn run_frame(&mut self) -> Result<(), JsValue> {
        // Stub: Would run one frame of emulation
        Ok(())
    }

    pub fn start(&mut self) {
        *self.running.borrow_mut() = true;
    }

    pub fn stop(&mut self) {
        *self.running.borrow_mut() = false;
    }

    pub fn is_running(&self) -> bool {
        *self.running.borrow()
    }

    pub fn handle_keyboard_event(&mut self, event: KeyboardEvent, pressed: bool) {
        let keycode = event.key_code();
        self.input_state.set_key(keycode, pressed);
    }

    pub fn get_frame_buffer(&self) -> Vec<u8> {
        self.frame_buffer.clone()
    }

    pub fn get_audio_buffer(&self) -> Vec<f32> {
        self.audio_buffer.clone()
    }
}

// Provide a stub for JsError since we don't have the full error module
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