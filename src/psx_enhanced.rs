// Enhanced PSX emulator implementation for WASM
// This provides a more complete emulation than the stub version

use super::error::{PsxError, Result};
use std::collections::HashMap;

// Simple logging macro for WASM
mod log {
    pub fn warn(_msg: &str) {}
    pub fn info(_msg: &str) {}
    pub fn error(_msg: &str) {}
}

macro_rules! log {
    (warn, $($arg:tt)*) => { log::warn(&format!($($arg)*)); };
    (info, $($arg:tt)*) => { log::info(&format!($($arg)*)); };
    (error, $($arg:tt)*) => { log::error(&format!($($arg)*)); };
}

// MIPS R3000A CPU implementation
pub struct Cpu {
    pub pc: u32,
    pub next_pc: u32,
    pub regs: [u32; 32],
    pub hi: u32,
    pub lo: u32,
    pub cop0_regs: [u32; 32],
    pub load_delay_slot: Option<(u8, u32)>,
    pub branch_delay: bool,
    pub exception_pending: bool,
}

impl Cpu {
    pub fn new() -> Self {
        let mut cpu = Cpu {
            pc: 0xbfc00000,        // BIOS entry point
            next_pc: 0xbfc00004,
            regs: [0; 32],
            hi: 0,
            lo: 0,
            cop0_regs: [0; 32],
            load_delay_slot: None,
            branch_delay: false,
            exception_pending: false,
        };
        
        // Initialize COP0 registers
        cpu.cop0_regs[12] = 0x10900000; // Status register
        cpu.cop0_regs[15] = 0x00000002; // PRID (processor ID)
        
        cpu
    }
    
    pub fn step(&mut self, bus: &mut Bus) -> Result<()> {
        // Handle load delay slot
        if let Some((reg, val)) = self.load_delay_slot {
            if reg != 0 {
                self.regs[reg as usize] = val;
            }
            self.load_delay_slot = None;
        }
        
        // Fetch instruction
        let instruction = bus.load32(self.pc)?;
        
        // Update PC
        self.pc = self.next_pc;
        self.next_pc = self.pc.wrapping_add(4);
        
        // Decode and execute
        self.execute_instruction(instruction, bus)?;
        
        // R0 is always 0
        self.regs[0] = 0;
        
        Ok(())
    }
    
    fn execute_instruction(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let opcode = (instruction >> 26) & 0x3f;
        
        match opcode {
            0x00 => self.execute_special(instruction, bus)?,
            0x01 => self.execute_regimm(instruction)?,
            0x02 => self.j(instruction),       // J
            0x03 => self.jal(instruction),     // JAL
            0x04 => self.beq(instruction),     // BEQ
            0x05 => self.bne(instruction),     // BNE
            0x06 => self.blez(instruction),    // BLEZ
            0x07 => self.bgtz(instruction),    // BGTZ
            0x08 => self.addi(instruction),    // ADDI
            0x09 => self.addiu(instruction),   // ADDIU
            0x0a => self.slti(instruction),    // SLTI
            0x0b => self.sltiu(instruction),   // SLTIU
            0x0c => self.andi(instruction),    // ANDI
            0x0d => self.ori(instruction),     // ORI
            0x0e => self.xori(instruction),    // XORI
            0x0f => self.lui(instruction),     // LUI
            0x10 => self.execute_cop0(instruction, bus)?, // COP0
            0x20 => self.lb(instruction, bus)?,   // LB
            0x21 => self.lh(instruction, bus)?,   // LH
            0x23 => self.lw(instruction, bus)?,   // LW
            0x24 => self.lbu(instruction, bus)?,  // LBU
            0x25 => self.lhu(instruction, bus)?,  // LHU
            0x28 => self.sb(instruction, bus)?,   // SB
            0x29 => self.sh(instruction, bus)?,   // SH
            0x2b => self.sw(instruction, bus)?,   // SW
            _ => {
                // Unknown opcode - for now just ignore
                log!(warn, "Unknown opcode: 0x{:02x} at PC: 0x{:08x}", opcode, self.pc);
            }
        }
        
        Ok(())
    }
    
    fn execute_special(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let funct = instruction & 0x3f;
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let rd = ((instruction >> 11) & 0x1f) as usize;
        let shamt = ((instruction >> 6) & 0x1f) as u32;
        
        match funct {
            0x00 => self.regs[rd] = self.regs[rt] << shamt,           // SLL
            0x02 => self.regs[rd] = self.regs[rt] >> shamt,           // SRL
            0x03 => self.regs[rd] = ((self.regs[rt] as i32) >> shamt) as u32, // SRA
            0x04 => self.regs[rd] = self.regs[rt] << (self.regs[rs] & 0x1f),  // SLLV
            0x06 => self.regs[rd] = self.regs[rt] >> (self.regs[rs] & 0x1f),  // SRLV
            0x07 => self.regs[rd] = ((self.regs[rt] as i32) >> (self.regs[rs] & 0x1f)) as u32, // SRAV
            0x08 => self.jr(instruction),      // JR
            0x09 => self.jalr(instruction),    // JALR
            0x0c => self.syscall(bus)?,        // SYSCALL
            0x10 => self.regs[rd] = self.hi,   // MFHI
            0x11 => self.hi = self.regs[rs],   // MTHI
            0x12 => self.regs[rd] = self.lo,   // MFLO
            0x13 => self.lo = self.regs[rs],   // MTLO
            0x18 => self.mult(instruction),    // MULT
            0x19 => self.multu(instruction),   // MULTU
            0x1a => self.div(instruction),     // DIV
            0x1b => self.divu(instruction),    // DIVU
            0x20 => self.regs[rd] = self.regs[rs].wrapping_add(self.regs[rt]), // ADD
            0x21 => self.regs[rd] = self.regs[rs].wrapping_add(self.regs[rt]), // ADDU
            0x22 => self.regs[rd] = self.regs[rs].wrapping_sub(self.regs[rt]), // SUB
            0x23 => self.regs[rd] = self.regs[rs].wrapping_sub(self.regs[rt]), // SUBU
            0x24 => self.regs[rd] = self.regs[rs] & self.regs[rt],  // AND
            0x25 => self.regs[rd] = self.regs[rs] | self.regs[rt],  // OR
            0x26 => self.regs[rd] = self.regs[rs] ^ self.regs[rt],  // XOR
            0x27 => self.regs[rd] = !(self.regs[rs] | self.regs[rt]), // NOR
            0x2a => self.regs[rd] = if (self.regs[rs] as i32) < (self.regs[rt] as i32) { 1 } else { 0 }, // SLT
            0x2b => self.regs[rd] = if self.regs[rs] < self.regs[rt] { 1 } else { 0 }, // SLTU
            _ => {
                log!(warn, "Unknown special function: 0x{:02x}", funct);
            }
        }
        
        Ok(())
    }
    
