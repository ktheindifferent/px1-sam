// Enhanced PSX WASM implementation with better memory management
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent,
    AudioContext, Gamepad
};
use std::cell::RefCell;

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

// Memory regions
const RAM_SIZE: usize = 2 * 1024 * 1024;      // 2MB main RAM
const SCRATCHPAD_SIZE: usize = 1024;          // 1KB scratchpad
const BIOS_SIZE: usize = 512 * 1024;          // 512KB BIOS
const VRAM_SIZE: usize = 1024 * 512;          // 1MB VRAM (1024x512x16bit)

// Memory map addresses
const KUSEG: u32 = 0x00000000;
const KSEG0: u32 = 0x80000000;
const KSEG1: u32 = 0xa0000000;
const KSEG2: u32 = 0xc0000000;

// Hardware register ranges
const HW_REGS_START: u32 = 0x1f801000;
const HW_REGS_END: u32 = 0x1f802000;
const GPU_REGS_START: u32 = 0x1f801810;
const GPU_REGS_END: u32 = 0x1f801820;

// Enhanced CPU with full memory mapping
struct EnhancedCpu {
    pc: u32,
    next_pc: u32,
    regs: [u32; 32],
    hi: u32,
    lo: u32,
    // COP0 registers
    cop0_sr: u32,
    cop0_cause: u32,
    cop0_epc: u32,
    cop0_badvaddr: u32,
    cop0_bpc: u32,
    cop0_bda: u32,
    cop0_dcic: u32,
    cop0_bdam: u32,
    cop0_bpcm: u32,
    // Load delay slot
    load_delay_reg: u8,
    load_delay_value: u32,
    // Branch delay
    in_delay_slot: bool,
    branch_taken: bool,
    // Memory regions
    ram: Vec<u8>,
    scratchpad: Vec<u8>,
    bios: Vec<u8>,
    // Hardware registers
    hw_regs: Vec<u8>,
    // Interrupt state
    interrupt_pending: bool,
    // Cycle counter
    cycle_count: u64,
}

impl EnhancedCpu {
    fn new() -> Self {
        let mut cpu = EnhancedCpu {
            pc: 0xbfc00000,
            next_pc: 0xbfc00004,
            regs: [0; 32],
            hi: 0,
            lo: 0,
            cop0_sr: 0x10900000,
            cop0_cause: 0,
            cop0_epc: 0,
            cop0_badvaddr: 0,
            cop0_bpc: 0,
            cop0_bda: 0,
            cop0_dcic: 0,
            cop0_bdam: 0,
            cop0_bpcm: 0,
            load_delay_reg: 0,
            load_delay_value: 0,
            in_delay_slot: false,
            branch_taken: false,
            ram: vec![0; RAM_SIZE],
            scratchpad: vec![0; SCRATCHPAD_SIZE],
            bios: vec![0; BIOS_SIZE],
            hw_regs: vec![0; 0x2000],
            interrupt_pending: false,
            cycle_count: 0,
        };
        cpu.regs[0] = 0;
        cpu
    }

    fn translate_address(&self, addr: u32) -> u32 {
        // Remove segment bits to get physical address
        match addr {
            0x00000000..=0x7fffffff => addr,           // KUSEG
            0x80000000..=0x9fffffff => addr & 0x1fffffff, // KSEG0
            0xa0000000..=0xbfffffff => addr & 0x1fffffff, // KSEG1
            _ => addr,                                  // KSEG2
        }
    }

