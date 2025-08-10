use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent,
    AudioContext, Gamepad
};
use std::cell::RefCell;
use std::rc::Rc;

// Import the cd_stub module for disc support
#[path = "cd_stub.rs"]
mod cdimage;

#[path = "psx_stub.rs"]
mod psx;

#[path = "error_stub.rs"]
mod error;

#[path = "bitwise.rs"]
mod bitwise;

#[path = "box_array.rs"]
mod box_array;

use psx::Psx;

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
    disc_loaded: RefCell<bool>,
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct InputState {
    keys: Rc<RefCell<[bool; 256]>>,
    gamepad_buttons: Rc<RefCell<[bool; 20]>>,
    gamepad_axes: Rc<RefCell<[f32; 4]>>,
}

#[wasm_bindgen]
impl InputState {
    pub fn new() -> Self {
        InputState {
            keys: Rc::new(RefCell::new([false; 256])),
            gamepad_buttons: Rc::new(RefCell::new([false; 20])),
            gamepad_axes: Rc::new(RefCell::new([0.0; 4])),
        }
    }
    
    pub fn set_key(&self, keycode: u32, pressed: bool) {
        if (keycode as usize) < 256 {
            self.keys.borrow_mut()[keycode as usize] = pressed;
        }
    }
    
    pub fn set_gamepad_button(&self, button: u32, pressed: bool) {
        if (button as usize) < 20 {
            self.gamepad_buttons.borrow_mut()[button as usize] = pressed;
        }
    }
    
    pub fn set_gamepad_axis(&self, axis: u32, value: f32) {
        if (axis as usize) < 4 {
            self.gamepad_axes.borrow_mut()[axis as usize] = value;
        }
    }
}

// CUE file parser
struct CueParser;

impl CueParser {
    fn parse(cue_content: &str) -> std::result::Result<CueFile, String> {
        let mut cue_file = CueFile {
            bin_files: Vec::new(),
            tracks: Vec::new(),
        };
        
        let lines: Vec<&str> = cue_content.lines().collect();
        let mut current_bin = None;
        let mut current_track = None;
        
        for line in lines {
            let line = line.trim();
            
            if line.starts_with("FILE ") {
                // Parse FILE "filename.bin" BINARY
                let parts: Vec<&str> = line.split('"').collect();
                if parts.len() >= 2 {
                    let filename = parts[1].to_string();
                    current_bin = Some(filename.clone());
                    cue_file.bin_files.push(filename);
                }
            } else if line.starts_with("TRACK ") {
                // Parse TRACK 01 MODE2/2352
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let track_num = parts[1].parse::<u8>().unwrap_or(1);
                    let mode = parts[2].to_string();
                    
                    current_track = Some(Track {
                        number: track_num,
                        mode,
                        bin_file: current_bin.clone().unwrap_or_default(),
                        indices: Vec::new(),
                    });
                }
            } else if line.starts_with("INDEX ") {
                // Parse INDEX 01 00:00:00
                if let Some(ref mut track) = current_track {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let index_num = parts[1].parse::<u8>().unwrap_or(1);
                        let time = parts[2].to_string();
                        track.indices.push(Index {
                            number: index_num,
                            time,
                        });
                    }
                }
            } else if line.trim().is_empty() && current_track.is_some() {
                // End of track, add it to the cue file
                if let Some(track) = current_track.take() {
                    cue_file.tracks.push(track);
                }
            }
        }
        
        // Add the last track if any
        if let Some(track) = current_track {
            cue_file.tracks.push(track);
        }
        
        Ok(cue_file)
    }
}

struct CueFile {
    bin_files: Vec<String>,
    tracks: Vec<Track>,
}

struct Track {
    number: u8,
    mode: String,
    bin_file: String,
    indices: Vec<Index>,
}

struct Index {
    number: u8,
    time: String,
}

// Disc image implementation for CUE/BIN files
struct CueBinDisc {
    bin_data: Vec<u8>,
    cue_file: CueFile,
    sector_size: usize,
}

impl CueBinDisc {
    fn new(cue_content: &str, bin_data: Vec<u8>) -> std::result::Result<Self, String> {
        let cue_file = CueParser::parse(cue_content)?;
        
        // Determine sector size from track mode
        let sector_size = if !cue_file.tracks.is_empty() {
            let mode = &cue_file.tracks[0].mode;
            if mode.contains("2352") {
                2352
            } else if mode.contains("2048") {
                2048
            } else {
                2352 // Default to raw sector size
            }
        } else {
            2352
        };
        
        Ok(CueBinDisc {
            bin_data,
            cue_file,
            sector_size,
        })
    }
    
    fn read_sector(&self, sector_num: usize) -> Vec<u8> {
        let start = sector_num * self.sector_size;
        let end = start + self.sector_size;
        
        if end <= self.bin_data.len() {
            self.bin_data[start..end].to_vec()
        } else {
            vec![0; self.sector_size]
        }
    }
}

