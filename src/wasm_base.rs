//! Base trait and common functionality for WASM emulator implementations
//!
//! This module provides a common foundation for all WASM emulator variants,
//! eliminating code duplication and providing a consistent interface.

use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, AudioContext};
use crate::psx::memory_map::{self, controller};
use std::rc::Rc;
use std::cell::RefCell;

// ============================================================================
// Error Handling
// ============================================================================

/// Common error type for WASM operations
#[derive(Debug, Clone)]
pub enum WasmError {
    InitializationError(String),
    EmulationError(String),
    RenderingError(String),
    AudioError(String),
    InputError(String),
    MemoryError(String),
}

impl WasmError {
    pub fn to_js_value(&self) -> JsValue {
        JsValue::from_str(&format!("{:?}", self))
    }
}

/// Result type for WASM operations
pub type WasmResult<T> = Result<T, WasmError>;

/// Trait for converting errors to WASM errors
pub trait ToWasmError {
    fn to_wasm_error(self) -> WasmError;
}

impl<E: std::fmt::Display> ToWasmError for E {
    fn to_wasm_error(self) -> WasmError {
        WasmError::EmulationError(self.to_string())
    }
}

// ============================================================================
// Base Emulator Trait
// ============================================================================

/// Base trait for all WASM emulator implementations
pub trait WasmEmulator {
    /// Initialize the emulator with BIOS
    fn initialize(&mut self, bios_data: &[u8]) -> WasmResult<()>;
    
    /// Load a game (disc image or executable)
    fn load_game(&mut self, data: &[u8], format: GameFormat) -> WasmResult<()>;
    
    /// Execute one frame of emulation
    fn run_frame(&mut self) -> WasmResult<()>;
    
    /// Update controller input state
    fn update_input(&mut self, controller_id: u8, state: ControllerState) -> WasmResult<()>;
    
    /// Render the current frame to canvas
    fn render(&self, ctx: &CanvasRenderingContext2d) -> WasmResult<()>;
    
    /// Get audio samples for the current frame
    fn get_audio_samples(&mut self) -> Vec<f32>;
    
    /// Reset the emulator
    fn reset(&mut self) -> WasmResult<()>;
    
    /// Save state
    fn save_state(&self) -> WasmResult<Vec<u8>>;
    
    /// Load state
    fn load_state(&mut self, state: &[u8]) -> WasmResult<()>;
}

// ============================================================================
// Game Format Support
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameFormat {
    PsxExe,
    CueBin,
    Iso,
    Chd,
}

impl GameFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "exe" | "psx" => Some(GameFormat::PsxExe),
            "cue" => Some(GameFormat::CueBin),
            "iso" | "bin" => Some(GameFormat::Iso),
            "chd" => Some(GameFormat::Chd),
            _ => None,
        }
    }
}

// ============================================================================
// Controller Input
// ============================================================================

/// Controller state with proper button mapping
#[derive(Debug, Clone, Default)]
pub struct ControllerState {
    pub buttons: u16,
    pub left_stick_x: u8,
    pub left_stick_y: u8,
    pub right_stick_x: u8,
    pub right_stick_y: u8,
}

impl ControllerState {
    pub fn new() -> Self {
        Self {
            buttons: 0,
            left_stick_x: 128,
            left_stick_y: 128,
            right_stick_x: 128,
            right_stick_y: 128,
        }
    }
    
    pub fn set_button(&mut self, button: ControllerButton, pressed: bool) {
        let mask = button.to_mask();
        if pressed {
            self.buttons |= mask;
        } else {
            self.buttons &= !mask;
        }
    }
    
