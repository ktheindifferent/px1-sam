//! PlayStation Multitap implementation
//!
//! The Multitap (SCPH-1070) allows connecting up to 4 controllers and 4 memory cards
//! to a single controller port, enabling 4-player games.

use super::{PeripheralTrait, Response};

/// A disconnected peripheral placeholder
#[derive(Clone)]
struct DisconnectedPeripheral;

impl PeripheralTrait for DisconnectedPeripheral {
    fn send_byte(&mut self, _cmd: u8, _target_device: bool) -> Response {
        Response {
            data: 0xFF,
            request_dsr: false,
        }
    }
    
    fn clone_box(&self) -> Box<dyn PeripheralTrait> {
        Box::new(self.clone())
    }
}

/// Multitap adapter state
#[derive(Clone)]
pub struct Multitap {
    /// Four controller slots (A, B, C, D)
    controllers: [Box<dyn PeripheralTrait>; 4],
    /// Four memory card slots
    memory_cards: [Box<dyn PeripheralTrait>; 4],
    /// Currently selected slot (0-3)
    current_slot: u8,
    /// Whether we're accessing controllers or memory cards
    accessing_memory_cards: bool,
    /// Current transfer state
    transfer_state: TransferState,
    /// Data buffer for current transaction
    data_buffer: Vec<u8>,
    /// Current position in data buffer
    buffer_position: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TransferState {
    Idle,
    SelectSlot,
    ForwardCommand,
    RelayData,
}

impl Multitap {
    pub fn new() -> Self {
        Multitap {
            controllers: [
                Box::new(DisconnectedPeripheral),
                Box::new(DisconnectedPeripheral),
                Box::new(DisconnectedPeripheral),
                Box::new(DisconnectedPeripheral),
            ],
            memory_cards: [
                Box::new(DisconnectedPeripheral),
                Box::new(DisconnectedPeripheral),
                Box::new(DisconnectedPeripheral),
                Box::new(DisconnectedPeripheral),
            ],
            current_slot: 0,
            accessing_memory_cards: false,
            transfer_state: TransferState::Idle,
            data_buffer: Vec::with_capacity(256),
            buffer_position: 0,
        }
    }

    /// Connect a controller to the specified slot (0-3)
    pub fn connect_controller(&mut self, slot: usize, controller: Box<dyn PeripheralTrait>) {
        if slot < 4 {
            self.controllers[slot] = controller;
            info!("Multitap: Connected controller to slot {}", slot);
        }
    }

    /// Connect a memory card to the specified slot (0-3)
    pub fn connect_memory_card(&mut self, slot: usize, memory_card: Box<dyn PeripheralTrait>) {
        if slot < 4 {
            self.memory_cards[slot] = memory_card;
            info!("Multitap: Connected memory card to slot {}", slot);
        }
    }

    /// Get the currently selected peripheral
    fn current_peripheral(&mut self) -> &mut Box<dyn PeripheralTrait> {
        if self.accessing_memory_cards {
            &mut self.memory_cards[self.current_slot as usize]
        } else {
            &mut self.controllers[self.current_slot as usize]
        }
    }
}

impl PeripheralTrait for Multitap {
    fn send_byte(&mut self, cmd: u8, target_device: bool) -> Response {
        use self::TransferState::*;

        match self.transfer_state {
            Idle => {
                // Check for multitap-specific commands
                match cmd {
                    0x01 => {
                        // Standard controller/memory card access
                        self.accessing_memory_cards = target_device;
                        self.transfer_state = SelectSlot;
                        Response {
                            data: 0xFF,
                            request_dsr: true,
                        }
                    }
                    0x11 | 0x12 | 0x13 | 0x14 => {
                        // Direct slot access (0x11 = slot 0, 0x12 = slot 1, etc.)
                        self.current_slot = (cmd & 0x0F) - 1;
                        self.accessing_memory_cards = false;
                        self.transfer_state = ForwardCommand;
                        Response {
                            data: 0x80, // Multitap identification
                            request_dsr: true,
                        }
                    }
                    0x21 | 0x22 | 0x23 | 0x24 => {
                        // Direct memory card slot access
                        self.current_slot = (cmd & 0x0F) - 1;
                        self.accessing_memory_cards = true;
                        self.transfer_state = ForwardCommand;
                        Response {
                            data: 0x80, // Multitap identification
                            request_dsr: true,
                        }
                    }
                    _ => {
                        // Unknown command
                        Response {
                            data: 0xFF,
                            request_dsr: false,
                        }
                    }
                }
            }
            SelectSlot => {
                // The next byte selects which slot to access
                if cmd < 4 {
                    self.current_slot = cmd;
                    self.transfer_state = ForwardCommand;
                    Response {
                        data: 0x5A, // Acknowledge slot selection
                        request_dsr: true,
                    }
                } else {
                    // Invalid slot
                    self.transfer_state = Idle;
                    Response {
                        data: 0xFF,
                        request_dsr: false,
                    }
                }
            }
            ForwardCommand => {
                // Forward the command to the selected peripheral
                let peripheral = self.current_peripheral();
                let response = peripheral.send_byte(cmd, target_device);
                
                if response.request_dsr {
                    self.transfer_state = RelayData;
                } else {
                    self.transfer_state = Idle;
                }
                
                response
            }
            RelayData => {
                // Continue relaying data to/from the selected peripheral
                let peripheral = self.current_peripheral();
                let response = peripheral.send_byte(cmd, target_device);
                
                if !response.request_dsr {
                    // Transaction complete
                    self.transfer_state = Idle;
                }
                
                response
            }
        }
    }

