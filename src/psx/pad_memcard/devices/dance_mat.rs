//! Dance Dance Revolution Dance Mat Implementation
//!
//! The dance mat is essentially a digital controller with foot-operated buttons
//! arranged in a 3x3 grid pattern.

use super::{PeripheralTrait, Response};

/// Dance mat controller state
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DanceMat {
    /// Button states (16-bit, active low)
    buttons: u16,
    /// Current transfer state
    transfer_state: TransferState,
    /// Pressure sensitivity for each pad (future enhancement)
    pad_pressure: [u8; 9],
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum TransferState {
    Idle,
    Connected,
    SendId1,
    SendId2,
    SendButtons1,
    SendButtons2,
}

impl DanceMat {
    pub fn new() -> Self {
        DanceMat {
            buttons: 0xFFFF, // All released (active low)
            transfer_state: TransferState::Idle,
            pad_pressure: [0; 9],
        }
    }

    /// Press a dance pad
    pub fn press_pad(&mut self, pad: DancePad) {
        self.buttons &= !pad.mask();
        // Simulate pressure
        let index = pad.index();
        if index < 9 {
            self.pad_pressure[index] = 255;
        }
    }

    /// Release a dance pad
    pub fn release_pad(&mut self, pad: DancePad) {
        self.buttons |= pad.mask();
        // Clear pressure
        let index = pad.index();
        if index < 9 {
            self.pad_pressure[index] = 0;
        }
    }

    /// Check if a pad is pressed
    pub fn is_pressed(&self, pad: DancePad) -> bool {
        (self.buttons & pad.mask()) == 0
    }

    /// Get pressure value for a pad (0-255)
    pub fn get_pressure(&self, pad: DancePad) -> u8 {
        let index = pad.index();
        if index < 9 {
            self.pad_pressure[index]
        } else {
            0
        }
    }

    /// Simulate pressure decay (for more realistic feel)
    pub fn update_pressure(&mut self) {
        for i in 0..9 {
            if self.pad_pressure[i] > 0 && self.pad_pressure[i] < 255 {
                // Gradual pressure release
                self.pad_pressure[i] = self.pad_pressure[i].saturating_sub(10);
            }
        }
    }
}

impl PeripheralTrait for DanceMat {
    fn send_byte(&mut self, cmd: u8, _target_device: bool) -> Response {
        use self::TransferState::*;

        let (response, next_state, request_dsr) = match self.transfer_state {
            Idle => {
                if cmd == 0x01 {
                    // Start pad access
                    (0xFF, Connected, true)
                } else {
                    (0xFF, Idle, false)
                }
            }
            Connected => {
                if cmd == 0x42 {
                    // Read pad state - report as digital pad
                    (0x41, SendId1, true) // Digital pad ID
                } else {
                    (0xFF, Idle, false)
                }
            }
            SendId1 => {
                // Send ID byte 2
                (0x5A, SendId2, true)
            }
            SendId2 => {
                // Send button states low byte
                // Map dance pads to standard controller buttons
                let buttons_low = self.buttons as u8;
                (buttons_low, SendButtons1, true)
            }
            SendButtons1 => {
                // Send button states high byte
                let buttons_high = (self.buttons >> 8) as u8;
                (buttons_high, SendButtons2, false) // Last byte
            }
            SendButtons2 => {
                // Transfer complete
                (0xFF, Idle, false)
            }
        };

        self.transfer_state = next_state;

        Response {
            data: response,
            request_dsr,
        }
    }

    fn set_button(&mut self, button: usize, pressed: bool) {
        // Map button indices to dance pads
        let pad = match button {
            0 => DancePad::Down,      // X button
            1 => DancePad::Right,     // Circle button
            2 => DancePad::Left,      // Square button
            3 => DancePad::Up,        // Triangle button
            4 => DancePad::UpLeft,    // L1
            5 => DancePad::UpRight,   // R1
            6 => DancePad::DownLeft,  // L2
            7 => DancePad::DownRight, // R2
            8 => DancePad::Center,    // Select (center pad)
            9 => DancePad::Start,     // Start
            _ => return,
        };

        if pressed {
            self.press_pad(pad);
        } else {
            self.release_pad(pad);
        }
    }

    fn new_frame(&mut self) {
        // Update pressure simulation
        self.update_pressure();
    }

    fn clone_box(&self) -> Box<dyn PeripheralTrait> {
        Box::new(self.clone())
    }

    fn description(&self) -> String {
        "Dance Mat Controller".to_string()
    }
}

/// Dance pad positions
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DancePad {
    // Arrow pads (main gameplay)
    Up,
    Down,
    Left,
    Right,
    // Corner pads (advanced gameplay)
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    // Center pad (menu navigation)
    Center,
    // Start button
    Start,
}

impl DancePad {
    fn mask(self) -> u16 {
        match self {
            // Map to PlayStation controller buttons
            DancePad::Up => 1 << 12,        // D-pad up
            DancePad::Right => 1 << 13,     // D-pad right
            DancePad::Down => 1 << 14,      // D-pad down
            DancePad::Left => 1 << 15,      // D-pad left
            DancePad::UpLeft => 1 << 4,     // L1
            DancePad::UpRight => 1 << 5,    // R1
            DancePad::DownLeft => 1 << 6,   // L2
            DancePad::DownRight => 1 << 7,  // R2
            DancePad::Center => 1 << 8,     // Select
            DancePad::Start => 1 << 11,     // Start
        }
    }

    fn index(self) -> usize {
        match self {
            DancePad::UpLeft => 0,
            DancePad::Up => 1,
            DancePad::UpRight => 2,
            DancePad::Left => 3,
            DancePad::Center => 4,
            DancePad::Right => 5,
            DancePad::DownLeft => 6,
            DancePad::Down => 7,
            DancePad::DownRight => 8,
            DancePad::Start => 9, // Outside the 3x3 grid
        }
    }
}

/// Create a new dance mat peripheral
pub fn dance_mat() -> Box<dyn PeripheralTrait> {
    Box::new(DanceMat::new())
}

/// Dance mat with combo detection
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DanceMatPro {
    base: DanceMat,
    /// Combo counter
    combo: u32,
    /// Last pressed pad for combo tracking
    last_pad: Option<DancePad>,
    /// Timing window for combos (in frames)
    combo_window: u8,
    /// Current combo timer
    combo_timer: u8,
}

impl DanceMatPro {
    pub fn new() -> Self {
        DanceMatPro {
            base: DanceMat::new(),
            combo: 0,
            last_pad: None,
            combo_window: 30, // ~0.5 seconds at 60fps
            combo_timer: 0,
        }
    }

    pub fn press_pad_with_timing(&mut self, pad: DancePad) {
        self.base.press_pad(pad);
        
        // Check for combo
        if self.combo_timer > 0 {
            if let Some(last) = self.last_pad {
                if last != pad {
                    // Different pad pressed within window - combo!
                    self.combo += 1;
                    debug!("Dance combo: {} hits!", self.combo);
                }
            }
        } else {
            // Combo broken
            if self.combo > 0 {
                debug!("Combo ended at {} hits", self.combo);
            }
            self.combo = 0;
        }
        
        self.last_pad = Some(pad);
        self.combo_timer = self.combo_window;
    }

    pub fn update(&mut self) {
        if self.combo_timer > 0 {
            self.combo_timer -= 1;
            if self.combo_timer == 0 && self.combo > 0 {
                debug!("Combo timeout - ended at {} hits", self.combo);
                self.combo = 0;
                self.last_pad = None;
            }
        }
        
        self.base.update_pressure();
    }
}