    fn execute_regimm(&mut self, instruction: u32) -> Result<()> {
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as i16 as i32 as u32;
        
        match rt {
            0x00 => { // BLTZ
                if (self.regs[rs] as i32) < 0 {
                    self.branch(imm);
                }
            }
            0x01 => { // BGEZ
                if (self.regs[rs] as i32) >= 0 {
                    self.branch(imm);
                }
            }
            _ => {
                log!(warn, "Unknown REGIMM rt: 0x{:02x}", rt);
            }
        }
        
        Ok(())
    }
    
    fn execute_cop0(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let cop_op = (instruction >> 21) & 0x1f;
        
        match cop_op {
            0x00 => { // MFC0
                let rt = ((instruction >> 16) & 0x1f) as usize;
                let rd = ((instruction >> 11) & 0x1f) as usize;
                self.load_delay_slot = Some((rt as u8, self.cop0_regs[rd]));
            }
            0x04 => { // MTC0
                let rt = ((instruction >> 16) & 0x1f) as usize;
                let rd = ((instruction >> 11) & 0x1f) as usize;
                self.cop0_regs[rd] = self.regs[rt];
            }
            0x10 => { // RFE
                let mode = self.cop0_regs[12] & 0x3f;
                self.cop0_regs[12] = (self.cop0_regs[12] & !0x3f) | ((mode >> 2) & 0xf);
            }
            _ => {
                log!(warn, "Unknown COP0 operation: 0x{:02x}", cop_op);
            }
        }
        
        Ok(())
    }
    
    // Branch and jump instructions
    fn branch(&mut self, offset: u32) {
        self.next_pc = self.pc.wrapping_add(offset << 2);
        self.branch_delay = true;
    }
    
    fn j(&mut self, instruction: u32) {
        let target = (instruction & 0x3ffffff) << 2;
        self.next_pc = (self.pc & 0xf0000000) | target;
    }
    
    fn jal(&mut self, instruction: u32) {
        self.regs[31] = self.next_pc; // Return address in $ra
        self.j(instruction);
    }
    
    fn jr(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        self.next_pc = self.regs[rs];
    }
    
    fn jalr(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rd = ((instruction >> 11) & 0x1f) as usize;
        self.regs[rd] = self.next_pc;
        self.next_pc = self.regs[rs];
    }
    
    fn beq(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as i16 as i32 as u32;
        
        if self.regs[rs] == self.regs[rt] {
            self.branch(imm);
        }
    }
    
    fn bne(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as i16 as i32 as u32;
        
        if self.regs[rs] != self.regs[rt] {
            self.branch(imm);
        }
    }
    
    fn blez(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as i16 as i32 as u32;
        
        if (self.regs[rs] as i32) <= 0 {
            self.branch(imm);
        }
    }
    
    fn bgtz(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as i16 as i32 as u32;
        
        if (self.regs[rs] as i32) > 0 {
            self.branch(imm);
        }
    }
    
    // Arithmetic instructions
    fn addi(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as i16 as i32 as u32;
        
        self.regs[rt] = self.regs[rs].wrapping_add(imm);
    }
    
    fn addiu(&mut self, instruction: u32) {
        self.addi(instruction); // Same as ADDI for our purposes
    }
    
    fn slti(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as i16 as i32;
        
        self.regs[rt] = if (self.regs[rs] as i32) < imm { 1 } else { 0 };
    }
    
    fn sltiu(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as i16 as i32 as u32;
        
        self.regs[rt] = if self.regs[rs] < imm { 1 } else { 0 };
    }
    
    fn andi(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as u32;
        
        self.regs[rt] = self.regs[rs] & imm;
    }
    
    fn ori(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as u32;
        
        self.regs[rt] = self.regs[rs] | imm;
    }
    
    fn xori(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as u32;
        
        self.regs[rt] = self.regs[rs] ^ imm;
    }
    
    fn lui(&mut self, instruction: u32) {
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let imm = (instruction & 0xffff) as u32;
        
        self.regs[rt] = imm << 16;
    }
    
    fn mult(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        
        let result = (self.regs[rs] as i32 as i64) * (self.regs[rt] as i32 as i64);
        self.lo = result as u32;
        self.hi = (result >> 32) as u32;
    }
    
    fn multu(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        
        let result = (self.regs[rs] as u64) * (self.regs[rt] as u64);
        self.lo = result as u32;
        self.hi = (result >> 32) as u32;
    }
    
    fn div(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        
        let dividend = self.regs[rs] as i32;
        let divisor = self.regs[rt] as i32;
        
        if divisor != 0 {
            self.lo = (dividend / divisor) as u32;
            self.hi = (dividend % divisor) as u32;
        } else {
            // Division by zero
            self.lo = if dividend >= 0 { 0xffffffff } else { 1 };
            self.hi = dividend as u32;
        }
    }
    