    fn set_button(&mut self, button: usize, pressed: bool) {
        // Route button input to all connected controllers
        for controller in &mut self.controllers {
            controller.set_button(button, pressed);
        }
    }

    fn set_axis(&mut self, axis: usize, value: i16) {
        // Route axis input to all connected controllers
        for controller in &mut self.controllers {
            controller.set_axis(axis, value);
        }
    }
    
    fn clone_box(&self) -> Box<dyn PeripheralTrait> {
        Box::new(self.clone())
    }
}

/// Create a new Multitap peripheral
pub fn multitap() -> Box<dyn PeripheralTrait> {
    Box::new(Multitap::new())
}

/// Create a Multitap with 4 controllers connected
pub fn multitap_with_controllers(controllers: [Box<dyn PeripheralTrait>; 4]) -> Box<dyn PeripheralTrait> {
    let mut tap = Multitap::new();
    tap.controllers = controllers;
    tap.memory_cards = [
        Box::new(DisconnectedPeripheral),
        Box::new(DisconnectedPeripheral),
        Box::new(DisconnectedPeripheral),
        Box::new(DisconnectedPeripheral),
    ];
    Box::new(tap)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock controller for testing
    #[derive(Clone)]
    struct MockController {
        id: u8,
        button_pressed: bool,
        axis_value: i16,
    }

    impl PeripheralTrait for MockController {
        fn send_byte(&mut self, cmd: u8, _target_device: bool) -> Response {
            match cmd {
                0x01 => Response { data: 0xFF, request_dsr: true },
                0x42 => Response { data: self.id, request_dsr: true },
                _ => Response { data: 0xFF, request_dsr: false },
            }
        }

        fn set_button(&mut self, _button: usize, pressed: bool) {
            self.button_pressed = pressed;
        }

        fn set_axis(&mut self, _axis: usize, value: i16) {
            self.axis_value = value;
        }

        fn clone_box(&self) -> Box<dyn PeripheralTrait> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn test_multitap_creation() {
        let multitap = Multitap::new();
        assert_eq!(multitap.current_slot, 0);
        assert_eq!(multitap.accessing_memory_cards, false);
        assert_eq!(multitap.transfer_state, TransferState::Idle);
    }

    #[test]
    fn test_multitap_slot_selection() {
        let mut multitap = Multitap::new();
        
        // Start multitap access
        let response = multitap.send_byte(0x01, false);
        assert_eq!(response.data, 0xFF);
        assert_eq!(response.request_dsr, true);
        assert_eq!(multitap.transfer_state, TransferState::SelectSlot);
        
        // Select slot 2
        let response = multitap.send_byte(2, false);
        assert_eq!(response.data, 0x5A);
        assert_eq!(response.request_dsr, true);
        assert_eq!(multitap.current_slot, 2);
        assert_eq!(multitap.transfer_state, TransferState::ForwardCommand);
    }

    #[test]
    fn test_direct_slot_access() {
        let mut multitap = Multitap::new();
        
        // Direct access to controller slot 1 (0x12)
        let response = multitap.send_byte(0x12, false);
        assert_eq!(response.data, 0x80); // Multitap identification
        assert_eq!(response.request_dsr, true);
        assert_eq!(multitap.current_slot, 1);
        assert_eq!(multitap.accessing_memory_cards, false);
        
        // Reset
        multitap.transfer_state = TransferState::Idle;
        
        // Direct access to memory card slot 2 (0x23)
        let response = multitap.send_byte(0x23, false);
        assert_eq!(response.data, 0x80); // Multitap identification
        assert_eq!(response.request_dsr, true);
        assert_eq!(multitap.current_slot, 2);
        assert_eq!(multitap.accessing_memory_cards, true);
    }

    #[test]
    fn test_connect_controller() {
        let mut multitap = Multitap::new();
        let controller = Box::new(MockController { 
            id: 0x41, 
            button_pressed: false,
            axis_value: 0 
        });
        
        multitap.connect_controller(2, controller);
        
        // Access the connected controller directly
        multitap.current_slot = 2;
        multitap.accessing_memory_cards = false;
        multitap.transfer_state = TransferState::ForwardCommand;
        
        let response = multitap.send_byte(0x42, false);
        assert_eq!(response.data, 0x41); // Controller ID
    }

    #[test]
    fn test_button_routing() {
        let mut multitap = Multitap::new();
        
        // Connect mock controllers
        for i in 0..4 {
            let controller = Box::new(MockController {
                id: 0x40 + i as u8,
                button_pressed: false,
                axis_value: 0,
            });
            multitap.connect_controller(i, controller);
        }
        
        // Press button - should route to all controllers
        multitap.set_button(0, true);
        
        // Verify all controllers received the button press
        // Note: In real implementation, we'd need getters to verify this
        // For now, this test ensures the routing doesn't panic
    }

    #[test]
    fn test_axis_routing() {
        let mut multitap = Multitap::new();
        
        // Connect mock controllers
        for i in 0..4 {
            let controller = Box::new(MockController {
                id: 0x40 + i as u8,
                button_pressed: false,
                axis_value: 0,
            });
            multitap.connect_controller(i, controller);
        }
        
        // Set axis - should route to all controllers
        multitap.set_axis(0, 12345);
        
        // Verify all controllers received the axis value
        // Note: In real implementation, we'd need getters to verify this
        // For now, this test ensures the routing doesn't panic
    }

    #[test]
    fn test_invalid_slot_selection() {
        let mut multitap = Multitap::new();
        
        // Start multitap access
        multitap.send_byte(0x01, false);
        
        // Try to select invalid slot (4 or higher)
        let response = multitap.send_byte(5, false);
        assert_eq!(response.data, 0xFF);
        assert_eq!(response.request_dsr, false);
        assert_eq!(multitap.transfer_state, TransferState::Idle);
    }

    #[test]
    fn test_forward_command_to_peripheral() {
        let mut multitap = Multitap::new();
        
        // Connect a mock controller to slot 1
        let controller = Box::new(MockController {
            id: 0x73,
            button_pressed: false,
            axis_value: 0,
        });
        multitap.connect_controller(1, controller);
        
        // Direct access to slot 1
        multitap.send_byte(0x12, false);
        
        // Send command to the controller
        let response = multitap.send_byte(0x01, false);
        assert_eq!(response.data, 0xFF);
        assert_eq!(response.request_dsr, true);
        assert_eq!(multitap.transfer_state, TransferState::RelayData);
        
        // Send another command
        let response = multitap.send_byte(0x42, false);
        assert_eq!(response.data, 0x73); // Controller ID
    }

    #[test]
    fn test_disconnected_peripheral() {
        let mut peripheral = DisconnectedPeripheral;
        
        // Any command should return 0xFF with no DSR
        let response = peripheral.send_byte(0x42, false);
        assert_eq!(response.data, 0xFF);
        assert_eq!(response.request_dsr, false);
        
        // Clone should work
        let mut cloned = peripheral.clone_box();
        let response = cloned.send_byte(0x01, true);
        assert_eq!(response.data, 0xFF);
        assert_eq!(response.request_dsr, false);
    }

    #[test]
    fn test_multitap_clone() {
        let mut multitap = Multitap::new();
        multitap.current_slot = 3;
        multitap.accessing_memory_cards = true;
        
        let cloned = multitap.clone();
        assert_eq!(cloned.current_slot, 3);
        assert_eq!(cloned.accessing_memory_cards, true);
    }

    #[test]
    fn test_memory_card_access() {
        let mut multitap = Multitap::new();
        
        // Connect a mock memory card
        let memory_card = Box::new(MockController {
            id: 0x5A,
            button_pressed: false,
            axis_value: 0,
        });
        multitap.connect_memory_card(1, memory_card);
        
        // Direct access to memory card slot 1
        let response = multitap.send_byte(0x22, false);
        assert_eq!(response.data, 0x80);
        assert_eq!(multitap.current_slot, 1);
        assert_eq!(multitap.accessing_memory_cards, true);
        
        // Send command to memory card
        let response = multitap.send_byte(0x42, false);
        assert_eq!(response.data, 0x5A);
    }
}