    pub fn is_button_pressed(&self, button: ControllerButton) -> bool {
        (self.buttons & button.to_mask()) != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerButton {
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    Triangle,
    Circle,
    Cross,
    Square,
    L1,
    L2,
    R1,
    R2,
    Select,
    Start,
    L3,
    R3,
}

impl ControllerButton {
    pub fn to_mask(self) -> u16 {
        use controller::*;
        match self {
            Self::DpadUp => DPAD_UP,
            Self::DpadDown => DPAD_DOWN,
            Self::DpadLeft => DPAD_LEFT,
            Self::DpadRight => DPAD_RIGHT,
            Self::Triangle => BUTTON_TRIANGLE,
            Self::Circle => BUTTON_CIRCLE,
            Self::Cross => BUTTON_CROSS,
            Self::Square => BUTTON_SQUARE,
            Self::L1 => BUTTON_L1,
            Self::L2 => BUTTON_L2,
            Self::R1 => BUTTON_R1,
            Self::R2 => BUTTON_R2,
            Self::Select => BUTTON_SELECT,
            Self::Start => BUTTON_START,
            Self::L3 => BUTTON_L3,
            Self::R3 => BUTTON_R3,
        }
    }
    
    pub fn from_keyboard_code(code: &str) -> Option<Self> {
        match code {
            "ArrowUp" | "KeyW" => Some(Self::DpadUp),
            "ArrowDown" | "KeyS" => Some(Self::DpadDown),
            "ArrowLeft" | "KeyA" => Some(Self::DpadLeft),
            "ArrowRight" | "KeyD" => Some(Self::DpadRight),
            "KeyI" => Some(Self::Triangle),
            "KeyL" => Some(Self::Circle),
            "KeyK" => Some(Self::Cross),
            "KeyJ" => Some(Self::Square),
            "KeyQ" => Some(Self::L1),
            "KeyE" => Some(Self::R1),
            "Digit1" => Some(Self::L2),
            "Digit3" => Some(Self::R2),
            "ShiftLeft" | "ShiftRight" => Some(Self::Select),
            "Enter" => Some(Self::Start),
            _ => None,
        }
    }
    
    pub fn from_gamepad_button(button: u32) -> Option<Self> {
        match button {
            12 => Some(Self::DpadUp),
            13 => Some(Self::DpadDown),
            14 => Some(Self::DpadLeft),
            15 => Some(Self::DpadRight),
            3 => Some(Self::Triangle),
            1 => Some(Self::Circle),
            0 => Some(Self::Cross),
            2 => Some(Self::Square),
            4 => Some(Self::L1),
            6 => Some(Self::L2),
            5 => Some(Self::R1),
            7 => Some(Self::R2),
            8 => Some(Self::Select),
            9 => Some(Self::Start),
            10 => Some(Self::L3),
            11 => Some(Self::R3),
            _ => None,
        }
    }
}

// ============================================================================
// Common Rendering Functions
// ============================================================================

pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>,
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width * height) as usize],
        }
    }
    
    pub fn clear(&mut self, color: u32) {
        self.data.fill(color);
    }
    
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            self.data[(y * self.width + x) as usize] = color;
        }
    }
    
    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if x < self.width && y < self.height {
            self.data[(y * self.width + x) as usize]
        } else {
            0
        }
    }
    
    pub fn render_to_canvas(&self, ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
        let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&self.to_rgba_bytes()),
            self.width,
            self.height,
        )?;
        
        ctx.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }
    
    fn to_rgba_bytes(&self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(self.data.len() * 4);
        for &pixel in &self.data {
            rgba.push((pixel >> 16) as u8); // R
            rgba.push((pixel >> 8) as u8);  // G
            rgba.push(pixel as u8);         // B
            rgba.push(255);                 // A
        }
        rgba
    }
}

// ============================================================================
// Audio Processing
// ============================================================================

pub struct AudioProcessor {
    sample_rate: u32,
    buffer: Vec<f32>,
    resampler: Option<AudioResampler>,
}

impl AudioProcessor {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            buffer: Vec::with_capacity(4096),
            resampler: None,
        }
    }
    
    pub fn set_output_rate(&mut self, output_rate: u32) {
        if output_rate != self.sample_rate {
            self.resampler = Some(AudioResampler::new(self.sample_rate, output_rate));
        }
    }
    
    pub fn push_samples(&mut self, samples: &[i16]) {
        for &sample in samples {
            self.buffer.push(sample as f32 / 32768.0);
        }
    }
    
    pub fn get_output_samples(&mut self) -> Vec<f32> {
        if let Some(resampler) = &mut self.resampler {
            resampler.resample(&self.buffer)
        } else {
            self.buffer.clone()
        }
    }
    
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

struct AudioResampler {
    input_rate: u32,
    output_rate: u32,
    accumulator: f32,
}

impl AudioResampler {
    fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            input_rate,
            output_rate,
            accumulator: 0.0,
        }
    }
    
    fn resample(&mut self, input: &[f32]) -> Vec<f32> {
        let ratio = self.input_rate as f32 / self.output_rate as f32;
        let output_len = (input.len() as f32 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);
        
        for i in 0..output_len {
            let pos = i as f32 * ratio;
            let index = pos as usize;
            let frac = pos - index as f32;
            
            if index + 1 < input.len() {
                let sample = input[index] * (1.0 - frac) + input[index + 1] * frac;
                output.push(sample);
            } else if index < input.len() {
                output.push(input[index]);
            }
        }
        
        output
    }
}

// ============================================================================
// Memory Management Helpers
// ============================================================================