    fn divu(&mut self, instruction: u32) {
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        
        let dividend = self.regs[rs];
        let divisor = self.regs[rt];
        
        if divisor != 0 {
            self.lo = dividend / divisor;
            self.hi = dividend % divisor;
        } else {
            // Division by zero
            self.lo = 0xffffffff;
            self.hi = dividend;
        }
    }
    
    // Load/Store instructions
    fn lb(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let base = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let offset = (instruction & 0xffff) as i16 as i32 as u32;
        
        let addr = self.regs[base].wrapping_add(offset);
        let val = bus.load8(addr)? as i8 as i32 as u32;
        self.load_delay_slot = Some((rt as u8, val));
        
        Ok(())
    }
    
    fn lh(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let base = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let offset = (instruction & 0xffff) as i16 as i32 as u32;
        
        let addr = self.regs[base].wrapping_add(offset);
        let val = bus.load16(addr)? as i16 as i32 as u32;
        self.load_delay_slot = Some((rt as u8, val));
        
        Ok(())
    }
    
    fn lw(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let base = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let offset = (instruction & 0xffff) as i16 as i32 as u32;
        
        let addr = self.regs[base].wrapping_add(offset);
        let val = bus.load32(addr)?;
        self.load_delay_slot = Some((rt as u8, val));
        
        Ok(())
    }
    
    fn lbu(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let base = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let offset = (instruction & 0xffff) as i16 as i32 as u32;
        
        let addr = self.regs[base].wrapping_add(offset);
        let val = bus.load8(addr)? as u32;
        self.load_delay_slot = Some((rt as u8, val));
        
        Ok(())
    }
    
    fn lhu(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let base = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let offset = (instruction & 0xffff) as i16 as i32 as u32;
        
        let addr = self.regs[base].wrapping_add(offset);
        let val = bus.load16(addr)? as u32;
        self.load_delay_slot = Some((rt as u8, val));
        
        Ok(())
    }
    
    fn sb(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let base = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let offset = (instruction & 0xffff) as i16 as i32 as u32;
        
        let addr = self.regs[base].wrapping_add(offset);
        bus.store8(addr, self.regs[rt] as u8)?;
        
        Ok(())
    }
    
    fn sh(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let base = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let offset = (instruction & 0xffff) as i16 as i32 as u32;
        
        let addr = self.regs[base].wrapping_add(offset);
        bus.store16(addr, self.regs[rt] as u16)?;
        
        Ok(())
    }
    
    fn sw(&mut self, instruction: u32, bus: &mut Bus) -> Result<()> {
        let base = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let offset = (instruction & 0xffff) as i16 as i32 as u32;
        
        let addr = self.regs[base].wrapping_add(offset);
        bus.store32(addr, self.regs[rt])?;
        
        Ok(())
    }
    
    fn syscall(&mut self, bus: &mut Bus) -> Result<()> {
        // BIOS HLE - handle common BIOS calls
        let function = self.regs[9]; // T1 register typically contains function number
        
        match self.pc {
            0xa0 => {
                // A-Function BIOS calls
                match function {
                    0x00 => { /* FileOpen */ }
                    0x01 => { /* FileSeek */ }
                    0x02 => { /* FileRead */ }
                    0x03 => { /* FileWrite */ }
                    0x04 => { /* FileClose */ }
                    0x3c => { /* putchar */
                        let c = self.regs[4] as u8; // A0 register
                        log!(info, "BIOS putchar: {}", c as char);
                    }
                    0x3e => { /* puts */
                        let mut addr = self.regs[4]; // A0 register
                        let mut s = String::new();
                        loop {
                            let c = bus.load8(addr)?;
                            if c == 0 { break; }
                            s.push(c as char);
                            addr += 1;
                        }
                        log!(info, "BIOS puts: {}", s);
                    }
                    0x40 => { /* SystemErrorExit */
                        log!(error, "BIOS SystemErrorExit called");
                    }
                    _ => {
                        log!(warn, "Unknown A-Function: 0x{:02x}", function);
                    }
                }
            }
            0xb0 => {
                // B-Function BIOS calls
                match function {
                    0x00 => { /* alloc_kernel_memory */ }
                    0x3d => { /* std_out_putchar */
                        let c = self.regs[4] as u8; // A0 register
                        log!(info, "BIOS std_out_putchar: {}", c as char);
                    }
                    0x3f => { /* std_out_puts */
                        let mut addr = self.regs[4]; // A0 register
                        let mut s = String::new();
                        loop {
                            let c = bus.load8(addr)?;
                            if c == 0 { break; }
                            s.push(c as char);
                            addr += 1;
                        }
                        log!(info, "BIOS std_out_puts: {}", s);
                    }
                    _ => {
                        log!(warn, "Unknown B-Function: 0x{:02x}", function);
                    }
                }
            }
            0xc0 => {
                // C-Function BIOS calls
                match function {
                    0x00 => { /* EnterCriticalSection */ }
                    0x01 => { /* ExitCriticalSection */ }
                    _ => {
                        log!(warn, "Unknown C-Function: 0x{:02x}", function);
                    }
                }
            }
            _ => {
                // Regular exception
                self.exception(0x08)?; // Syscall exception
            }
        }
        
        Ok(())
    }
    
    fn exception(&mut self, code: u32) -> Result<()> {
        // Save current state
        let mode = self.cop0_regs[12] & 0x3f;
        self.cop0_regs[12] = (self.cop0_regs[12] & !0x3f) | ((mode << 2) & 0x3f);
        
        // Set exception code
        self.cop0_regs[13] = (self.cop0_regs[13] & !0x7c) | ((code << 2) & 0x7c);
        
        // Set EPC (return address)
        self.cop0_regs[14] = self.pc;
        
        // Jump to exception handler
        if self.cop0_regs[12] & 0x400000 != 0 {
            // BEV = 1, use boot exception vectors
            self.pc = 0xbfc00180;
        } else {
            // BEV = 0, use normal exception vectors
            self.pc = 0x80000080;
        }
        
        self.next_pc = self.pc + 4;
        
        Ok(())
    }
}

