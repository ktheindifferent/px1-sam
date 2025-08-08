//! PlayStation Multitap implementation
//!
//! The Multitap (SCPH-1070) allows connecting up to 4 controllers and 4 memory cards
//! to a single controller port, enabling 4-player games.

use super::{Peripheral, Response};

/// Multitap adapter state
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Multitap {
    /// Four controller slots (A, B, C, D)
    controllers: [Box<dyn Peripheral>; 4],
    /// Four memory card slots
    memory_cards: [Box<dyn Peripheral>; 4],
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

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum TransferState {
    Idle,
    SelectSlot,
    ForwardCommand,
    RelayData,
}

impl Multitap {
    pub fn new() -> Self {
        use super::gamepad::disconnected_digital_pad;
        use super::memory_card::disconnected_memory_card;

        Multitap {
            controllers: [
                disconnected_digital_pad(),
                disconnected_digital_pad(),
                disconnected_digital_pad(),
                disconnected_digital_pad(),
            ],
            memory_cards: [
                disconnected_memory_card(),
                disconnected_memory_card(),
                disconnected_memory_card(),
                disconnected_memory_card(),
            ],
            current_slot: 0,
            accessing_memory_cards: false,
            transfer_state: TransferState::Idle,
            data_buffer: Vec::with_capacity(256),
            buffer_position: 0,
        }
    }

    /// Connect a controller to the specified slot (0-3)
    pub fn connect_controller(&mut self, slot: usize, controller: Box<dyn Peripheral>) {
        if slot < 4 {
            self.controllers[slot] = controller;
            info!("Multitap: Connected controller to slot {}", slot);
        }
    }

    /// Connect a memory card to the specified slot (0-3)
    pub fn connect_memory_card(&mut self, slot: usize, memory_card: Box<dyn Peripheral>) {
        if slot < 4 {
            self.memory_cards[slot] = memory_card;
            info!("Multitap: Connected memory card to slot {}", slot);
        }
    }

    /// Get the currently selected peripheral
    fn current_peripheral(&mut self) -> &mut Box<dyn Peripheral> {
        if self.accessing_memory_cards {
            &mut self.memory_cards[self.current_slot as usize]
        } else {
            &mut self.controllers[self.current_slot as usize]
        }
    }
}

impl Peripheral for Multitap {
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
}

/// Create a new Multitap peripheral
pub fn multitap() -> Box<dyn Peripheral> {
    Box::new(Multitap::new())
}

/// Create a Multitap with 4 controllers connected
pub fn multitap_with_controllers(controllers: [Box<dyn Peripheral>; 4]) -> Box<dyn Peripheral> {
    use super::memory_card::disconnected_memory_card;
    
    let mut tap = Multitap::new();
    tap.controllers = controllers;
    tap.memory_cards = [
        disconnected_memory_card(),
        disconnected_memory_card(),
        disconnected_memory_card(),
        disconnected_memory_card(),
    ];
    Box::new(tap)
}