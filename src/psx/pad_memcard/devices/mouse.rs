//! PlayStation Mouse implementation
//!
//! The PlayStation Mouse (SCPH-1090) provides:
//! - 2-axis relative movement
//! - Left and right buttons
//! - 256 counts per inch resolution

use super::{PeripheralTrait, Response};

/// PlayStation Mouse state
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Mouse {
    /// Accumulated X movement since last read (-128 to 127)
    delta_x: i8,
    /// Accumulated Y movement since last read (-128 to 127)
    delta_y: i8,
    /// Button states (bit 0 = left, bit 1 = right)
    buttons: u8,
    /// Current transfer state
    transfer_state: TransferState,
    /// Mouse speed/sensitivity setting
    sensitivity: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum TransferState {
    Idle,
    Connected,
    SendId1,
    SendId2,
    SendButtons,
    SendDeltaX,
    SendDeltaY,
}

impl Mouse {
    pub fn new() -> Self {
        Mouse {
            delta_x: 0,
            delta_y: 0,
            buttons: 0xFF, // All buttons released (active low)
            transfer_state: TransferState::Idle,
            sensitivity: 1,
        }
    }

    /// Move the mouse by the given amount
    pub fn move_mouse(&mut self, dx: i32, dy: i32) {
        // Clamp and accumulate movement
        let new_x = (self.delta_x as i32) + dx;
        let new_y = (self.delta_y as i32) + dy;
        
        self.delta_x = new_x.max(-128).min(127) as i8;
        self.delta_y = new_y.max(-128).min(127) as i8;
    }

    /// Set mouse position from absolute coordinates
    pub fn set_position(&mut self, x: f32, y: f32) {
        // Convert to relative movement
        let dx = (x * self.sensitivity as f32) as i32;
        let dy = (y * self.sensitivity as f32) as i32;
        self.move_mouse(dx, dy);
    }

    /// Press a mouse button
    pub fn press_button(&mut self, button: MouseButton) {
        self.buttons &= !button.mask();
    }

    /// Release a mouse button
    pub fn release_button(&mut self, button: MouseButton) {
        self.buttons |= button.mask();
    }

    /// Set mouse sensitivity (1-3)
    pub fn set_sensitivity(&mut self, sens: u8) {
        self.sensitivity = sens.max(1).min(3);
    }

    /// Clear accumulated movement after reading
    fn clear_deltas(&mut self) {
        self.delta_x = 0;
        self.delta_y = 0;
    }
}

impl PeripheralTrait for Mouse {
    fn send_byte(&mut self, cmd: u8, _target_device: bool) -> Response {
        use self::TransferState::*;

        let (response, next_state, request_dsr) = match self.transfer_state {
            Idle => {
                if cmd == 0x01 {
                    // Start mouse access
                    (0xFF, Connected, true)
                } else {
                    // Unknown command
                    (0xFF, Idle, false)
                }
            }
            Connected => {
                if cmd == 0x42 {
                    // Read mouse state
                    (0x12, SendId1, true) // Mouse ID byte 1
                } else {
                    // Unsupported command
                    (0xFF, Idle, false)
                }
            }
            SendId1 => {
                // Send ID byte 2 (0x12 for mouse in normal mode)
                (0x5A, SendId2, true)
            }
            SendId2 => {
                // Send button states
                (self.buttons, SendButtons, true)
            }
            SendButtons => {
                // Send X delta (with sign extension)
                let x_data = self.delta_x as u8;
                (x_data, SendDeltaX, true)
            }
            SendDeltaX => {
                // Send Y delta (with sign extension)
                let y_data = self.delta_y as u8;
                (y_data, SendDeltaY, false) // Last byte, no DSR
            }
            SendDeltaY => {
                // Transfer complete, clear deltas for next read
                self.clear_deltas();
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
        // Map button indices to mouse buttons
        match button {
            0 => {
                // Left button
                if pressed {
                    self.press_button(MouseButton::Left);
                } else {
                    self.release_button(MouseButton::Left);
                }
            }
            1 => {
                // Right button
                if pressed {
                    self.press_button(MouseButton::Right);
                } else {
                    self.release_button(MouseButton::Right);
                }
            }
            _ => {}
        }
    }

    fn set_axis(&mut self, axis: usize, value: i16) {
        // Map axis inputs to mouse movement
        match axis {
            0 => {
                // X-axis movement
                let movement = (value / 256) as i32; // Scale down for reasonable speed
                self.move_mouse(movement, 0);
            }
            1 => {
                // Y-axis movement
                let movement = (value / 256) as i32;
                self.move_mouse(0, movement);
            }
            _ => {}
        }
    }
    
    fn clone_box(&self) -> Box<dyn PeripheralTrait> {
        Box::new(self.clone())
    }
}

/// Mouse button enumeration
#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
}

impl MouseButton {
    fn mask(self) -> u8 {
        match self {
            MouseButton::Left => 1 << 3,
            MouseButton::Right => 1 << 2,
        }
    }
}

/// Create a new PlayStation Mouse peripheral
pub fn mouse() -> Box<dyn PeripheralTrait> {
    Box::new(Mouse::new())
}