// GPU implementation with command processing
pub struct Gpu {
    pub vram: Vec<u16>,
    pub display_width: u32,
    pub display_height: u32,
    pub display_x: u32,
    pub display_y: u32,
    pub draw_x_offset: i16,
    pub draw_y_offset: i16,
    pub texture_window_mask_x: u8,
    pub texture_window_mask_y: u8,
    pub texture_window_offset_x: u8,
    pub texture_window_offset_y: u8,
    pub dither: bool,
    pub draw_to_display: bool,
    pub texture_disable: bool,
    pub texture_page_x: u8,
    pub texture_page_y: u8,
    pub texture_depth: TextureDepth,
    pub transparency_mode: TransparencyMode,
    pub rectangle_texture_flip_x: bool,
    pub rectangle_texture_flip_y: bool,
    pub display_off: bool,
    pub command_buffer: Vec<u32>,
    pub command_remaining: usize,
    pub current_command: Option<GpuCommand>,
    pub status: u32,
    pub read_buffer: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum TextureDepth {
    T4Bit,
    T8Bit,
    T15Bit,
}

#[derive(Clone, Copy, Debug)]
pub enum TransparencyMode {
    Half,
    Add,
    Subtract,
    AddQuarter,
}

#[derive(Clone, Copy, Debug)]
pub enum GpuCommand {
    DrawMode,
    TextureWindow,
    SetDrawingAreaTopLeft,
    SetDrawingAreaBottomRight,
    SetDrawingOffset,
    MaskBitSetting,
    ClearCache,
    FillRectangle,
    CopyRectangle,
    CopyVramToVram,
    DrawPolygon { vertices: u8, shaded: bool, textured: bool, semi_transparent: bool },
    DrawLine { vertices: u8, shaded: bool, semi_transparent: bool },
    DrawRectangle { size: RectSize, textured: bool, semi_transparent: bool },
    Unknown(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RectSize {
    Variable,
    One,
    Eight,
    Sixteen,
}

impl Gpu {
    pub fn new() -> Self {
        Gpu {
            vram: vec![0; 1024 * 512],
            display_width: 640,
            display_height: 480,
            display_x: 0,
            display_y: 0,
            draw_x_offset: 0,
            draw_y_offset: 0,
            texture_window_mask_x: 0,
            texture_window_mask_y: 0,
            texture_window_offset_x: 0,
            texture_window_offset_y: 0,
            dither: false,
            draw_to_display: false,
            texture_disable: false,
            texture_page_x: 0,
            texture_page_y: 0,
            texture_depth: TextureDepth::T15Bit,
            transparency_mode: TransparencyMode::Half,
            rectangle_texture_flip_x: false,
            rectangle_texture_flip_y: false,
            display_off: false,
            command_buffer: Vec::new(),
            command_remaining: 0,
            current_command: None,
            status: 0x14802000, // Ready to receive commands
            read_buffer: 0,
        }
    }
    
    pub fn gp0_write(&mut self, val: u32) {
        if self.command_remaining > 0 {
            self.command_buffer.push(val);
            self.command_remaining -= 1;
            
            if self.command_remaining == 0 {
                self.execute_gp0_command();
            }
        } else {
            // New command
            let cmd = (val >> 24) & 0xff;
            self.command_buffer.clear();
            self.command_buffer.push(val);
            
            // Decode command and determine parameter count
            let (command, params) = self.decode_gp0_command(cmd as u8);
            self.current_command = Some(command);
            self.command_remaining = params;
            
            if self.command_remaining == 0 {
                self.execute_gp0_command();
            }
        }
    }
    
    fn decode_gp0_command(&self, cmd: u8) -> (GpuCommand, usize) {
        match cmd {
            0x00 => (GpuCommand::ClearCache, 0),
            0x01 => (GpuCommand::ClearCache, 0),
            0x02 => (GpuCommand::FillRectangle, 2),
            0x20..=0x3f => {
                let vertices = if cmd & 0x10 != 0 { 4 } else { 3 };
                let shaded = cmd & 0x10 != 0;
                let textured = cmd & 0x04 != 0;
                let semi_transparent = cmd & 0x02 != 0;
                
                let params = vertices + if shaded { vertices - 1 } else { 0 } + if textured { vertices } else { 0 };
                
                (GpuCommand::DrawPolygon { vertices, shaded, textured, semi_transparent }, params as usize - 1)
            }
            0x40..=0x5f => {
                let vertices = if cmd & 0x08 != 0 { 2 } else { 2 }; // Line strips not implemented
                let shaded = cmd & 0x10 != 0;
                let semi_transparent = cmd & 0x02 != 0;
                
                let params = vertices + if shaded { vertices - 1 } else { 0 };
                
                (GpuCommand::DrawLine { vertices, shaded, semi_transparent }, params as usize - 1)
            }
            0x60..=0x7f => {
                let size = match (cmd >> 3) & 0x3 {
                    0 => RectSize::Variable,
                    1 => RectSize::One,
                    2 => RectSize::Eight,
                    3 => RectSize::Sixteen,
                    _ => unreachable!(),
                };
                let textured = cmd & 0x04 != 0;
                let semi_transparent = cmd & 0x02 != 0;
                
                let params = if size == RectSize::Variable { 2 } else { 1 } + if textured { 1 } else { 0 };
                
                (GpuCommand::DrawRectangle { size, textured, semi_transparent }, params as usize - 1)
            }
            0x80..=0x9f => (GpuCommand::CopyVramToVram, 3),
            0xa0..=0xbf => (GpuCommand::CopyRectangle, 3),
            0xe1 => (GpuCommand::DrawMode, 0),
            0xe2 => (GpuCommand::TextureWindow, 0),
            0xe3 => (GpuCommand::SetDrawingAreaTopLeft, 0),
            0xe4 => (GpuCommand::SetDrawingAreaBottomRight, 0),
            0xe5 => (GpuCommand::SetDrawingOffset, 0),
            0xe6 => (GpuCommand::MaskBitSetting, 0),
            _ => (GpuCommand::Unknown(cmd), 0),
        }
    }
    
    fn execute_gp0_command(&mut self) {
        if let Some(command) = self.current_command {
            match command {
                GpuCommand::ClearCache => {
                    // Invalidate texture cache
                }
                GpuCommand::FillRectangle => {
                    let color = self.command_buffer[0] & 0xffffff;
                    let xy = self.command_buffer[1];
                    let wh = self.command_buffer[2];
                    
                    let x = xy & 0x3ff;
                    let y = (xy >> 16) & 0x1ff;
                    let w = wh & 0x3ff;
                    let h = (wh >> 16) & 0x1ff;
                    
                    self.fill_rect(x, y, w, h, color);
                }
                GpuCommand::DrawPolygon { vertices, shaded, textured, semi_transparent } => {
                    // Simple triangle/quad rendering
                    self.draw_polygon(&self.command_buffer.clone(), vertices, shaded, textured, semi_transparent);
                }
                GpuCommand::DrawRectangle { size, textured, semi_transparent } => {
                    self.draw_rectangle(&self.command_buffer.clone(), size, textured, semi_transparent);
                }
                GpuCommand::DrawMode => {
                    let val = self.command_buffer[0];
                    self.texture_page_x = (val & 0xf) as u8;
                    self.texture_page_y = ((val >> 4) & 0x1) as u8;
                    self.transparency_mode = match (val >> 5) & 0x3 {
                        0 => TransparencyMode::Half,
                        1 => TransparencyMode::Add,
                        2 => TransparencyMode::Subtract,
                        3 => TransparencyMode::AddQuarter,
                        _ => unreachable!(),
                    };
                    self.texture_depth = match (val >> 7) & 0x3 {
                        0 => TextureDepth::T4Bit,
                        1 => TextureDepth::T8Bit,
                        2 | 3 => TextureDepth::T15Bit,
                        _ => unreachable!(),
                    };
                    self.dither = (val >> 9) & 0x1 != 0;
                    self.draw_to_display = (val >> 10) & 0x1 != 0;
                    self.texture_disable = (val >> 11) & 0x1 != 0;
                    self.rectangle_texture_flip_x = (val >> 12) & 0x1 != 0;
                    self.rectangle_texture_flip_y = (val >> 13) & 0x1 != 0;
                }
                GpuCommand::TextureWindow => {
                    let val = self.command_buffer[0];
                    self.texture_window_mask_x = (val & 0x1f) as u8;
                    self.texture_window_mask_y = ((val >> 5) & 0x1f) as u8;
                    self.texture_window_offset_x = ((val >> 10) & 0x1f) as u8;
                    self.texture_window_offset_y = ((val >> 15) & 0x1f) as u8;
                }
                GpuCommand::SetDrawingAreaTopLeft => {
                    let val = self.command_buffer[0];
                    let x = val & 0x3ff;
                    let y = (val >> 10) & 0x3ff;
                    // Set drawing area top-left
                }
                GpuCommand::SetDrawingAreaBottomRight => {
                    let val = self.command_buffer[0];
                    let x = val & 0x3ff;
                    let y = (val >> 10) & 0x3ff;
                    // Set drawing area bottom-right
                }
                GpuCommand::SetDrawingOffset => {
                    let val = self.command_buffer[0];
                    self.draw_x_offset = ((val & 0x7ff) as i16) << 5 >> 5; // Sign extend
                    self.draw_y_offset = (((val >> 11) & 0x7ff) as i16) << 5 >> 5;
                }
                GpuCommand::MaskBitSetting => {
                    let val = self.command_buffer[0];
                    // Set mask bit settings
                }
                _ => {
                    log!(warn, "Unimplemented GPU command: {:?}", command);
                }
            }
        }
        
        self.current_command = None;
    }
    
    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        let r = ((color >> 0) & 0xff) as u16;
        let g = ((color >> 8) & 0xff) as u16;
        let b = ((color >> 16) & 0xff) as u16;
        
        let color16 = ((b >> 3) << 10) | ((g >> 3) << 5) | (r >> 3);
        
        for dy in 0..h {
            for dx in 0..w {
                let vram_x = (x + dx) & 0x3ff;
                let vram_y = (y + dy) & 0x1ff;
                let idx = vram_y * 1024 + vram_x;
                if (idx as usize) < self.vram.len() {
                    self.vram[idx as usize] = color16;
                }
            }
        }
    }
    
    fn draw_polygon(&mut self, buffer: &[u32], vertices: u8, shaded: bool, textured: bool, semi_transparent: bool) {
        // Simplified polygon rendering - just draw as filled triangles
        // In a real implementation, this would use proper rasterization
        
        if vertices < 3 { return; }
        
        // Extract first vertex color
        let color = buffer[0] & 0xffffff;
        
        // For now, just fill a small area at the first vertex position
        let xy = buffer[1];
        let x = xy & 0x7ff;
        let y = (xy >> 16) & 0x7ff;
        
        self.fill_rect(x, y, 10, 10, color);
    }
    
    fn draw_rectangle(&mut self, buffer: &[u32], size: RectSize, textured: bool, semi_transparent: bool) {
        let color = buffer[0] & 0xffffff;
        let xy = buffer[1];
        let x = xy & 0x7ff;
        let y = (xy >> 16) & 0x7ff;
        
        let (w, h) = match size {
            RectSize::Variable => {
                let wh = buffer[2 + if textured { 1 } else { 0 }];
                ((wh & 0x7ff), ((wh >> 16) & 0x7ff))
            }
            RectSize::One => (1, 1),
            RectSize::Eight => (8, 8),
            RectSize::Sixteen => (16, 16),
        };
        
        self.fill_rect(x, y, w, h, color);
    }
    
    pub fn gp1_write(&mut self, val: u32) {
        let cmd = (val >> 24) & 0xff;
        
        match cmd {
            0x00 => {
                // Reset GPU
                self.reset();
            }
            0x01 => {
                // Reset command buffer
                self.command_buffer.clear();
                self.command_remaining = 0;
                self.current_command = None;
            }
            0x02 => {
                // Acknowledge GPU interrupt
            }
            0x03 => {
                // Display enable
                self.display_off = val & 0x1 != 0;
            }
            0x04 => {
                // DMA direction
                // 0 = Off, 1 = FIFO, 2 = CPUtoGP0, 3 = GPUREADtoCPU
            }
            0x05 => {
                // Start of display area
                self.display_x = val & 0x3ff;
                self.display_y = (val >> 10) & 0x1ff;
            }
            0x06 => {
                // Horizontal display range
                let x1 = val & 0xfff;
                let x2 = (val >> 12) & 0xfff;
            }
            0x07 => {
                // Vertical display range
                let y1 = val & 0x3ff;
                let y2 = (val >> 10) & 0x3ff;
            }
            0x08 => {
                // Display mode
                let hres = match val & 0x3 {
                    0 => 256,
                    1 => 320,
                    2 => 512,
                    3 => 640,
                    _ => unreachable!(),
                };
                let vres = if val & 0x4 != 0 { 480 } else { 240 };
                let interlaced = val & 0x20 != 0;
                
                self.display_width = hres;
                self.display_height = vres;
            }
            0x10..=0x1f => {
                // Get GPU info
                match cmd & 0xf {
                    2 => self.read_buffer = self.texture_window_mask_x as u32 |
                                            ((self.texture_window_mask_y as u32) << 5) |
                                            ((self.texture_window_offset_x as u32) << 10) |
                                            ((self.texture_window_offset_y as u32) << 15),
                    3 => self.read_buffer = 0, // Drawing area top left
                    4 => self.read_buffer = 0, // Drawing area bottom right
                    5 => self.read_buffer = ((self.draw_x_offset as u32) & 0x7ff) |
                                            (((self.draw_y_offset as u32) & 0x7ff) << 11),
                    7 => self.read_buffer = 2, // GPU version
                    _ => self.read_buffer = 0,
                }
            }
            _ => {
                log!(warn, "Unknown GP1 command: 0x{:02x}", cmd);
            }
        }
    }
    
    pub fn reset(&mut self) {
        self.command_buffer.clear();
        self.command_remaining = 0;
        self.current_command = None;
        self.status = 0x14802000;
        self.display_x = 0;
        self.display_y = 0;
        self.draw_x_offset = 0;
        self.draw_y_offset = 0;
        self.texture_window_mask_x = 0;
        self.texture_window_mask_y = 0;
        self.texture_window_offset_x = 0;
        self.texture_window_offset_y = 0;
    }
    
    pub fn get_status(&self) -> u32 {
        self.status
    }
    
    pub fn get_read(&self) -> u32 {
        self.read_buffer
    }
    
    pub fn get_framebuffer_rgb(&self, buffer: &mut Vec<u8>) {
        let width = self.display_width as usize;
        let height = self.display_height as usize;
        
        buffer.resize(width * height * 4, 0);
        
        let start_x = self.display_x as usize;
        let start_y = self.display_y as usize;
        
        for y in 0..height {
            for x in 0..width {
                let vram_x = (start_x + x) & 0x3ff;
                let vram_y = (start_y + y) & 0x1ff;
                let vram_idx = vram_y * 1024 + vram_x;
                
                let pixel = if vram_idx < self.vram.len() {
                    self.vram[vram_idx]
                } else {
                    0
                };
                
                // Convert 15-bit RGB to 24-bit RGB
                let r = ((pixel & 0x1f) << 3) as u8;
                let g = (((pixel >> 5) & 0x1f) << 3) as u8;
                let b = (((pixel >> 10) & 0x1f) << 3) as u8;
                
                let buffer_idx = (y * width + x) * 4;
                buffer[buffer_idx] = r;
                buffer[buffer_idx + 1] = g;
                buffer[buffer_idx + 2] = b;
                buffer[buffer_idx + 3] = 255;
            }
        }
    }
    
}

// SPU (Sound Processing Unit) stub
pub struct Spu {
    audio_buffer: Vec<f32>,
    control: u16,
    status: u16,
    volume_left: u16,
    volume_right: u16,
}

impl Spu {
    pub fn new() -> Self {
        Spu {
            audio_buffer: Vec::new(),
            control: 0,
            status: 0,
            volume_left: 0,
            volume_right: 0,
        }
    }
}

// DMA Controller
pub struct Dma {
    channels: [DmaChannel; 7],
    control: u32,
    interrupt: u32,
}

impl Dma {
    pub fn new() -> Self {
        Dma {
            channels: [DmaChannel::new(); 7],
            control: 0x07654321,
            interrupt: 0,
        }
    }
    
