//! PlayStation Link Cable Implementation
//! 
//! The link cable connects two PlayStation consoles for multiplayer gaming.
//! It uses the serial I/O port for bidirectional communication.

use super::{irq, sync, AccessWidth, Addressable, CycleCount, Psx};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const LINKSYNC: sync::SyncToken = sync::SyncToken::LinkCable;

/// Link cable connection states
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LinkState {
    Disconnected,
    Connected,
    Transmitting,
    Receiving,
    Error,
}

/// Link cable protocol modes
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LinkMode {
    /// 8-bit synchronous mode
    Sync8Bit,
    /// 16-bit synchronous mode
    Sync16Bit,
    /// Asynchronous mode
    Async,
}

/// Shared link cable buffer for inter-console communication
#[derive(Clone)]
pub struct LinkBuffer {
    /// Transmit buffer
    tx_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// Receive buffer
    rx_buffer: Arc<Mutex<VecDeque<u8>>>,
}

impl LinkBuffer {
    pub fn new() -> Self {
        LinkBuffer {
            tx_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(256))),
            rx_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(256))),
        }
    }

    /// Create a paired buffer for the other console
    pub fn create_pair(&self) -> Self {
        LinkBuffer {
            // Swap TX and RX for the other console
            tx_buffer: self.rx_buffer.clone(),
            rx_buffer: self.tx_buffer.clone(),
        }
    }
}

/// Link cable controller
#[derive(serde::Serialize, serde::Deserialize)]
pub struct LinkCable {
    /// Current connection state
    state: LinkState,
    /// Communication mode
    mode: LinkMode,
    /// Baud rate divider
    baud_rate: u16,
    /// Control register
    control: u16,
    /// Status register
    status: u16,
    /// Data register (8 or 16 bits depending on mode)
    data: u16,
    /// Transmit buffer
    tx_fifo: VecDeque<u8>,
    /// Receive buffer
    rx_fifo: VecDeque<u8>,
    /// Clock source (0 = internal, 1 = external)
    clock_source: bool,
    /// Interrupt on receive
    rx_interrupt: bool,
    /// Interrupt on transmit complete
    tx_interrupt: bool,
    /// DSR (Data Set Ready) signal
    dsr: bool,
    /// DTR (Data Terminal Ready) signal
    dtr: bool,
    /// RTS (Request To Send) signal
    rts: bool,
    /// CTS (Clear To Send) signal
    cts: bool,
    /// Shared communication buffer (for actual link cable connection)
    #[serde(skip)]
    link_buffer: Option<LinkBuffer>,
    /// Cycle counter for transmission timing
    tx_cycles: CycleCount,
    /// Cycle counter for reception timing
    rx_cycles: CycleCount,
}

impl LinkCable {
    pub fn new() -> Self {
        LinkCable {
            state: LinkState::Disconnected,
            mode: LinkMode::Sync8Bit,
            baud_rate: 0,
            control: 0,
            status: 0x0005, // TX ready, TX empty
            data: 0,
            tx_fifo: VecDeque::with_capacity(64),
            rx_fifo: VecDeque::with_capacity(64),
            clock_source: false,
            rx_interrupt: false,
            tx_interrupt: false,
            dsr: false,
            dtr: false,
            rts: false,
            cts: false,
            link_buffer: None,
            tx_cycles: 0,
            rx_cycles: 0,
        }
    }

    /// Connect to another console via link cable
    pub fn connect(&mut self, buffer: LinkBuffer) {
        self.link_buffer = Some(buffer);
        self.state = LinkState::Connected;
        self.cts = true; // Other console is ready
        info!("Link cable connected");
    }

    /// Disconnect the link cable
    pub fn disconnect(&mut self) {
        self.link_buffer = None;
        self.state = LinkState::Disconnected;
        self.cts = false;
        info!("Link cable disconnected");
    }

    /// Check if link cable is connected
    pub fn is_connected(&self) -> bool {
        self.state != LinkState::Disconnected
    }

    /// Write to control register
    pub fn set_control(&mut self, value: u16) {
        self.control = value;
        
        // Parse control bits
        self.rx_interrupt = (value & 0x0001) != 0;
        self.tx_interrupt = (value & 0x0002) != 0;
        self.dtr = (value & 0x0004) != 0;
        self.rts = (value & 0x0008) != 0;
        
        let mode_bits = (value >> 4) & 0x3;
        self.mode = match mode_bits {
            0 => LinkMode::Sync8Bit,
            1 => LinkMode::Sync16Bit,
            2 | 3 => LinkMode::Async,
            _ => LinkMode::Sync8Bit,
        };
        
        self.clock_source = (value & 0x0100) != 0;
        
        // Reset FIFOs if requested
        if (value & 0x0040) != 0 {
            self.tx_fifo.clear();
            self.status |= 0x0001; // TX ready
            self.status |= 0x0004; // TX empty
        }
        if (value & 0x0080) != 0 {
            self.rx_fifo.clear();
            self.status &= !0x0002; // RX not ready
        }
    }

    /// Read control register
    pub fn control(&self) -> u16 {
        self.control
    }

    /// Read status register
    pub fn status(&self) -> u16 {
        let mut status = self.status;
        
        // Update status bits
        if !self.tx_fifo.is_empty() {
            status &= !0x0001; // TX not ready
            status &= !0x0004; // TX not empty
        } else {
            status |= 0x0001; // TX ready
            status |= 0x0004; // TX empty
        }
        
        if !self.rx_fifo.is_empty() {
            status |= 0x0002; // RX ready
        } else {
            status &= !0x0002; // RX not ready
        }
        
        // DSR signal from other console
        if self.dsr {
            status |= 0x0080;
        }
        
        // CTS signal (other console ready)
        if self.cts {
            status |= 0x0100;
        }
        
        status
    }

