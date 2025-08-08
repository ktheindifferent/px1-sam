//! Fishing Controller Implementation
//!
//! The fishing controller features:
//! - Motion sensor for casting
//! - Reel with rotation sensor
//! - Buttons for menu navigation

use super::{PeripheralTrait, Response};

/// Fishing controller state
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FishingController {
    /// Button states (16-bit)
    buttons: u16,
    /// Rod motion sensor value (0-255, 128 = neutral)
    rod_motion: u8,
    /// Reel rotation value (0-255, increments with clockwise rotation)
    reel_position: u8,
    /// Reel rotation speed (0-255)
    reel_speed: u8,
    /// Tilt sensor X axis
    tilt_x: u8,
    /// Tilt sensor Y axis
    tilt_y: u8,
    /// Current transfer state
    transfer_state: TransferState,
    /// Motion accumulator for realistic physics
    motion_accumulator: f32,
    /// Reel rotation accumulator
    reel_accumulator: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum TransferState {
    Idle,
    Connected,
    SendId1,
    SendId2,
    SendButtons1,
    SendButtons2,
    SendRodMotion,
    SendReelPosition,
    SendReelSpeed,
    SendTiltX,
    SendTiltY,
}

impl FishingController {
    pub fn new() -> Self {
        FishingController {
            buttons: 0xFFFF, // All released
            rod_motion: 128,  // Neutral position
            reel_position: 0,
            reel_speed: 0,
            tilt_x: 128,
            tilt_y: 128,
            transfer_state: TransferState::Idle,
            motion_accumulator: 0.0,
            reel_accumulator: 0.0,
        }
    }

    /// Simulate casting motion
    pub fn cast(&mut self, strength: f32) {
        let clamped = strength.max(0.0).min(1.0);
        self.rod_motion = (128.0 + clamped * 127.0) as u8;
        self.motion_accumulator = clamped * 10.0; // Decay over time
    }

    /// Simulate reeling in
    pub fn reel(&mut self, speed: f32) {
        let clamped = speed.max(-1.0).min(1.0);
        self.reel_speed = ((clamped + 1.0) * 127.5) as u8;
        
        // Update reel position
        self.reel_accumulator += clamped * 10.0;
        self.reel_position = (self.reel_position as i16 + self.reel_accumulator as i16) as u8;
        self.reel_accumulator = self.reel_accumulator.fract();
    }

    /// Set rod tilt
    pub fn set_tilt(&mut self, x: f32, y: f32) {
        self.tilt_x = ((x.max(-1.0).min(1.0) + 1.0) * 127.5) as u8;
        self.tilt_y = ((y.max(-1.0).min(1.0) + 1.0) * 127.5) as u8;
    }

    /// Update physics simulation
    pub fn update_physics(&mut self) {
        // Decay motion over time
        if self.motion_accumulator > 0.0 {
            self.motion_accumulator -= 0.1;
            if self.motion_accumulator < 0.0 {
                self.motion_accumulator = 0.0;
            }
        }
        
        // Return rod to neutral position
        if self.rod_motion > 128 {
            self.rod_motion = (self.rod_motion - 1).max(128);
        } else if self.rod_motion < 128 {
            self.rod_motion = (self.rod_motion + 1).min(128);
        }
        
        // Slow down reel
        if self.reel_speed > 128 {
            self.reel_speed = (self.reel_speed - 1).max(128);
        } else if self.reel_speed < 128 {
            self.reel_speed = (self.reel_speed + 1).min(128);
        }
    }

    fn press_button(&mut self, button: FishingButton) {
        self.buttons &= !button.mask();
    }

    fn release_button(&mut self, button: FishingButton) {
        self.buttons |= button.mask();
    }
}

impl PeripheralTrait for FishingController {
    fn send_byte(&mut self, cmd: u8, _target_device: bool) -> Response {
        use self::TransferState::*;

        let (response, next_state, request_dsr) = match self.transfer_state {
            Idle => {
                if cmd == 0x01 {
                    (0xFF, Connected, true)
                } else {
                    (0xFF, Idle, false)
                }
            }
            Connected => {
                if cmd == 0x42 {
                    // Read controller state
                    (0x5E, SendId1, true) // Fishing controller ID
                } else {
                    (0xFF, Idle, false)
                }
            }
            SendId1 => {
                (0x5A, SendId2, true) // Standard ID2
            }
            SendId2 => {
                // Send button states low byte
                (self.buttons as u8, SendButtons1, true)
            }
            SendButtons1 => {
                // Send button states high byte
                ((self.buttons >> 8) as u8, SendButtons2, true)
            }
            SendButtons2 => {
                // Send rod motion sensor
                (self.rod_motion, SendRodMotion, true)
            }
            SendRodMotion => {
                // Send reel position
                (self.reel_position, SendReelPosition, true)
            }
            SendReelPosition => {
                // Send reel speed
                (self.reel_speed, SendReelSpeed, true)
            }
            SendReelSpeed => {
                // Send tilt X
                (self.tilt_x, SendTiltX, true)
            }
            SendTiltX => {
                // Send tilt Y
                (self.tilt_y, SendTiltY, false) // Last byte
            }
            SendTiltY => {
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
        let fishing_button = match button {
            0 => FishingButton::Cast,
            1 => FishingButton::Reel,
            2 => FishingButton::Select,
            3 => FishingButton::Start,
            4 => FishingButton::Up,
            5 => FishingButton::Right,
            6 => FishingButton::Down,
            7 => FishingButton::Left,
            _ => return,
        };

        if pressed {
            self.press_button(fishing_button);
        } else {
            self.release_button(fishing_button);
        }
    }

    fn set_axis(&mut self, axis: usize, value: i16) {
        match axis {
            0 => {
                // X-axis -> Reel rotation
                let normalized = (value as f32) / 32768.0;
                self.reel(normalized);
            }
            1 => {
                // Y-axis -> Rod motion
                if value > 10000 {
                    // Quick upward motion = cast
                    let strength = (value as f32) / 32767.0;
                    self.cast(strength);
                }
            }
            2 => {
                // Right stick X -> Tilt X
                let normalized = (value as f32) / 32768.0;
                self.set_tilt(normalized, (self.tilt_y as f32 - 128.0) / 127.5);
            }
            3 => {
                // Right stick Y -> Tilt Y
                let normalized = (value as f32) / 32768.0;
                self.set_tilt((self.tilt_x as f32 - 128.0) / 127.5, normalized);
            }
            _ => {}
        }
    }

    fn new_frame(&mut self) {
        self.update_physics();
    }

    fn clone_box(&self) -> Box<dyn PeripheralTrait> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone, Copy)]
enum FishingButton {
    Cast,
    Reel,
    Select,
    Start,
    Up,
    Right,
    Down,
    Left,
}

impl FishingButton {
    fn mask(self) -> u16 {
        match self {
            FishingButton::Cast => 1 << 0,
            FishingButton::Reel => 1 << 1,
            FishingButton::Select => 1 << 8,
            FishingButton::Start => 1 << 11,
            FishingButton::Up => 1 << 12,
            FishingButton::Right => 1 << 13,
            FishingButton::Down => 1 << 14,
            FishingButton::Left => 1 << 15,
        }
    }
}

/// Create a new fishing controller peripheral
pub fn fishing_controller() -> Box<dyn PeripheralTrait> {
    Box::new(FishingController::new())
}