    pub fn channel_mut(&mut self, channel: usize) -> &mut DmaChannel {
        &mut self.channels[channel]
    }
}

#[derive(Clone, Copy)]
pub struct DmaChannel {
    base_addr: u32,
    block_control: u32,
    control: u32,
}

impl DmaChannel {
    fn new() -> Self {
        DmaChannel {
            base_addr: 0,
            block_control: 0,
            control: 0,
        }
    }
}

// Timer implementation
pub struct Timers {
    timers: [Timer; 3],
}

impl Timers {
    pub fn new() -> Self {
        Timers {
            timers: [Timer::new(); 3],
        }
    }
    
    pub fn timer_mut(&mut self, index: usize) -> &mut Timer {
        &mut self.timers[index]
    }
}

#[derive(Clone, Copy)]
pub struct Timer {
    counter: u16,
    target: u16,
    mode: u16,
}

impl Timer {
    fn new() -> Self {
        Timer {
            counter: 0,
            target: 0,
            mode: 0,
        }
    }
}

// IRQ Controller
pub struct Irq {
    status: u32,
    mask: u32,
}

impl Irq {
    pub fn new() -> Self {
        Irq {
            status: 0,
            mask: 0,
        }
    }
    
    pub fn request(&mut self, irq: u32) {
        self.status |= 1 << irq;
    }
    