    /// Write data to transmit
    pub fn write_data(&mut self, value: u16) {
        match self.mode {
            LinkMode::Sync8Bit | LinkMode::Async => {
                self.tx_fifo.push_back(value as u8);
            }
            LinkMode::Sync16Bit => {
                self.tx_fifo.push_back(value as u8);
                self.tx_fifo.push_back((value >> 8) as u8);
            }
        }
        
        self.state = LinkState::Transmitting;
        self.status &= !0x0001; // TX not ready
        self.status &= !0x0004; // TX not empty
        
        // Start transmission
        self.tx_cycles = self.calculate_tx_cycles();
    }

    /// Read received data
    pub fn read_data(&mut self) -> u16 {
        match self.mode {
            LinkMode::Sync8Bit | LinkMode::Async => {
                self.rx_fifo.pop_front().unwrap_or(0xFF) as u16
            }
            LinkMode::Sync16Bit => {
                let lo = self.rx_fifo.pop_front().unwrap_or(0xFF) as u16;
                let hi = self.rx_fifo.pop_front().unwrap_or(0xFF) as u16;
                lo | (hi << 8)
            }
        }
    }

    /// Calculate transmission cycles based on baud rate
    fn calculate_tx_cycles(&self) -> CycleCount {
        // Calculate cycles based on baud rate
        // PSX CPU is 33.8688 MHz
        let cpu_freq = 33_868_800;
        let baud_divisor = if self.baud_rate == 0 { 1 } else { self.baud_rate as i32 };
        let bits_per_byte = match self.mode {
            LinkMode::Async => 10, // Start + 8 data + stop bit
            _ => 8,
        };
        
        (cpu_freq / (baud_divisor * bits_per_byte)) as CycleCount
    }

    /// Process link cable communication
    pub fn run(&mut self, cycles: CycleCount) -> irq::IrqState {
        let mut irq = irq::IrqState::Idle;
        
        // Handle transmission
        if self.state == LinkState::Transmitting {
            self.tx_cycles -= cycles;
            if self.tx_cycles <= 0 {
                // Transmission complete
                if let Some(ref buffer) = self.link_buffer {
                    // Send data to other console
                    while let Some(byte) = self.tx_fifo.pop_front() {
                        if let Ok(mut tx) = buffer.tx_buffer.lock() {
                            tx.push_back(byte);
                        }
                    }
                }
                
                self.state = LinkState::Connected;
                self.status |= 0x0001; // TX ready
                self.status |= 0x0004; // TX empty
                
                if self.tx_interrupt {
                    irq = irq::IrqState::Active;
                }
            }
        }
        
        // Handle reception
        if let Some(ref buffer) = self.link_buffer {
            if let Ok(mut rx) = buffer.rx_buffer.lock() {
                while let Some(byte) = rx.pop_front() {
                    self.rx_fifo.push_back(byte);
                    self.status |= 0x0002; // RX ready
                    
                    if self.rx_interrupt {
                        irq = irq::IrqState::Active;
                    }
                }
            }
        }
        
        irq
    }

    /// Set baud rate
    pub fn set_baud_rate(&mut self, rate: u16) {
        self.baud_rate = rate;
    }
}

/// Link cable I/O port addresses
pub mod addresses {
    pub const SIO_DATA: u32 = 0x1F801040;
    pub const SIO_STAT: u32 = 0x1F801044;
    pub const SIO_MODE: u32 = 0x1F801048;
    pub const SIO_CTRL: u32 = 0x1F80104A;
    pub const SIO_MISC: u32 = 0x1F80104C;
    pub const SIO_BAUD: u32 = 0x1F80104E;
}

/// Load from link cable registers
pub fn load<T: Addressable>(link: &mut LinkCable, offset: u32) -> T {
    let value = match offset {
        0x00 => link.read_data() as u32,              // SIO_DATA
        0x04 => link.status() as u32,                 // SIO_STAT
        0x08 => link.mode as u32,                     // SIO_MODE
        0x0A => link.control() as u32,                // SIO_CTRL
        0x0C => 0,                                     // SIO_MISC (not used)
        0x0E => link.baud_rate as u32,                // SIO_BAUD
        _ => {
            warn!("Unknown link cable read at offset 0x{:x}", offset);
            0xFFFFFFFF
        }
    };
    
    T::from_u32(value)
}

/// Store to link cable registers
pub fn store<T: Addressable>(link: &mut LinkCable, offset: u32, value: T) {
    let val = value.as_u32();
    
    match offset {
        0x00 => link.write_data(val as u16),          // SIO_DATA
        0x08 => {                                      // SIO_MODE
            link.mode = match val & 3 {
                0 => LinkMode::Sync8Bit,
                1 => LinkMode::Sync16Bit,
                _ => LinkMode::Async,
            };
        }
        0x0A => link.set_control(val as u16),         // SIO_CTRL
        0x0E => link.set_baud_rate(val as u16),       // SIO_BAUD
        _ => {
            warn!("Unknown link cable write at offset 0x{:x} = 0x{:x}", offset, val);
        }
    }
}

/// Create a linked pair of consoles
pub fn create_linked_pair() -> (LinkCable, LinkCable) {
    let buffer1 = LinkBuffer::new();
    let buffer2 = buffer1.create_pair();
    
    let mut cable1 = LinkCable::new();
    let mut cable2 = LinkCable::new();
    
    cable1.connect(buffer1);
    cable2.connect(buffer2);
    
    (cable1, cable2)
}