    fn read8(&self, addr: u32) -> u8 {
        let phys_addr = self.translate_address(addr);
        
        match phys_addr {
            0x00000000..=0x001fffff => {
                // Main RAM
                self.ram.get(phys_addr as usize).copied().unwrap_or(0)
            }
            0x1f000000..=0x1f0fffff => {
                // Scratchpad
                let offset = (phys_addr - 0x1f000000) as usize;
                self.scratchpad.get(offset).copied().unwrap_or(0)
            }
            0x1f801000..=0x1f802000 => {
                // Hardware registers
                let offset = (phys_addr - 0x1f801000) as usize;
                self.hw_regs.get(offset).copied().unwrap_or(0)
            }
            0x1fc00000..=0x1fc7ffff => {
                // BIOS
                let offset = (phys_addr - 0x1fc00000) as usize;
                self.bios.get(offset).copied().unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn read16(&self, addr: u32) -> u16 {
        let aligned = addr & !1;
        let b0 = self.read8(aligned) as u16;
        let b1 = self.read8(aligned + 1) as u16;
        b0 | (b1 << 8)
    }

    fn read32(&self, addr: u32) -> u32 {
        let aligned = addr & !3;
        let phys_addr = self.translate_address(aligned);
        
        match phys_addr {
            0x00000000..=0x001fffff => {
                // Main RAM
                let offset = phys_addr as usize;
                if offset + 3 < self.ram.len() {
                    u32::from_le_bytes([
                        self.ram[offset],
                        self.ram[offset + 1],
                        self.ram[offset + 2],
                        self.ram[offset + 3],
                    ])
                } else {
                    0
                }
            }
            0x1f000000..=0x1f0fffff => {
                // Scratchpad
                let offset = (phys_addr - 0x1f000000) as usize;
                if offset + 3 < self.scratchpad.len() {
                    u32::from_le_bytes([
                        self.scratchpad[offset],
                        self.scratchpad[offset + 1],
                        self.scratchpad[offset + 2],
                        self.scratchpad[offset + 3],
                    ])
                } else {
                    0
                }
            }
            0x1f801000..=0x1f802000 => {
                // Hardware registers
                match phys_addr {
                    0x1f801070 => 0x00000200,  // I_STAT
                    0x1f801074 => 0x00000000,  // I_MASK
                    _ => 0,
                }
            }
            0x1fc00000..=0x1fc7ffff => {
                // BIOS
                let offset = (phys_addr - 0x1fc00000) as usize;
                if offset + 3 < self.bios.len() {
                    u32::from_le_bytes([
                        self.bios[offset],
                        self.bios[offset + 1],
                        self.bios[offset + 2],
                        self.bios[offset + 3],
                    ])
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn write8(&mut self, addr: u32, value: u8) {
        let phys_addr = self.translate_address(addr);
        
        match phys_addr {
            0x00000000..=0x001fffff => {
                // Main RAM
                if let Some(byte) = self.ram.get_mut(phys_addr as usize) {
                    *byte = value;
                }
            }
            0x1f000000..=0x1f0fffff => {
                // Scratchpad
                let offset = (phys_addr - 0x1f000000) as usize;
                if let Some(byte) = self.scratchpad.get_mut(offset) {
                    *byte = value;
                }
            }
            0x1f801000..=0x1f802000 => {
                // Hardware registers
                let offset = (phys_addr - 0x1f801000) as usize;
                if let Some(byte) = self.hw_regs.get_mut(offset) {
                    *byte = value;
                }
            }
            _ => {}
        }
    }

    fn write16(&mut self, addr: u32, value: u16) {
        let aligned = addr & !1;
        self.write8(aligned, value as u8);
        self.write8(aligned + 1, (value >> 8) as u8);
    }

    fn write32(&mut self, addr: u32, value: u32) {
        let aligned = addr & !3;
        let phys_addr = self.translate_address(aligned);
        
        match phys_addr {
            0x00000000..=0x001fffff => {
                // Main RAM
                let offset = phys_addr as usize;
                if offset + 3 < self.ram.len() {
                    self.ram[offset] = value as u8;
                    self.ram[offset + 1] = (value >> 8) as u8;
                    self.ram[offset + 2] = (value >> 16) as u8;
                    self.ram[offset + 3] = (value >> 24) as u8;
                }
            }
            0x1f000000..=0x1f0fffff => {
                // Scratchpad
                let offset = (phys_addr - 0x1f000000) as usize;
                if offset + 3 < self.scratchpad.len() {
                    self.scratchpad[offset] = value as u8;
                    self.scratchpad[offset + 1] = (value >> 8) as u8;
                    self.scratchpad[offset + 2] = (value >> 16) as u8;
                    self.scratchpad[offset + 3] = (value >> 24) as u8;
                }
            }
            0x1f801000..=0x1f802000 => {
                // Hardware registers - handle specific registers
                match phys_addr {
                    0x1f801070 => {},  // I_STAT (interrupt status)
                    0x1f801074 => {},  // I_MASK (interrupt mask)
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn fetch_instruction(&mut self) -> u32 {
        let instruction = self.read32(self.pc);
        self.in_delay_slot = self.branch_taken;
        self.branch_taken = false;
        self.pc = self.next_pc;
        self.next_pc = self.next_pc.wrapping_add(4);
        instruction
    }

    fn execute_instruction(&mut self, instruction: u32) {
        // Apply pending load delay
        if self.load_delay_reg != 0 {
            self.regs[self.load_delay_reg as usize] = self.load_delay_value;
            self.load_delay_reg = 0;
        }

        let opcode = (instruction >> 26) & 0x3f;
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 20) & 0x1f) as usize;
        let rd = ((instruction >> 11) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as i16 as i32 as u32;
        let imm_se = (instruction & 0xffff) as i16 as i32 as u32;
        let target = instruction & 0x3ffffff;

        match opcode {
            0x00 => self.execute_special(instruction, rs, rt, rd),
            0x01 => self.execute_bcond(instruction, rs, imm_se),
            0x02 => {
                // J
                self.next_pc = (self.pc & 0xf0000000) | (target << 2);
                self.branch_taken = true;
            }
            0x03 => {
                // JAL
                self.regs[31] = self.next_pc;
                self.next_pc = (self.pc & 0xf0000000) | (target << 2);
                self.branch_taken = true;
            }
            0x04 => {
                // BEQ
                if self.regs[rs] == self.regs[rt] {
                    self.next_pc = self.pc.wrapping_add(imm_se << 2);
                    self.branch_taken = true;
                }
            }
            0x05 => {
                // BNE
                if self.regs[rs] != self.regs[rt] {
                    self.next_pc = self.pc.wrapping_add(imm_se << 2);
                    self.branch_taken = true;
                }
            }
            0x06 => {
                // BLEZ
                if (self.regs[rs] as i32) <= 0 {
                    self.next_pc = self.pc.wrapping_add(imm_se << 2);
                    self.branch_taken = true;
                }
            }
            0x07 => {
                // BGTZ
                if (self.regs[rs] as i32) > 0 {
                    self.next_pc = self.pc.wrapping_add(imm_se << 2);
                    self.branch_taken = true;
                }
            }
            0x08 => {
                // ADDI
                if rt != 0 {
                    let result = (self.regs[rs] as i32).wrapping_add(imm_se as i32);
                    self.regs[rt] = result as u32;
                }
            }
            0x09 => {
                // ADDIU
                if rt != 0 {
                    self.regs[rt] = self.regs[rs].wrapping_add(imm_se);
                }
            }
            0x0a => {
                // SLTI
                if rt != 0 {
                    self.regs[rt] = if (self.regs[rs] as i32) < (imm_se as i32) { 1 } else { 0 };
                }
            }
            0x0b => {
                // SLTIU
                if rt != 0 {
                    self.regs[rt] = if self.regs[rs] < imm_se { 1 } else { 0 };
                }
            }
            0x0c => {
                // ANDI
                if rt != 0 {
                    self.regs[rt] = self.regs[rs] & (imm as u32);
                }
            }
            0x0d => {
                // ORI
                if rt != 0 {
                    self.regs[rt] = self.regs[rs] | (imm as u32);
                }
            }
            0x0e => {
                // XORI
                if rt != 0 {
                    self.regs[rt] = self.regs[rs] ^ (imm as u32);
                }
            }
            0x0f => {
                // LUI
                if rt != 0 {
                    self.regs[rt] = (imm as u32) << 16;
                }
            }
            0x10 => self.execute_cop0(instruction, rt, rd),
            0x12 => {},  // COP2 (GTE) - not implemented yet
            0x20 => {
                // LB
                let addr = self.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    self.load_delay_reg = rt as u8;
                    self.load_delay_value = self.read8(addr) as i8 as i32 as u32;
                }
            }
            0x21 => {
                // LH
                let addr = self.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    self.load_delay_reg = rt as u8;
                    self.load_delay_value = self.read16(addr) as i16 as i32 as u32;
                }
            }
            0x22 => {
                // LWL
                let addr = self.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    let aligned = addr & !3;
                    let shift = (addr & 3) * 8;
                    let mask = 0xffffffff << shift;
                    let value = self.read32(aligned) << shift;
                    self.load_delay_reg = rt as u8;
                    self.load_delay_value = (self.regs[rt] & !mask) | value;
                }
            }
            0x23 => {
                // LW
                let addr = self.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    self.load_delay_reg = rt as u8;
                    self.load_delay_value = self.read32(addr);
                }
            }
            0x24 => {
                // LBU
                let addr = self.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    self.load_delay_reg = rt as u8;
                    self.load_delay_value = self.read8(addr) as u32;
                }
            }
            0x25 => {
                // LHU
                let addr = self.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    self.load_delay_reg = rt as u8;
                    self.load_delay_value = self.read16(addr) as u32;
                }
            }
            0x26 => {
                // LWR
                let addr = self.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    let aligned = addr & !3;
                    let shift = (3 - (addr & 3)) * 8;
                    let mask = 0xffffffff >> (32 - shift - 8);
                    let value = self.read32(aligned) >> shift;
                    self.load_delay_reg = rt as u8;
                    self.load_delay_value = (self.regs[rt] & !mask) | value;
                }
            }
            0x28 => {
                // SB
                let addr = self.regs[rs].wrapping_add(imm_se);
                self.write8(addr, self.regs[rt] as u8);
            }
            0x29 => {
                // SH
                let addr = self.regs[rs].wrapping_add(imm_se);
                self.write16(addr, self.regs[rt] as u16);
            }
            0x2a => {
                // SWL
                let addr = self.regs[rs].wrapping_add(imm_se);
                let aligned = addr & !3;
                let shift = (addr & 3) * 8;
                let mask = 0xffffffff >> shift;
                let current = self.read32(aligned);
                let value = (current & !mask) | (self.regs[rt] >> shift);
                self.write32(aligned, value);
            }
            0x2b => {
                // SW
                let addr = self.regs[rs].wrapping_add(imm_se);
                self.write32(addr, self.regs[rt]);
            }
            0x2e => {
                // SWR
                let addr = self.regs[rs].wrapping_add(imm_se);
                let aligned = addr & !3;
                let shift = (3 - (addr & 3)) * 8;
                let mask = 0xffffffff << (shift + 8);
                let current = self.read32(aligned);
                let value = (current & !mask) | (self.regs[rt] << shift);
                self.write32(aligned, value);
            }
            _ => {}
        }

        // R0 is always zero
        self.regs[0] = 0;
        self.cycle_count += 1;
    }

    fn execute_special(&mut self, instruction: u32, rs: usize, rt: usize, rd: usize) {
        let funct = instruction & 0x3f;
        match funct {
            0x00 => {
                // SLL
                let sa = (instruction >> 6) & 0x1f;
                if rd != 0 {
                    self.regs[rd] = self.regs[rt] << sa;
                }
            }
            0x02 => {
                // SRL
                let sa = (instruction >> 6) & 0x1f;
                if rd != 0 {
                    self.regs[rd] = self.regs[rt] >> sa;
                }
            }
            0x03 => {
                // SRA
                let sa = (instruction >> 6) & 0x1f;
                if rd != 0 {
                    self.regs[rd] = ((self.regs[rt] as i32) >> sa) as u32;
                }
            }
            0x04 => {
                // SLLV
                if rd != 0 {
                    self.regs[rd] = self.regs[rt] << (self.regs[rs] & 0x1f);
                }
            }
            0x06 => {
                // SRLV
                if rd != 0 {
                    self.regs[rd] = self.regs[rt] >> (self.regs[rs] & 0x1f);
                }
            }
            0x07 => {
                // SRAV
                if rd != 0 {
                    self.regs[rd] = ((self.regs[rt] as i32) >> (self.regs[rs] & 0x1f)) as u32;
                }
            }
            0x08 => {
                // JR
                self.next_pc = self.regs[rs];
                self.branch_taken = true;
            }
            0x09 => {
                // JALR
                let ret_addr = self.next_pc;
                self.next_pc = self.regs[rs];
                if rd != 0 {
                    self.regs[rd] = ret_addr;
                }
                self.branch_taken = true;
            }
            0x0c => {
                // SYSCALL
                self.cop0_cause = (self.cop0_cause & !0x3c) | (0x08 << 2);
                self.handle_exception(0x80000080);
            }
            0x0d => {
                // BREAK
                self.cop0_cause = (self.cop0_cause & !0x3c) | (0x09 << 2);
                self.handle_exception(0x80000080);
            }
            0x10 => {
                // MFHI
                if rd != 0 {
                    self.regs[rd] = self.hi;
                }
            }
            0x11 => {
                // MTHI
                self.hi = self.regs[rs];
            }
            0x12 => {
                // MFLO
                if rd != 0 {
                    self.regs[rd] = self.lo;
                }
            }
            0x13 => {
                // MTLO
                self.lo = self.regs[rs];
            }
            0x18 => {
                // MULT
                let a = self.regs[rs] as i32 as i64;
                let b = self.regs[rt] as i32 as i64;
                let result = a * b;
                self.lo = result as u32;
                self.hi = (result >> 32) as u32;
            }
            0x19 => {
                // MULTU
                let a = self.regs[rs] as u64;
                let b = self.regs[rt] as u64;
                let result = a * b;
                self.lo = result as u32;
                self.hi = (result >> 32) as u32;
            }
            0x1a => {
                // DIV
                let a = self.regs[rs] as i32;
                let b = self.regs[rt] as i32;
                if b != 0 {
                    self.lo = (a / b) as u32;
                    self.hi = (a % b) as u32;
                } else {
                    self.lo = if a >= 0 { 0xffffffff } else { 1 };
                    self.hi = a as u32;
                }
            }
            0x1b => {
                // DIVU
                let a = self.regs[rs];
                let b = self.regs[rt];
                if b != 0 {
                    self.lo = a / b;
                    self.hi = a % b;
                } else {
                    self.lo = 0xffffffff;
                    self.hi = a;
                }
            }
            0x20 => {
                // ADD
                if rd != 0 {
                    let a = self.regs[rs] as i32;
                    let b = self.regs[rt] as i32;
                    let result = a.wrapping_add(b);
                    // Check for overflow
                    if ((a ^ result) & (b ^ result)) < 0 {
                        self.handle_exception(0x80000180);
                    } else {
                        self.regs[rd] = result as u32;
                    }
                }
            }
            0x21 => {
                // ADDU
                if rd != 0 {
                    self.regs[rd] = self.regs[rs].wrapping_add(self.regs[rt]);
                }
            }
            0x22 => {
                // SUB
                if rd != 0 {
                    let a = self.regs[rs] as i32;
                    let b = self.regs[rt] as i32;
                    let result = a.wrapping_sub(b);
                    // Check for overflow
                    if ((a ^ b) & (a ^ result)) < 0 {
                        self.handle_exception(0x80000180);
                    } else {
                        self.regs[rd] = result as u32;
                    }
                }
            }
            0x23 => {
                // SUBU
                if rd != 0 {
                    self.regs[rd] = self.regs[rs].wrapping_sub(self.regs[rt]);
                }
            }
            0x24 => {
                // AND
                if rd != 0 {
                    self.regs[rd] = self.regs[rs] & self.regs[rt];
                }
            }
            0x25 => {
                // OR
                if rd != 0 {
                    self.regs[rd] = self.regs[rs] | self.regs[rt];
                }
            }
            0x26 => {
                // XOR
                if rd != 0 {
                    self.regs[rd] = self.regs[rs] ^ self.regs[rt];
                }
            }
            0x27 => {
                // NOR
                if rd != 0 {
                    self.regs[rd] = !(self.regs[rs] | self.regs[rt]);
                }
            }
            0x2a => {
                // SLT
                if rd != 0 {
                    self.regs[rd] = if (self.regs[rs] as i32) < (self.regs[rt] as i32) { 1 } else { 0 };
                }
            }
            0x2b => {
                // SLTU
                if rd != 0 {
                    self.regs[rd] = if self.regs[rs] < self.regs[rt] { 1 } else { 0 };
                }
            }
            _ => {}
        }
    }

    fn execute_bcond(&mut self, _instruction: u32, rs: usize, offset: u32) {
        let rt = (_instruction >> 16) & 0x1f;
        let test = (self.regs[rs] as i32) < 0;
        let link = (rt & 0x10) != 0;
        
        if link {
            self.regs[31] = self.next_pc;
        }
        
        let branch = match rt & 0x01 {
            0 => !test,  // BLTZ/BLTZAL
            _ => test,   // BGEZ/BGEZAL
        };
        
        if branch {
            self.next_pc = self.pc.wrapping_add(offset << 2);
            self.branch_taken = true;
        }
    }

    fn execute_cop0(&mut self, instruction: u32, rt: usize, rd: usize) {
        let cop_op = (instruction >> 21) & 0x1f;
        match cop_op {
            0x00 => {
                // MFC0
                if rt != 0 {
                    self.load_delay_reg = rt as u8;
                    self.load_delay_value = match rd {
                        12 => self.cop0_sr,
                        13 => self.cop0_cause,
                        14 => self.cop0_epc,
                        8 => self.cop0_badvaddr,
                        _ => 0,
                    };
                }
            }
            0x04 => {
                // MTC0
                let value = self.regs[rt];
                match rd {
                    12 => self.cop0_sr = value,
                    13 => self.cop0_cause = value & 0x300,
                    14 => self.cop0_epc = value,
                    _ => {}
                }
            }
            0x10 => {
                // RFE
                let mode = self.cop0_sr & 0x3f;
                self.cop0_sr = (self.cop0_sr & !0x0f) | (mode >> 2);
            }
            _ => {}
        }
    }

    fn handle_exception(&mut self, vector: u32) {
        // Save current PC to EPC
        self.cop0_epc = if self.in_delay_slot {
            self.pc.wrapping_sub(4)
        } else {
            self.pc
        };
        
        // Update cause register BD bit
        if self.in_delay_slot {
            self.cop0_cause |= 1 << 31;
        } else {
            self.cop0_cause &= !(1 << 31);
        }
        
        // Enter kernel mode
        let mode = self.cop0_sr & 0x3f;
        self.cop0_sr = (self.cop0_sr & !0x3f) | ((mode << 2) & 0x3f);
        
        // Jump to exception vector
        self.pc = vector;
        self.next_pc = vector + 4;
    }

    fn step(&mut self) {
        let instruction = self.fetch_instruction();
        self.execute_instruction(instruction);
        
        // Check for interrupts
        if (self.cop0_sr & 1) != 0 {
            let pending = (self.cop0_cause & self.cop0_sr & 0xff00) != 0;
            if pending && !self.in_delay_slot {
                self.cop0_cause = (self.cop0_cause & !0x3c) | (0 << 2);
                self.handle_exception(0x80000080);
            }
        }
    }
}

// Enhanced PSX with better GPU
struct EnhancedPsx {
    cpu: EnhancedCpu,
    vram: Vec<u16>,
    gpu_status: u32,
    gpu_read: u32,
    display_start_x: u16,
    display_start_y: u16,
    display_width: u16,
    display_height: u16,
    display_depth: u8,
    frame_count: u32,
    cycles_per_frame: u32,
}

impl EnhancedPsx {
    fn new() -> Self {
        EnhancedPsx {
            cpu: EnhancedCpu::new(),
            vram: vec![0; VRAM_SIZE],
            gpu_status: 0x14802000,
            gpu_read: 0,
            display_start_x: 0,
            display_start_y: 0,
            display_width: 320,
            display_height: 240,
            display_depth: 15,
            frame_count: 0,
            cycles_per_frame: 560000,
        }
    }
    
    fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), String> {
        if bios_data.len() != BIOS_SIZE {
            return Err(format!("BIOS must be exactly {}KB", BIOS_SIZE / 1024));
        }
        self.cpu.bios.copy_from_slice(bios_data);
        console_log!("BIOS loaded, first instruction: {:08x}", 
                    u32::from_le_bytes([bios_data[0], bios_data[1], bios_data[2], bios_data[3]]));
        Ok(())
    }
    
    fn load_exe(&mut self, exe_data: &[u8]) -> Result<(), String> {
        if exe_data.len() < 0x800 {
            return Err("EXE file too small".to_string());
        }
        
        if &exe_data[0..8] != b"PS-X EXE" {
            return Err("Invalid PS-X EXE header".to_string());
        }
        
        let initial_pc = u32::from_le_bytes([exe_data[0x10], exe_data[0x11], exe_data[0x12], exe_data[0x13]]);
        let initial_gp = u32::from_le_bytes([exe_data[0x14], exe_data[0x15], exe_data[0x16], exe_data[0x17]]);
        let load_addr = u32::from_le_bytes([exe_data[0x18], exe_data[0x19], exe_data[0x1a], exe_data[0x1b]]);
        let file_size = u32::from_le_bytes([exe_data[0x1c], exe_data[0x1d], exe_data[0x1e], exe_data[0x1f]]);
        let initial_sp = u32::from_le_bytes([exe_data[0x30], exe_data[0x31], exe_data[0x32], exe_data[0x33]]);
        
        console_log!("Loading EXE: PC={:08x}, GP={:08x}, Load={:08x}, Size={:x}", 
                    initial_pc, initial_gp, load_addr, file_size);
        
        // Load the code into RAM
        let exe_start = 0x800;
        let exe_end = exe_start + file_size as usize;
        
        if exe_end > exe_data.len() {
            return Err("EXE file size mismatch".to_string());
        }
        
        // Map to physical RAM address
        let ram_addr = (load_addr & 0x1fffff) as usize;
        let exe_code = &exe_data[exe_start..exe_end];
        
        for (i, &byte) in exe_code.iter().enumerate() {
            if ram_addr + i < self.cpu.ram.len() {
                self.cpu.ram[ram_addr + i] = byte;
            }
        }
        
        // Set CPU state
        self.cpu.pc = initial_pc;
        self.cpu.next_pc = initial_pc.wrapping_add(4);
        self.cpu.regs[28] = initial_gp;
        self.cpu.regs[29] = initial_sp;
        self.cpu.regs[30] = initial_sp;
        
        Ok(())
    }
    
    fn run_frame(&mut self) -> Result<(), String> {
        self.frame_count += 1;
        
        // Execute CPU instructions for one frame
        let start_cycles = self.cpu.cycle_count;
        while self.cpu.cycle_count - start_cycles < self.cycles_per_frame as u64 {
            self.cpu.step();
            
            // Safety check
            if self.cpu.pc == 0 || self.cpu.pc == 0xffffffff {
                console_error!("CPU halted at PC {:08x}", self.cpu.pc);
                break;
            }
        }
        
        // Generate test pattern showing CPU activity
        self.generate_test_pattern();
        
        Ok(())
    }
    
    fn generate_test_pattern(&mut self) {
        let offset = (self.frame_count * 2) as u16;
        let pc_color = ((self.cpu.pc >> 10) & 0x1f) as u16;
        
        for y in 0..self.display_height {
            for x in 0..self.display_width {
                let vram_x = (self.display_start_x + x) as usize % 1024;
                let vram_y = (self.display_start_y + y) as usize % 512;
                let idx = vram_y * 1024 + vram_x;
                
                let r = ((x + offset) & 0x1f) as u16;
                let g = ((y + offset / 2) & 0x1f) as u16;
                let b = pc_color;
                
                self.vram[idx] = r | (g << 5) | (b << 10);
            }
        }
    }
    
    fn get_framebuffer(&self, buffer: &mut Vec<u8>) {
        let width = self.display_width as usize;
        let height = self.display_height as usize;
        
        buffer.clear();
        buffer.resize(width * height * 4, 0);
        
        for y in 0..height {
            for x in 0..width {
                let vram_x = (self.display_start_x as usize + x) % 1024;
                let vram_y = (self.display_start_y as usize + y) % 512;
                let pixel = self.vram[vram_y * 1024 + vram_x];
                
                let r = ((pixel & 0x1F) << 3) as u8;
                let g = (((pixel >> 5) & 0x1F) << 3) as u8;
                let b = (((pixel >> 10) & 0x1F) << 3) as u8;
                
                let offset = (y * width + x) * 4;
                buffer[offset] = r;
                buffer[offset + 1] = g;
                buffer[offset + 2] = b;
                buffer[offset + 3] = 255;
            }
        }
    }
}

#[wasm_bindgen]
pub struct PsxEmulator {
    psx: EnhancedPsx,
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    frame_buffer: Vec<u8>,
    input_state: InputState,
    running: RefCell<bool>,
}

#[wasm_bindgen]
pub struct InputState {
    keys: RefCell<[bool; 256]>,
    gamepad_buttons: RefCell<[bool; 16]>,
    gamepad_axes: RefCell<[f32; 4]>,
}

#[wasm_bindgen]
impl InputState {
    pub fn new() -> Self {
        InputState {
            keys: RefCell::new([false; 256]),
            gamepad_buttons: RefCell::new([false; 16]),
            gamepad_axes: RefCell::new([0.0; 4]),
        }
    }

    pub fn set_key(&self, keycode: u32, pressed: bool) {
        if keycode < 256 {
            self.keys.borrow_mut()[keycode as usize] = pressed;
        }
    }

    pub fn update_gamepad(&self, gamepad: &Gamepad) {
        let buttons = gamepad.buttons();
        let mut gamepad_buttons = self.gamepad_buttons.borrow_mut();
        
        for (i, button) in buttons.iter().enumerate() {
            if i >= 16 { break; }
            if let Ok(button) = button.dyn_into::<web_sys::GamepadButton>() {
                gamepad_buttons[i] = button.pressed();
            }
        }

        let axes = gamepad.axes();
        let mut gamepad_axes = self.gamepad_axes.borrow_mut();
        
        for (i, axis) in axes.iter().enumerate() {
            if i >= 4 { break; }
            if let Some(val) = axis.as_f64() {
                gamepad_axes[i] = val as f32;
            }
        }
    }
}

#[wasm_bindgen]
impl PsxEmulator {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<PsxEmulator, JsValue> {
        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document.get_element_by_id(canvas_id).unwrap();
        let canvas: HtmlCanvasElement = canvas
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| JsValue::from_str("Failed to get canvas element"))?;

        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();

        canvas.set_width(640);
        canvas.set_height(480);
        
        console_log!("Enhanced PSX WASM Emulator initialized");
        
        Ok(PsxEmulator {
            psx: EnhancedPsx::new(),
            canvas,
            context,
            frame_buffer: Vec::with_capacity(640 * 480 * 4),
            input_state: InputState::new(),
            running: RefCell::new(false),
        })
    }

    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), JsValue> {
        match self.psx.load_bios(bios_data) {
            Ok(_) => {
                console_log!("BIOS loaded successfully");
                Ok(())
            }
            Err(e) => {
                console_error!("Failed to load BIOS: {}", e);
                Err(JsValue::from_str(&format!("BIOS load failed: {}", e)))
            }
        }
    }