    pub fn acknowledge(&mut self, irq: u32) {
        self.status &= !(1 << irq);
    }
    
    pub fn pending(&self) -> bool {
        (self.status & self.mask) != 0
    }
}

// Pad/Memory Card controller
pub struct PadMemCard {
    controller_state: [u16; 2],
    tx_data: u8,
    rx_data: u8,
    status: u32,
    mode: u16,
    control: u16,
    baud: u16,
}

impl PadMemCard {
    pub fn new() -> Self {
        PadMemCard {
            controller_state: [0xffff; 2],
            tx_data: 0,
            rx_data: 0xff,
            status: 0x00000005,
            mode: 0,
            control: 0,
            baud: 0,
        }
    }
    
    pub fn set_controller_state(&mut self, controller: usize, state: u16) {
        if controller < 2 {
            self.controller_state[controller] = !state; // Active low
        }
    }
}

// Memory bus
pub struct Bus {
    ram: Vec<u8>,
    bios: Vec<u8>,
    scratchpad: Vec<u8>,
    io_ports: HashMap<u32, u32>,
}

impl Bus {
    pub fn new() -> Self {
        Bus {
            ram: vec![0; 2 * 1024 * 1024],      // 2MB main RAM
            bios: vec![0; 512 * 1024],          // 512KB BIOS
            scratchpad: vec![0; 1024],          // 1KB scratchpad
            io_ports: HashMap::new(),
        }
    }
    
