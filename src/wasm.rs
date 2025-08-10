use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent, MouseEvent,
    AudioContext, AudioBuffer, AudioBufferSourceNode, GainNode, Gamepad
};
use std::cell::RefCell;
use std::rc::Rc;

mod psx;
mod memory_card;
mod error;
mod bitwise;
mod box_array;

use psx::Psx;

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
    psx: Psx,
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

        let audio_context = AudioContext::new().ok();
        
        let psx = Psx::new();
        
        Ok(PsxEmulator {
            psx,
            canvas,
            context,
            audio_context,
            frame_buffer: vec![0; 640 * 480 * 4],
            audio_buffer: Vec::with_capacity(2048),
            input_state: InputState::new(),
            running: RefCell::new(false),
        })
    }

    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), JsValue> {
        self.psx.load_bios(bios_data)
            .map_err(|e| JsValue::from_str(&format!("Failed to load BIOS: {:?}", e)))
    }

    pub fn load_game(&mut self, game_data: &[u8]) -> Result<(), JsValue> {
        self.psx.load_disc(game_data)
            .map_err(|e| JsValue::from_str(&format!("Failed to load game: {:?}", e)))
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

    pub fn run_frame(&mut self) -> Result<(), JsValue> {
        if !self.is_running() {
            return Ok(());
        }

        self.update_input();
        
        self.psx.run_frame()
            .map_err(|e| JsValue::from_str(&format!("Frame execution error: {:?}", e)))?;
        
        self.render_frame()?;
        self.process_audio()?;
        
        Ok(())
    }

    fn update_input(&mut self) {
        let keys = self.input_state.keys.borrow();
        let gamepad_buttons = self.input_state.gamepad_buttons.borrow();
        
        let mut pad_state = 0u16;
        
        if keys[38] || gamepad_buttons[12] { pad_state |= 0x0010; }
        if keys[40] || gamepad_buttons[13] { pad_state |= 0x0040; }
        if keys[37] || gamepad_buttons[14] { pad_state |= 0x0080; }
        if keys[39] || gamepad_buttons[15] { pad_state |= 0x0020; }
        
        if keys[88] || gamepad_buttons[0] { pad_state |= 0x4000; }
        if keys[90] || gamepad_buttons[1] { pad_state |= 0x2000; }
        if keys[83] || gamepad_buttons[2] { pad_state |= 0x8000; }
        if keys[65] || gamepad_buttons[3] { pad_state |= 0x1000; }
        
        if keys[81] || gamepad_buttons[4] { pad_state |= 0x0004; }
        if keys[87] || gamepad_buttons[5] { pad_state |= 0x0008; }
        if keys[69] || gamepad_buttons[6] { pad_state |= 0x0001; }
        if keys[82] || gamepad_buttons[7] { pad_state |= 0x0002; }
        
        if keys[13] || gamepad_buttons[9] { pad_state |= 0x0800; }
        if keys[16] || gamepad_buttons[8] { pad_state |= 0x0100; }
        
        self.psx.set_controller_state(0, pad_state);
    }

    fn render_frame(&mut self) -> Result<(), JsValue> {
        let (width, height) = self.psx.get_display_size();
        
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        
        self.psx.get_framebuffer(&mut self.frame_buffer);
        
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&self.frame_buffer),
            width,
            height,
        )?;
        
        self.context.put_image_data(&image_data, 0.0, 0.0)?;
        
        Ok(())
    }

    fn process_audio(&mut self) -> Result<(), JsValue> {
        if let Some(ref audio_ctx) = self.audio_context {
            self.psx.get_audio_samples(&mut self.audio_buffer);
            
            if !self.audio_buffer.is_empty() {
                let buffer = audio_ctx.create_buffer(
                    2,
                    self.audio_buffer.len() as u32 / 2,
                    44100.0,
                )?;
                
                let mut left_channel = vec![0.0f32; self.audio_buffer.len() / 2];
                let mut right_channel = vec![0.0f32; self.audio_buffer.len() / 2];
                
                for (i, chunk) in self.audio_buffer.chunks(2).enumerate() {
                    left_channel[i] = chunk[0];
                    right_channel[i] = chunk[1];
                }
                
                buffer.copy_to_channel(&left_channel, 0)?;
                buffer.copy_to_channel(&right_channel, 1)?;
                
                let source = audio_ctx.create_buffer_source()?;
                source.set_buffer(Some(&buffer));
                source.connect_with_audio_node(&audio_ctx.destination())?;
                source.start()?;
                
                self.audio_buffer.clear();
            }
        }
        
        Ok(())
    }

    pub fn set_input(&mut self, input_state: InputState) {
        self.input_state = input_state;
    }

    pub fn get_save_state(&self) -> Vec<u8> {
        self.psx.serialize_state()
    }

    pub fn load_save_state(&mut self, state_data: &[u8]) -> Result<(), JsValue> {
        self.psx.deserialize_state(state_data)
            .map_err(|e| JsValue::from_str(&format!("Failed to load save state: {:?}", e)))
    }
}