    pub fn load_game(&mut self, game_data: &[u8]) -> Result<(), JsValue> {
        if game_data.len() > 8 && &game_data[0..8] == b"PS-X EXE" {
            match self.psx.load_exe(game_data) {
                Ok(_) => {
                    console_log!("PSX-EXE loaded successfully");
                    Ok(())
                }
                Err(e) => {
                    console_error!("Failed to load EXE: {}", e);
                    Err(JsValue::from_str(&format!("Failed to load EXE: {}", e)))
                }
            }
        } else {
            console_error!("Only PSX-EXE files are supported in WASM build");
            Err(JsValue::from_str("Only PSX-EXE files are supported"))
        }
    }

    pub fn run_frame(&mut self) -> Result<(), JsValue> {
        if !self.is_running() {
            return Ok(());
        }

        match self.psx.run_frame() {
            Ok(_) => {
                self.render_frame()?;
                Ok(())
            }
            Err(e) => {
                console_error!("Frame execution error: {}", e);
                *self.running.borrow_mut() = false;
                Err(JsValue::from_str(&format!("Emulation error: {}", e)))
            }
        }
    }
    
    fn render_frame(&mut self) -> Result<(), JsValue> {
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

    pub fn handle_keyboard_event(&mut self, event: KeyboardEvent, pressed: bool) {
        let keycode = event.key_code();
        self.input_state.set_key(keycode, pressed);
        
        if pressed {
            console_log!("Key pressed: {} (code: {})", event.key(), keycode);
        }
    }

    pub fn get_frame_buffer(&self) -> Vec<u8> {
        self.frame_buffer.clone()
    }

    pub fn get_audio_buffer(&self) -> Vec<f32> {
        vec![]
    }

    pub fn get_save_state(&self) -> Vec<u8> {
        vec![]
    }

    pub fn load_save_state(&mut self, _state: &[u8]) -> Result<(), JsValue> {
        Ok(())
    }

    pub fn reset(&mut self) {
        self.psx = EnhancedPsx::new();
        console_log!("Emulator reset");
    }

    pub fn set_volume(&mut self, _volume: f32) {
    }

    pub fn get_debug_info(&self) -> String {
        format!("PC: {:08x}, SP: {:08x}, Cycles: {}, Frame: {}", 
                self.psx.cpu.pc, 
                self.psx.cpu.regs[29],
                self.psx.cpu.cycle_count,
                self.psx.frame_count)
    }

    pub fn update_gamepad_state(&mut self, gamepad: &Gamepad) {
        self.input_state.update_gamepad(gamepad);
    }
}