    pub fn load8(&self, addr: u32) -> Result<u8> {
        let physical = addr & 0x1fffffff;
        
        match physical {
            0x00000000..=0x001fffff => {
                // Main RAM
                Ok(self.ram[(physical & 0x1fffff) as usize])
            }
            0x1f000000..=0x1f0003ff => {
                // Scratchpad
                Ok(self.scratchpad[(physical & 0x3ff) as usize])
            }
            0x1fc00000..=0x1fc7ffff => {
                // BIOS
                Ok(self.bios[(physical & 0x7ffff) as usize])
            }
            _ => {
                log!(warn, "Unhandled load8 from 0x{:08x}", addr);
                Ok(0)
            }
        }
    }
    
    pub fn load16(&self, addr: u32) -> Result<u16> {
        let b0 = self.load8(addr)? as u16;
        let b1 = self.load8(addr + 1)? as u16;
        Ok(b0 | (b1 << 8))
    }
    
    pub fn load32(&self, addr: u32) -> Result<u32> {
        let physical = addr & 0x1fffffff;
        
        match physical {
            0x00000000..=0x001fffff => {
                // Main RAM
                let offset = (physical & 0x1fffff) as usize;
                if offset + 3 < self.ram.len() {
                    Ok(u32::from_le_bytes([
                        self.ram[offset],
                        self.ram[offset + 1],
                        self.ram[offset + 2],
                        self.ram[offset + 3],
                    ]))
                } else {
                    Ok(0)
                }
            }
            0x1f000000..=0x1f0003ff => {
                // Scratchpad
                let offset = (physical & 0x3ff) as usize;
                if offset + 3 < self.scratchpad.len() {
                    Ok(u32::from_le_bytes([
                        self.scratchpad[offset],
                        self.scratchpad[offset + 1],
                        self.scratchpad[offset + 2],
                        self.scratchpad[offset + 3],
                    ]))
                } else {
                    Ok(0)
                }
            }
            0x1f801000..=0x1f802fff => {
                // I/O ports
                Ok(*self.io_ports.get(&physical).unwrap_or(&0))
            }
            0x1fc00000..=0x1fc7ffff => {
                // BIOS
                let offset = (physical & 0x7ffff) as usize;
                if offset + 3 < self.bios.len() {
                    Ok(u32::from_le_bytes([
                        self.bios[offset],
                        self.bios[offset + 1],
                        self.bios[offset + 2],
                        self.bios[offset + 3],
                    ]))
                } else {
                    Ok(0)
                }
            }
            _ => {
                log!(warn, "Unhandled load32 from 0x{:08x}", addr);
                Ok(0)
            }
        }
    }
    
    pub fn store8(&mut self, addr: u32, val: u8) -> Result<()> {
        let physical = addr & 0x1fffffff;
        
        match physical {
            0x00000000..=0x001fffff => {
                // Main RAM
                self.ram[(physical & 0x1fffff) as usize] = val;
            }
            0x1f000000..=0x1f0003ff => {
                // Scratchpad
                self.scratchpad[(physical & 0x3ff) as usize] = val;
            }
            _ => {
                log!(warn, "Unhandled store8 to 0x{:08x} = 0x{:02x}", addr, val);
            }
        }
        
        Ok(())
    }
    
    pub fn store16(&mut self, addr: u32, val: u16) -> Result<()> {
        self.store8(addr, val as u8)?;
        self.store8(addr + 1, (val >> 8) as u8)?;
        Ok(())
    }
    
