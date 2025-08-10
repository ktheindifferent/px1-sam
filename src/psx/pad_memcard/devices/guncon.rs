//! GunCon/G-Con 45 light gun implementation
//!
//! The GunCon (NPC-103) is a light gun controller that provides:
//! - Screen position detection via CRT timing
//! - Trigger and two action buttons (A, B)
//! - Compatible with games like Time Crisis, Point Blank

use super::{PeripheralTrait, Response};

/// GunCon light gun state
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct GunCon {
    /// X coordinate on screen (0-1023, though typically 0-380 for visible area)
    x_position: u16,
    /// Y coordinate on screen (0-262 for NTSC, 0-312 for PAL)
    y_position: u16,
    /// Button states (trigger, A, B)
    buttons: u8,
    /// Current transfer state
    transfer_state: TransferState,
    /// Whether the gun is currently aimed at the screen
    on_screen: bool,
    /// Video standard (affects Y coordinate range)
    video_standard: VideoStandard,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum TransferState {
    Idle,
    Connected,
    SendId1,
    SendId2,
    SendButtons,
    SendXLow,
    SendXHigh,
    SendYLow,
    SendYHigh,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VideoStandard {
    NTSC,
    PAL,
}

impl GunCon {
    pub fn new(video_standard: VideoStandard) -> Self {
        GunCon {
            x_position: 0x100, // Center of screen
            y_position: 0x80,  // Center of screen
            buttons: 0xFF,     // All buttons released
            transfer_state: TransferState::Idle,
            on_screen: false,
            video_standard,
        }
    }

    /// Set the gun's aim position in screen coordinates
    pub fn set_position(&mut self, x: u16, y: u16) {
        self.x_position = x.min(1023);
        let y_max = match self.video_standard {
            VideoStandard::NTSC => 262,
            VideoStandard::PAL => 312,
        };
        self.y_position = y.min(y_max);
        self.on_screen = true;
    }

    /// Set position from normalized coordinates (0.0 to 1.0)
    pub fn set_normalized_position(&mut self, x: f32, y: f32) {
        let x_pos = (x.max(0.0).min(1.0) * 380.0) as u16; // Visible area is ~380 pixels
        let y_max = match self.video_standard {
            VideoStandard::NTSC => 240.0,
            VideoStandard::PAL => 288.0,
        };
        let y_pos = (y.max(0.0).min(1.0) * y_max) as u16;
        self.set_position(x_pos, y_pos);
    }

    /// Set whether the gun is aimed at the screen
    pub fn set_on_screen(&mut self, on_screen: bool) {
        self.on_screen = on_screen;
        if !on_screen {
            // When off-screen, set to maximum values
            self.x_position = 0x3FF;
            self.y_position = match self.video_standard {
                VideoStandard::NTSC => 0x106,
                VideoStandard::PAL => 0x138,
            };
        }
    }

    /// Press a button
    pub fn press_button(&mut self, button: GunConButton) {
        self.buttons &= !button.mask();
    }

    /// Release a button
    pub fn release_button(&mut self, button: GunConButton) {
        self.buttons |= button.mask();
    }

    /// Pull the trigger
    pub fn pull_trigger(&mut self) {
        self.press_button(GunConButton::Trigger);
    }

    /// Release the trigger
    pub fn release_trigger(&mut self) {
        self.release_button(GunConButton::Trigger);
    }
}

impl PeripheralTrait for GunCon {
    fn send_byte(&mut self, cmd: u8, _target_device: bool) -> Response {
        use self::TransferState::*;

        let (response, next_state, request_dsr) = match self.transfer_state {
            Idle => {
                if cmd == 0x01 {
                    // Start GunCon access
                    (0x63, Connected, true) // GunCon returns 0x63 instead of 0xFF
                } else {
                    // Unknown command
                    (0xFF, Idle, false)
                }
            }
            Connected => {
                if cmd == 0x42 {
                    // Read GunCon state
                    (0x5A, SendId1, true) // Standard controller ID
                } else {
                    // Unsupported command
                    (0xFF, Idle, false)
                }
            }
            SendId1 => {
                // Send button states
                (self.buttons, SendButtons, true)
            }
            SendButtons => {
                // Send X coordinate low byte
                (self.x_position as u8, SendXLow, true)
            }
            SendXLow => {
                // Send X coordinate high byte
                ((self.x_position >> 8) as u8, SendXHigh, true)
            }
            SendXHigh => {
                // Send Y coordinate low byte
                (self.y_position as u8, SendYLow, true)
            }
            SendYLow => {
                // Send Y coordinate high byte
                ((self.y_position >> 8) as u8, SendYHigh, false) // Last byte, no DSR
            }
            SendYHigh => {
                // Transfer complete
                (0xFF, Idle, false)
            }
            _ => {
                // Should not happen
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
        // Map button indices to GunCon buttons
        match button {
            0 => {
                // Trigger
                if pressed {
                    self.pull_trigger();
                } else {
                    self.release_trigger();
                }
            }
            1 => {
                // A button
                if pressed {
                    self.press_button(GunConButton::A);
                } else {
                    self.release_button(GunConButton::A);
                }
            }
            2 => {
                // B button
                if pressed {
                    self.press_button(GunConButton::B);
                } else {
                    self.release_button(GunConButton::B);
                }
            }
            _ => {}
        }
    }

    fn set_axis(&mut self, axis: usize, value: i16) {
        // Map axis inputs to screen position
        match axis {
            0 => {
                // X-axis -> Screen X position
                let normalized = (value as f32 + 32768.0) / 65536.0;
                let current_y = (self.y_position as f32) / match self.video_standard {
                    VideoStandard::NTSC => 240.0,
                    VideoStandard::PAL => 288.0,
                };
                self.set_normalized_position(normalized, current_y);
            }
            1 => {
                // Y-axis -> Screen Y position
                let normalized = (value as f32 + 32768.0) / 65536.0;
                let current_x = (self.x_position as f32) / 380.0;
                self.set_normalized_position(current_x, normalized);
            }
            _ => {}
        }
    }
    
    fn clone_box(&self) -> Box<dyn PeripheralTrait> {
        Box::new(self.clone())
    }
}

/// GunCon button enumeration
#[derive(Debug, Clone, Copy)]
pub enum GunConButton {
    Trigger,
    A,
    B,
}

impl GunConButton {
    fn mask(self) -> u8 {
        match self {
            GunConButton::Trigger => 1 << 5,
            GunConButton::A => 1 << 3,
            GunConButton::B => 1 << 6,
        }
    }
}

/// Create a new GunCon light gun peripheral
pub fn guncon(video_standard: VideoStandard) -> Box<dyn PeripheralTrait> {
    Box::new(GunCon::new(video_standard))
}