#[wasm_bindgen]
impl PsxEmulator {
    pub fn new(canvas: HtmlCanvasElement) -> std::result::Result<PsxEmulator, JsValue> {
        console_log!("Creating PSX emulator");
        
        let context = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("Failed to get 2D context"))?
            .dyn_into::<CanvasRenderingContext2d>()?;
        
        let psx = Psx::new().map_err(|e| {
            console_error!("Failed to create PSX: {:?}", e);
            JsValue::from_str(&format!("Failed to create PSX: {:?}", e))
        })?;
        
        let audio_context = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|_| AudioContext::new().ok());
        
        Ok(PsxEmulator {
            psx,
            canvas,
            context,
            audio_context,
            frame_buffer: vec![0; 640 * 480 * 4],
            audio_buffer: Vec::with_capacity(4096),
            input_state: InputState::new(),
            running: RefCell::new(false),
            disc_loaded: RefCell::new(false),
        })
    }
    
    pub fn load_bios(&mut self, bios_data: &[u8]) -> std::result::Result<(), JsValue> {
        if bios_data.is_empty() {
            console_error!("BIOS data is empty!");
            return Err(JsValue::from_str("BIOS data is empty"));
        }
        
        console_log!("Loading BIOS ({} bytes)", bios_data.len());
        
        self.psx.load_bios(bios_data).map_err(|e| {
            console_error!("Failed to load BIOS: {:?}", e);
            JsValue::from_str(&format!("Failed to load BIOS: {:?}", e))
        })?;
        
        console_log!("BIOS loaded successfully");
        Ok(())
    }
    
    pub fn load_game(&mut self, game_data: &[u8], filename: &str) -> std::result::Result<(), JsValue> {
        console_log!("Loading game: {} ({} bytes)", filename, game_data.len());
        
        // Check if it's a PSX-EXE file
        if game_data.len() > 8 && &game_data[0..8] == b"PS-X EXE" {
            console_log!("Detected PSX-EXE format");
            
            self.psx.load_exe(game_data).map_err(|e| {
                console_error!("Failed to load EXE: {:?}", e);
                JsValue::from_str(&format!("Failed to load EXE: {:?}", e))
            })?;
            
            console_log!("PSX-EXE loaded successfully");
            Ok(())
        }
        // Check if it's a CUE file
        else if filename.to_lowercase().ends_with(".cue") {
            console_log!("Detected CUE file format");
            
            // Parse CUE file content
            let cue_content = std::str::from_utf8(game_data).map_err(|e| {
                console_error!("Invalid CUE file encoding: {:?}", e);
                JsValue::from_str("Invalid CUE file format")
            })?;
            
            // For now, we'll return an informative error about needing the BIN file
            console_error!("CUE file detected. Please load the corresponding BIN file along with the CUE file.");
            Err(JsValue::from_str("CUE files need to be loaded with their corresponding BIN files. Please use loadGameWithBin() instead."))
        }
        // Check if it's a BIN file  
        else if filename.to_lowercase().ends_with(".bin") {
            console_log!("Detected BIN file format");
            
            // Create a simple disc from the BIN data
            // Assume it's a Mode2/2352 raw disc image
            self.load_bin_direct(game_data)?;
            
            console_log!("BIN file loaded successfully");
            Ok(())
        }
        else {
            console_error!("Unsupported file format: {}", filename);
            Err(JsValue::from_str(&format!("Unsupported file format. Supported formats: PSX-EXE, CUE/BIN")))
        }
    }
    
    pub fn load_game_with_bin(&mut self, cue_data: &[u8], bin_data: &[u8]) -> std::result::Result<(), JsValue> {
        console_log!("Loading CUE/BIN game ({} + {} bytes)", cue_data.len(), bin_data.len());
        
        // Parse CUE file
        let cue_content = std::str::from_utf8(cue_data).map_err(|e| {
            console_error!("Invalid CUE file encoding: {:?}", e);
            JsValue::from_str("Invalid CUE file format")
        })?;
        
        // Create disc from CUE/BIN
        let disc = CueBinDisc::new(cue_content, bin_data.to_vec()).map_err(|e| {
            console_error!("Failed to parse CUE/BIN: {}", e);
            JsValue::from_str(&format!("Failed to parse CUE/BIN: {}", e))
        })?;
        
        // Load the disc into the PSX emulator
        self.load_disc(disc)?;
        
        *self.disc_loaded.borrow_mut() = true;
        console_log!("CUE/BIN loaded successfully");
        Ok(())
    }
    
    fn load_bin_direct(&mut self, bin_data: &[u8]) -> std::result::Result<(), JsValue> {
        // Create a simple disc from just BIN data
        // Assume standard Mode2/2352 format
        let cue_content = r#"FILE "game.bin" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00"#;
        
        let disc = CueBinDisc::new(cue_content, bin_data.to_vec()).map_err(|e| {
            console_error!("Failed to create disc from BIN: {}", e);
            JsValue::from_str(&format!("Failed to create disc from BIN: {}", e))
        })?;
        
        self.load_disc(disc)?;
        *self.disc_loaded.borrow_mut() = true;
        Ok(())
    }
    
    fn load_disc(&mut self, disc: CueBinDisc) -> std::result::Result<(), JsValue> {
        // Load the disc data into PSX
        // For now, we'll load the boot executable from the disc
        
        // Try to find and load the PSX executable from the disc
        // Usually at sector 24 for PlayStation discs
        let boot_sector = disc.read_sector(24);
        
        if boot_sector.len() >= 2048 {
            // Check for PS-X EXE signature in the boot sector
            if boot_sector.len() > 8 && &boot_sector[0..8] == b"PS-X EXE" {
                self.psx.load_exe(&boot_sector).map_err(|e| {
                    console_error!("Failed to load boot executable: {:?}", e);
                    JsValue::from_str(&format!("Failed to load boot executable: {:?}", e))
                })?;
            } else {
                // Try to boot from system.cnf or other boot methods
                console_log!("No direct executable found, attempting ISO9660 boot");
                
                // For now, we'll just initialize the PSX with the disc loaded
                // The actual disc emulation would need more work
                self.psx.init_with_disc().map_err(|e| {
                    console_error!("Failed to initialize with disc: {:?}", e);
                    JsValue::from_str(&format!("Failed to initialize with disc: {:?}", e))
                })?;
            }
        }
        
        Ok(())
    }
    
    pub fn run_frame(&mut self) -> std::result::Result<(), JsValue> {
        if !self.is_running() {
            return Ok(());
        }
        
        self.update_input();
        
        self.psx.run_frame().map_err(|e| {
            console_error!("Frame execution error: {:?}", e);
            JsValue::from_str(&format!("Frame execution error: {:?}", e))
        })?;
        
        self.render_frame()?;
        self.process_audio()?;
        
        Ok(())
    }
    
    fn update_input(&mut self) {
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
        if keys[90] || gamepad_buttons[1] { pad_state |= 0x2000; } // Circle
        if keys[83] || gamepad_buttons[2] { pad_state |= 0x8000; } // Square
        if keys[65] || gamepad_buttons[3] { pad_state |= 0x1000; } // Triangle
        
        // Shoulder buttons
        if keys[81] || gamepad_buttons[4] { pad_state |= 0x0004; } // L1
        if keys[87] || gamepad_buttons[5] { pad_state |= 0x0008; } // R1
        if keys[69] || gamepad_buttons[6] { pad_state |= 0x0001; } // L2
        if keys[82] || gamepad_buttons[7] { pad_state |= 0x0002; } // R2
        
        // Start/Select
        if keys[13] || gamepad_buttons[9] { pad_state |= 0x0800; } // Start
        if keys[16] || gamepad_buttons[8] { pad_state |= 0x0100; } // Select
        
        self.psx.set_controller_state(0, pad_state);
    }
    
    fn render_frame(&mut self) -> std::result::Result<(), JsValue> {
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
    
    fn process_audio(&mut self) -> std::result::Result<(), JsValue> {
        // Audio processing disabled for now - would require more web-sys features
        // to be properly implemented
        self.audio_buffer.clear();
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
    
    pub fn reset(&mut self) -> std::result::Result<(), JsValue> {
        self.psx.reset().map_err(|e| {
            console_error!("Failed to reset: {:?}", e);
            JsValue::from_str(&format!("Failed to reset: {:?}", e))
        })?;
        console_log!("Emulator reset");
        Ok(())
    }
    
    pub fn handle_keyboard_event(&mut self, event: KeyboardEvent, pressed: bool) {
        let keycode = event.key_code();
        self.input_state.set_key(keycode, pressed);
        
        if pressed {
            console_log!("Key pressed: {} (code: {})", event.key(), keycode);
        }
    }
    
    pub fn handle_gamepad(&mut self, gamepad: &Gamepad) {
        let buttons = gamepad.buttons();
        for (i, button) in buttons.iter().enumerate() {
            if let Ok(button) = button.dyn_into::<web_sys::GamepadButton>() {
                self.input_state.set_gamepad_button(i as u32, button.pressed());
            }
        }
        
        let axes = gamepad.axes();
        for (i, axis) in axes.iter().enumerate() {
            if let Some(value) = axis.as_f64() {
                self.input_state.set_gamepad_axis(i as u32, value as f32);
            }
        }
    }
    
    pub fn get_save_state(&self) -> Vec<u8> {
        vec![] // TODO: Implement save states
    }
    
    pub fn load_save_state(&mut self, _state: &[u8]) -> std::result::Result<(), JsValue> {
        Err(JsValue::from_str("Save states not yet implemented"))
    }
}