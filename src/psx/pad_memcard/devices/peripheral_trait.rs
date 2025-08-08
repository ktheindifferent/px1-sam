//! Common trait for all PlayStation peripherals

use super::super::DsrState;

/// Response from a peripheral device
pub struct Response {
    /// Data byte to send back
    pub data: u8,
    /// Whether to request DSR pulse
    pub request_dsr: bool,
}

impl Response {
    /// Convert to tuple format for compatibility
    pub fn to_tuple(self) -> (u8, DsrState) {
        let dsr = if self.request_dsr {
            DsrState::Pending(100) // Standard DSR delay
        } else {
            DsrState::Idle
        };
        (self.data, dsr)
    }
}

/// Trait for PlayStation peripherals
pub trait Peripheral: Send {
    /// Send a byte to the peripheral and get response
    fn send_byte(&mut self, cmd: u8, target_device: bool) -> Response;
    
    /// Set button state (for controllers)
    fn set_button(&mut self, _button: usize, _pressed: bool) {}
    
    /// Set axis state (for analog controllers) 
    fn set_axis(&mut self, _axis: usize, _value: i16) {}
    
    /// Get rumble state (for DualShock)
    fn get_rumble(&self) -> (u8, u8) {
        (0, 0)
    }
    
    /// Called when device is selected
    fn select(&mut self) {}
    
    /// Called once per frame
    fn new_frame(&mut self) {}
    
    /// Get device description
    fn description(&self) -> String {
        "Generic Peripheral".to_string()
    }
    
    /// Clone the peripheral (for serialization)
    fn clone_box(&self) -> Box<dyn Peripheral>;
}

// Implement cloning for Box<dyn Peripheral>
impl Clone for Box<dyn Peripheral> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}