/// Helper for managing emulator memory regions
pub struct MemoryManager {
    ram: Vec<u8>,
    bios: Vec<u8>,
    scratchpad: Vec<u8>,
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            ram: vec![0; memory_map::RAM_SIZE_DEFAULT as usize],
            bios: vec![0; memory_map::BIOS_SIZE as usize],
            scratchpad: vec![0; memory_map::SCRATCHPAD_SIZE as usize],
        }
    }
    
    pub fn load_bios(&mut self, data: &[u8]) -> WasmResult<()> {
        if data.len() != memory_map::BIOS_SIZE as usize {
            return Err(WasmError::InitializationError(
                format!("Invalid BIOS size: expected {}, got {}", 
                    memory_map::BIOS_SIZE, data.len())
            ));
        }
        self.bios.copy_from_slice(data);
        Ok(())
    }
    
    pub fn read32(&self, addr: u32) -> u32 {
        let physical = memory_map::to_physical_address(addr);
        
        match physical {
            a if a < memory_map::RAM_SIZE_DEFAULT => {
                let offset = (a & memory_map::RAM_MASK_2MB) as usize;
                u32::from_le_bytes([
                    self.ram[offset],
                    self.ram[offset + 1],
                    self.ram[offset + 2],
                    self.ram[offset + 3],
                ])
            }
            a if a >= memory_map::BIOS_PHYSICAL_ADDR && 
                 a < memory_map::BIOS_PHYSICAL_ADDR + memory_map::BIOS_SIZE => {
                let offset = (a - memory_map::BIOS_PHYSICAL_ADDR) as usize;
                u32::from_le_bytes([
                    self.bios[offset],
                    self.bios[offset + 1],
                    self.bios[offset + 2],
                    self.bios[offset + 3],
                ])
            }
            a if a >= memory_map::SCRATCHPAD_ADDR && 
                 a < memory_map::SCRATCHPAD_ADDR + memory_map::SCRATCHPAD_SIZE => {
                let offset = (a - memory_map::SCRATCHPAD_ADDR) as usize;
                u32::from_le_bytes([
                    self.scratchpad[offset],
                    self.scratchpad[offset + 1],
                    self.scratchpad[offset + 2],
                    self.scratchpad[offset + 3],
                ])
            }
            _ => 0xffffffff, // Open bus
        }
    }
    
    pub fn write32(&mut self, addr: u32, value: u32) {
        let physical = memory_map::to_physical_address(addr);
        
        match physical {
            a if a < memory_map::RAM_SIZE_DEFAULT => {
                let offset = (a & memory_map::RAM_MASK_2MB) as usize;
                let bytes = value.to_le_bytes();
                self.ram[offset..offset + 4].copy_from_slice(&bytes);
            }
            a if a >= memory_map::SCRATCHPAD_ADDR && 
                 a < memory_map::SCRATCHPAD_ADDR + memory_map::SCRATCHPAD_SIZE => {
                let offset = (a - memory_map::SCRATCHPAD_ADDR) as usize;
                let bytes = value.to_le_bytes();
                self.scratchpad[offset..offset + 4].copy_from_slice(&bytes);
            }
            _ => {} // Ignore writes to other regions
        }
    }
}

// ============================================================================
// JavaScript Console Logging
// ============================================================================

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
    
    #[wasm_bindgen(js_namespace = console)]
    pub fn error(s: &str);
    
    #[wasm_bindgen(js_namespace = console)]
    pub fn warn(s: &str);
}

#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => {
        $crate::wasm_base::log(&format!($($t)*))
    };
}

#[macro_export]
macro_rules! console_error {
    ($($t:tt)*) => {
        $crate::wasm_base::error(&format!($($t)*))
    };
}

#[macro_export]
macro_rules! console_warn {
    ($($t:tt)*) => {
        $crate::wasm_base::warn(&format!($($t)*))
    };
}

// ============================================================================
// Performance Monitoring
// ============================================================================

pub struct PerformanceMonitor {
    frame_times: Vec<f64>,
    last_frame_time: f64,
    fps: f64,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            frame_times: Vec::with_capacity(60),
            last_frame_time: 0.0,
            fps: 0.0,
        }
    }
    
    pub fn frame_start(&mut self, timestamp: f64) {
        if self.last_frame_time > 0.0 {
            let frame_time = timestamp - self.last_frame_time;
            self.frame_times.push(frame_time);
            
            if self.frame_times.len() > 60 {
                self.frame_times.remove(0);
            }
            
            if self.frame_times.len() >= 30 {
                let avg_frame_time = self.frame_times.iter().sum::<f64>() 
                    / self.frame_times.len() as f64;
                self.fps = 1000.0 / avg_frame_time;
            }
        }
        
        self.last_frame_time = timestamp;
    }
    
    pub fn get_fps(&self) -> f64 {
        self.fps
    }
    
    pub fn get_frame_time(&self) -> f64 {
        if !self.frame_times.is_empty() {
            self.frame_times.last().copied().unwrap_or(0.0)
        } else {
            0.0
        }
    }
}