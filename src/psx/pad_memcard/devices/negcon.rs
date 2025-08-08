//! NeGcon racing controller implementation
//!
//! The NeGcon (NPC-101) is a specialized racing controller with:
//! - Analog twist control for steering
//! - Analog I and II buttons for gas/brake
//! - Digital buttons (A, B, Start)
//! - L shoulder button (digital)

use super::{PeripheralTrait, Response};

/// NeGcon controller state
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct NeGcon {
    /// Twist axis value (0x00 = full left, 0x80 = center, 0xFF = full right)
    twist: u8,
    /// I button analog value (0x00 = released, 0xFF = fully pressed)
    button_i: u8,
    /// II button analog value (0x00 = released, 0xFF = fully pressed)
    button_ii: u8,
    /// L button analog value (0x00 = released, 0xFF = fully pressed)
    button_l: u8,
    /// Digital button states (Start, Up, Right, Down, Left, L)
    digital_buttons: u16,
    /// Current transfer state
    transfer_state: TransferState,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum TransferState {
    Idle,
    Connected,
    SendId1,
    SendId2,
    SendTwist,
    SendButtonI,
    SendButtonII,
    SendButtonL,
    SendButtons1,
    SendButtons2,
}

impl NeGcon {
    pub fn new() -> Self {
        NeGcon {
            twist: 0x80,        // Center position
            button_i: 0x00,     // Released
            button_ii: 0x00,    // Released
            button_l: 0x00,     // Released
            digital_buttons: 0xFFFF, // All released (active low)
            transfer_state: TransferState::Idle,
        }
    }

    /// Set the twist (steering) position
    pub fn set_twist(&mut self, value: u8) {
        self.twist = value;
    }

    /// Set analog button values
    pub fn set_button_i(&mut self, value: u8) {
        self.button_i = value;
    }

    pub fn set_button_ii(&mut self, value: u8) {
        self.button_ii = value;
    }

    pub fn set_button_l(&mut self, value: u8) {
        self.button_l = value;
    }

    /// Press a digital button
    pub fn press_button(&mut self, button: NeGconButton) {
        self.digital_buttons &= !button.mask();
    }

    /// Release a digital button
    pub fn release_button(&mut self, button: NeGconButton) {
        self.digital_buttons |= button.mask();
    }

    /// Map analog steering input (-1.0 to 1.0) to twist value
    pub fn set_steering(&mut self, steering: f32) {
        let clamped = steering.max(-1.0).min(1.0);
        // Convert to 0x00-0xFF range with 0x80 as center
        self.twist = ((clamped + 1.0) * 127.5) as u8;
    }

    /// Map analog throttle input (0.0 to 1.0) to button I
    pub fn set_throttle(&mut self, throttle: f32) {
        let clamped = throttle.max(0.0).min(1.0);
        self.button_i = (clamped * 255.0) as u8;
    }

    /// Map analog brake input (0.0 to 1.0) to button II
    pub fn set_brake(&mut self, brake: f32) {
        let clamped = brake.max(0.0).min(1.0);
        self.button_ii = (clamped * 255.0) as u8;
    }
}

impl PeripheralTrait for NeGcon {
    fn send_byte(&mut self, cmd: u8, _target_device: bool) -> Response {
        use self::TransferState::*;

        let (response, next_state, request_dsr) = match self.transfer_state {
            Idle => {
                if cmd == 0x01 {
                    // Start pad access
                    (0xFF, Connected, true)
                } else {
                    // Unknown command
                    (0xFF, Idle, false)
                }
            }
            Connected => {
                if cmd == 0x42 {
                    // Read pad state
                    (0x23, SendId1, true) // NeGcon ID byte 1
                } else {
                    // Unsupported command
                    (0xFF, Idle, false)
                }
            }
            SendId1 => {
                // Send ID byte 2 (0x23 for NeGcon)
                (0x23, SendId2, true)
            }
            SendId2 => {
                // Send twist value
                (self.twist, SendTwist, true)
            }
            SendTwist => {
                // Send button I value
                (self.button_i, SendButtonI, true)
            }
            SendButtonI => {
                // Send button II value
                (self.button_ii, SendButtonII, true)
            }
            SendButtonII => {
                // Send button L value
                (self.button_l, SendButtonL, true)
            }
            SendButtonL => {
                // Send digital buttons byte 1
                let buttons1 = self.digital_buttons as u8;
                (buttons1, SendButtons1, true)
            }
            SendButtons1 => {
                // Send digital buttons byte 2
                let buttons2 = (self.digital_buttons >> 8) as u8;
                (buttons2, SendButtons2, false) // Last byte, no DSR
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
        // Map standard button indices to NeGcon buttons
        match button {
            0 => {
                // A button
                if pressed {
                    self.press_button(NeGconButton::A);
                } else {
                    self.release_button(NeGconButton::A);
                }
            }
            1 => {
                // B button
                if pressed {
                    self.press_button(NeGconButton::B);
                } else {
                    self.release_button(NeGconButton::B);
                }
            }
            7 => {
                // Start button
                if pressed {
                    self.press_button(NeGconButton::Start);
                } else {
                    self.release_button(NeGconButton::Start);
                }
            }
            _ => {}
        }
    }

    fn set_axis(&mut self, axis: usize, value: i16) {
        // Map axis inputs to NeGcon controls
        match axis {
            0 => {
                // X-axis -> Twist (steering)
                let normalized = (value as f32) / 32768.0;
                self.set_steering(normalized);
            }
            1 => {
                // Y-axis -> Throttle/Brake
                if value > 0 {
                    // Positive = throttle
                    let normalized = (value as f32) / 32767.0;
                    self.set_throttle(normalized);
                    self.set_brake(0.0);
                } else {
                    // Negative = brake
                    let normalized = (-value as f32) / 32768.0;
                    self.set_brake(normalized);
                    self.set_throttle(0.0);
                }
            }
            _ => {}
        }
    }
}

/// NeGcon button enumeration
#[derive(Debug, Clone, Copy)]
pub enum NeGconButton {
    Start,
    Up,
    Right,
    Down,
    Left,
    R,
    B,
    A,
}

impl NeGconButton {
    fn mask(self) -> u16 {
        use self::NeGconButton::*;
        match self {
            Start => 1 << 3,
            Up => 1 << 4,
            Right => 1 << 5,
            Down => 1 << 6,
            Left => 1 << 7,
            R => 1 << 8,
            B => 1 << 9,
            A => 1 << 10,
        }
    }
}

/// Create a new NeGcon controller peripheral
pub fn negcon() -> Box<dyn PeripheralTrait> {
    Box::new(NeGcon::new())
}