    pub fn store32(&mut self, addr: u32, val: u32) -> Result<()> {
        let physical = addr & 0x1fffffff;
        
        match physical {
            0x00000000..=0x001fffff => {
                // Main RAM
                let offset = (physical & 0x1fffff) as usize;
                if offset + 3 < self.ram.len() {
                    self.ram[offset] = val as u8;
                    self.ram[offset + 1] = (val >> 8) as u8;
                    self.ram[offset + 2] = (val >> 16) as u8;
                    self.ram[offset + 3] = (val >> 24) as u8;
                }
            }
            0x1f000000..=0x1f0003ff => {
                // Scratchpad
                let offset = (physical & 0x3ff) as usize;
                if offset + 3 < self.scratchpad.len() {
                    self.scratchpad[offset] = val as u8;
                    self.scratchpad[offset + 1] = (val >> 8) as u8;
                    self.scratchpad[offset + 2] = (val >> 16) as u8;
                    self.scratchpad[offset + 3] = (val >> 24) as u8;
                }
            }
            0x1f801000..=0x1f802fff => {
                // I/O ports
                self.io_ports.insert(physical, val);
            }
            _ => {
                log!(warn, "Unhandled store32 to 0x{:08x} = 0x{:08x}", addr, val);
            }
        }
        
        Ok(())
    }
}

// Main PSX structure
pub struct Psx {
    pub cpu: Cpu,
    pub gpu: Gpu,
    pub spu: Spu,
    pub dma: Dma,
    pub timers: Timers,
    pub irq: Irq,
    pub pad_memcard: PadMemCard,
    pub bus: Bus,
    pub display_width: u32,
    pub display_height: u32,
    cycle_counter: u64,
}

impl Psx {
    pub fn new() -> Result<Self> {
        Ok(Psx {
            cpu: Cpu::new(),
            gpu: Gpu::new(),
            spu: Spu::new(),
            dma: Dma::new(),
            timers: Timers::new(),
            irq: Irq::new(),
            pad_memcard: PadMemCard::new(),
            bus: Bus::new(),
            display_width: 640,
            display_height: 480,
            cycle_counter: 0,
        })
    }
    
    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<()> {
        if bios_data.len() != 512 * 1024 {
            return Err(PsxError::InvalidBios);
        }
        self.bus.bios.copy_from_slice(bios_data);
        Ok(())
    }
    
    pub fn load_exe(&mut self, exe_data: &[u8]) -> Result<()> {
        // Parse PSX-EXE header
        if exe_data.len() < 0x800 {
            return Err(PsxError::InvalidExe);
        }
        
        // Check magic
        if &exe_data[0..8] != b"PS-X EXE" {
            return Err(PsxError::InvalidExe);
        }
        
        // Read header fields (little-endian)
        let pc = u32::from_le_bytes([exe_data[0x10], exe_data[0x11], exe_data[0x12], exe_data[0x13]]);
        let gp = u32::from_le_bytes([exe_data[0x14], exe_data[0x15], exe_data[0x16], exe_data[0x17]]);
        let dest = u32::from_le_bytes([exe_data[0x18], exe_data[0x19], exe_data[0x1a], exe_data[0x1b]]);
        let size = u32::from_le_bytes([exe_data[0x1c], exe_data[0x1d], exe_data[0x1e], exe_data[0x1f]]);
        let sp = u32::from_le_bytes([exe_data[0x30], exe_data[0x31], exe_data[0x32], exe_data[0x33]]);
        
        // Copy executable to RAM
        let dest_offset = (dest & 0x1fffff) as usize;
        let exe_size = size.min((exe_data.len() - 0x800) as u32) as usize;
        
        if dest_offset + exe_size <= self.bus.ram.len() {
            self.bus.ram[dest_offset..dest_offset + exe_size]
                .copy_from_slice(&exe_data[0x800..0x800 + exe_size]);
        }
        
        // Set CPU registers
        self.cpu.pc = pc;
        self.cpu.next_pc = pc + 4;
        self.cpu.regs[28] = gp; // GP register
        if sp != 0 {
            self.cpu.regs[29] = sp; // SP register
        } else {
            self.cpu.regs[29] = 0x801fff00; // Default stack
        }
        
        Ok(())
    }
    
    pub fn init_with_disc(&mut self) -> Result<()> {
        // Initialize PSX with a disc loaded
        // This would normally boot from the disc
        self.reset()
    }
    
    pub fn reset(&mut self) -> Result<()> {
        self.cpu = Cpu::new();
        self.gpu.reset();
        self.cycle_counter = 0;
        Ok(())
    }
    
    pub fn run_frame(&mut self) -> Result<()> {
        // Run approximately one frame worth of CPU cycles
        // PSX runs at ~33.8688 MHz, 60 FPS = ~564,480 cycles per frame
        let cycles_per_frame = 564480;
        let start_cycle = self.cycle_counter;
        
        while self.cycle_counter - start_cycle < cycles_per_frame {
            // Handle I/O port reads/writes for GPU
            if let Some(&val) = self.bus.io_ports.get(&0x1f801810) {
                // GP0 command
                self.gpu.gp0_write(val);
                self.bus.io_ports.remove(&0x1f801810);
            }
            
            if let Some(&val) = self.bus.io_ports.get(&0x1f801814) {
                // GP1 command
                self.gpu.gp1_write(val);
                self.bus.io_ports.remove(&0x1f801814);
            }
            
            // Update GPUSTAT register
            self.bus.io_ports.insert(0x1f801814, self.gpu.get_status());
            
            // Update GPUREAD register
            self.bus.io_ports.insert(0x1f801810, self.gpu.get_read());
            
            // Update pad/memcard status
            self.bus.io_ports.insert(0x1f801044, self.pad_memcard.status);
            self.bus.io_ports.insert(0x1f801040, self.pad_memcard.rx_data as u32);
            
            // Run CPU
            self.cpu.step(&mut self.bus)?;
            
            // Update timers
            // Timer updates would go here
            
            // Check for interrupts
            if self.irq.pending() {
                // Trigger CPU interrupt
            }
            
            self.cycle_counter += 1;
        }
        
        // Update display dimensions
        self.display_width = self.gpu.display_width;
        self.display_height = self.gpu.display_height;
        
        Ok(())
    }
    
    pub fn set_controller_state(&mut self, controller: usize, state: u16) {
        self.pad_memcard.set_controller_state(controller, state);
    }
    
    pub fn get_framebuffer(&self, buffer: &mut Vec<u8>) {
        self.gpu.get_framebuffer_rgb(buffer);
    }
    
    pub fn get_audio_samples(&self, buffer: &mut Vec<f32>) {
        // Generate silence for now
        buffer.clear();
    }
}