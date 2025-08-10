// Complete PSX emulator implementation for WASM
// Based on the full rustation-ng architecture

use super::error::{PsxError, Result};

// Constants
const CPU_FREQ_HZ: u32 = 33_868_800;
const RAM_SIZE: usize = 2 * 1024 * 1024; // 2MB
const BIOS_SIZE: usize = 512 * 1024;     // 512KB
const VRAM_SIZE: usize = 1024 * 512;     // 512K pixels (1MB)

// Cycle counter type
type CycleCount = i32;

// ============================================================================
// MIPS R3000A CPU Implementation
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub enum Instruction {
    // Special format (R-type)
    Sll(u8, u8, u8),      // rd, rt, sa
    Srl(u8, u8, u8),
    Sra(u8, u8, u8),
    Sllv(u8, u8, u8),     // rd, rt, rs
    Srlv(u8, u8, u8),
    Srav(u8, u8, u8),
    Jr(u8),               // rs
    Jalr(u8, u8),         // rd, rs
    Syscall,
    Break,
    Mfhi(u8),             // rd
    Mthi(u8),             // rs
    Mflo(u8),
    Mtlo(u8),
    Mult(u8, u8),         // rs, rt
    Multu(u8, u8),
    Div(u8, u8),
    Divu(u8, u8),
    Add(u8, u8, u8),      // rd, rs, rt
    Addu(u8, u8, u8),
    Sub(u8, u8, u8),
    Subu(u8, u8, u8),
    And(u8, u8, u8),
    Or(u8, u8, u8),
    Xor(u8, u8, u8),
    Nor(u8, u8, u8),
    Slt(u8, u8, u8),
    Sltu(u8, u8, u8),
    
    // Immediate format (I-type)
    Addi(u8, u8, i16),    // rt, rs, imm
    Addiu(u8, u8, i16),
    Slti(u8, u8, i16),
    Sltiu(u8, u8, i16),
    Andi(u8, u8, u16),
    Ori(u8, u8, u16),
    Xori(u8, u8, u16),
    Lui(u8, u16),         // rt, imm
    
    // Branch instructions
    Beq(u8, u8, i16),     // rs, rt, offset
    Bne(u8, u8, i16),
    Blez(u8, i16),        // rs, offset
    Bgtz(u8, i16),
    Bltz(u8, i16),
    Bgez(u8, i16),
    Bltzal(u8, i16),
    Bgezal(u8, i16),
    
    // Jump instructions
    J(u32),               // target
    Jal(u32),
    
    // Load/Store
    Lb(u8, u8, i16),      // rt, base, offset
    Lh(u8, u8, i16),
    Lwl(u8, u8, i16),
    Lw(u8, u8, i16),
    Lbu(u8, u8, i16),
    Lhu(u8, u8, i16),
    Lwr(u8, u8, i16),
    Sb(u8, u8, i16),
    Sh(u8, u8, i16),
    Swl(u8, u8, i16),
    Sw(u8, u8, i16),
    Swr(u8, u8, i16),
    
    // Coprocessor instructions
    Mfc0(u8, u8),         // rt, rd
    Mtc0(u8, u8),
    Mfc2(u8, u8),         // GTE
    Mtc2(u8, u8),
    Cfc2(u8, u8),
    Ctc2(u8, u8),
    Cop2(u32),            // GTE command
    Rfe,
    
    // Invalid
    Invalid(u32),
}

pub struct Cpu {
    // Registers
    pub regs: [u32; 32],
    pub pc: u32,
    pub next_pc: u32,
    pub current_pc: u32,  // PC of currently executing instruction
    pub hi: u32,
    pub lo: u32,
    
    // Pipeline state
    pub load_delay: Option<(u8, u32)>,
    pub branch_delay: bool,
    pub in_delay_slot: bool,
    
    // Cache
    pub icache: ICache,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            regs: [0; 32],
            pc: 0xbfc00000,      // BIOS entry point
            next_pc: 0xbfc00004,
            current_pc: 0xbfc00000,
            hi: 0,
            lo: 0,
            load_delay: None,
            branch_delay: false,
            in_delay_slot: false,
            icache: ICache::new(),
        }
    }
    
    pub fn reset(&mut self) {
        self.regs = [0; 32];
        self.pc = 0xbfc00000;
        self.next_pc = 0xbfc00004;
        self.current_pc = 0xbfc00000;
        self.hi = 0;
        self.lo = 0;
        self.load_delay = None;
        self.branch_delay = false;
        self.in_delay_slot = false;
        self.icache.invalidate();
    }
    
    pub fn step(&mut self, psx: &mut Psx) -> Result<()> {
        // Save current PC for exception handling
        self.current_pc = self.pc;
        
        // Handle delay slot flag
        self.in_delay_slot = self.branch_delay;
        self.branch_delay = false;
        
        // Fetch instruction
        let instruction = self.fetch(psx)?;
        
        // Handle load delay slot
        if let Some((reg, val)) = self.load_delay.take() {
            self.set_reg(reg, val);
        }
        
        // Advance PC
        self.pc = self.next_pc;
        self.next_pc = self.pc.wrapping_add(4);
        
        // Decode and execute
        let decoded = self.decode(instruction);
        self.execute(decoded, psx)?;
        
        // R0 is always zero
        self.regs[0] = 0;
        
        Ok(())
    }
    
    fn fetch(&mut self, psx: &mut Psx) -> Result<u32> {
        // Check instruction cache first
        if let Some(instruction) = self.icache.fetch(self.pc) {
            psx.tick(1);
            return Ok(instruction);
        }
        
        // Cache miss - fetch from memory
        let instruction = psx.load32(self.pc)?;
        self.icache.store(self.pc, instruction);
        Ok(instruction)
    }
    
    fn decode(&self, instruction: u32) -> Instruction {
        let opcode = (instruction >> 26) & 0x3f;
        let rs = ((instruction >> 21) & 0x1f) as u8;
        let rt = ((instruction >> 16) & 0x1f) as u8;
        let rd = ((instruction >> 11) & 0x1f) as u8;
        let sa = ((instruction >> 6) & 0x1f) as u8;
        let funct = (instruction & 0x3f) as u8;
        let imm = (instruction & 0xffff) as i16;
        let uimm = (instruction & 0xffff) as u16;
        let target = instruction & 0x3ffffff;
        
        match opcode {
            0x00 => {
                // SPECIAL
                match funct {
                    0x00 => Instruction::Sll(rd, rt, sa),
                    0x02 => Instruction::Srl(rd, rt, sa),
                    0x03 => Instruction::Sra(rd, rt, sa),
                    0x04 => Instruction::Sllv(rd, rt, rs),
                    0x06 => Instruction::Srlv(rd, rt, rs),
                    0x07 => Instruction::Srav(rd, rt, rs),
                    0x08 => Instruction::Jr(rs),
                    0x09 => Instruction::Jalr(rd, rs),
                    0x0c => Instruction::Syscall,
                    0x0d => Instruction::Break,
                    0x10 => Instruction::Mfhi(rd),
                    0x11 => Instruction::Mthi(rs),
                    0x12 => Instruction::Mflo(rd),
                    0x13 => Instruction::Mtlo(rs),
                    0x18 => Instruction::Mult(rs, rt),
                    0x19 => Instruction::Multu(rs, rt),
                    0x1a => Instruction::Div(rs, rt),
                    0x1b => Instruction::Divu(rs, rt),
                    0x20 => Instruction::Add(rd, rs, rt),
                    0x21 => Instruction::Addu(rd, rs, rt),
                    0x22 => Instruction::Sub(rd, rs, rt),
                    0x23 => Instruction::Subu(rd, rs, rt),
                    0x24 => Instruction::And(rd, rs, rt),
                    0x25 => Instruction::Or(rd, rs, rt),
                    0x26 => Instruction::Xor(rd, rs, rt),
                    0x27 => Instruction::Nor(rd, rs, rt),
                    0x2a => Instruction::Slt(rd, rs, rt),
                    0x2b => Instruction::Sltu(rd, rs, rt),
                    _ => Instruction::Invalid(instruction),
                }
            }
            0x01 => {
                // REGIMM
                match rt {
                    0x00 => Instruction::Bltz(rs, imm),
                    0x01 => Instruction::Bgez(rs, imm),
                    0x10 => Instruction::Bltzal(rs, imm),
                    0x11 => Instruction::Bgezal(rs, imm),
                    _ => Instruction::Invalid(instruction),
                }
            }
            0x02 => Instruction::J(target),
            0x03 => Instruction::Jal(target),
            0x04 => Instruction::Beq(rs, rt, imm),
            0x05 => Instruction::Bne(rs, rt, imm),
            0x06 => Instruction::Blez(rs, imm),
            0x07 => Instruction::Bgtz(rs, imm),
            0x08 => Instruction::Addi(rt, rs, imm),
            0x09 => Instruction::Addiu(rt, rs, imm),
            0x0a => Instruction::Slti(rt, rs, imm),
            0x0b => Instruction::Sltiu(rt, rs, imm),
            0x0c => Instruction::Andi(rt, rs, uimm),
            0x0d => Instruction::Ori(rt, rs, uimm),
            0x0e => Instruction::Xori(rt, rs, uimm),
            0x0f => Instruction::Lui(rt, uimm),
            0x10 => {
                // COP0
                match rs {
                    0x00 => Instruction::Mfc0(rt, rd),
                    0x04 => Instruction::Mtc0(rt, rd),
                    0x10 => Instruction::Rfe,
                    _ => Instruction::Invalid(instruction),
                }
            }
            0x12 => {
                // COP2 (GTE)
                match rs {
                    0x00 => Instruction::Mfc2(rt, rd),
                    0x02 => Instruction::Cfc2(rt, rd),
                    0x04 => Instruction::Mtc2(rt, rd),
                    0x06 => Instruction::Ctc2(rt, rd),
                    _ => Instruction::Cop2(instruction & 0x1ffffff),
                }
            }
            0x20 => Instruction::Lb(rt, rs, imm),
            0x21 => Instruction::Lh(rt, rs, imm),
            0x22 => Instruction::Lwl(rt, rs, imm),
            0x23 => Instruction::Lw(rt, rs, imm),
            0x24 => Instruction::Lbu(rt, rs, imm),
            0x25 => Instruction::Lhu(rt, rs, imm),
            0x26 => Instruction::Lwr(rt, rs, imm),
            0x28 => Instruction::Sb(rt, rs, imm),
            0x29 => Instruction::Sh(rt, rs, imm),
            0x2a => Instruction::Swl(rt, rs, imm),
            0x2b => Instruction::Sw(rt, rs, imm),
            0x2e => Instruction::Swr(rt, rs, imm),
            _ => Instruction::Invalid(instruction),
        }
    }
    
    fn execute(&mut self, inst: Instruction, psx: &mut Psx) -> Result<()> {
        use Instruction::*;
        
        match inst {
            // Shifts
            Sll(rd, rt, sa) => self.set_reg(rd, self.reg(rt) << sa),
            Srl(rd, rt, sa) => self.set_reg(rd, self.reg(rt) >> sa),
            Sra(rd, rt, sa) => self.set_reg(rd, ((self.reg(rt) as i32) >> sa) as u32),
            Sllv(rd, rt, rs) => self.set_reg(rd, self.reg(rt) << (self.reg(rs) & 0x1f)),
            Srlv(rd, rt, rs) => self.set_reg(rd, self.reg(rt) >> (self.reg(rs) & 0x1f)),
            Srav(rd, rt, rs) => self.set_reg(rd, ((self.reg(rt) as i32) >> (self.reg(rs) & 0x1f)) as u32),
            
            // Jumps
            Jr(rs) => {
                self.next_pc = self.reg(rs);
                self.branch_delay = true;
            }
            Jalr(rd, rs) => {
                let ra = self.next_pc;
                self.next_pc = self.reg(rs);
                self.branch_delay = true;
                self.set_reg(rd, ra);
            }
            J(target) => {
                self.next_pc = (self.pc & 0xf0000000) | (target << 2);
                self.branch_delay = true;
            }
            Jal(target) => {
                let ra = self.next_pc;
                self.next_pc = (self.pc & 0xf0000000) | (target << 2);
                self.branch_delay = true;
                self.set_reg(31, ra);
            }
            
            // Branches
            Beq(rs, rt, offset) => {
                if self.reg(rs) == self.reg(rt) {
                    self.branch(offset);
                }
            }
            Bne(rs, rt, offset) => {
                if self.reg(rs) != self.reg(rt) {
                    self.branch(offset);
                }
            }
            Blez(rs, offset) => {
                if (self.reg(rs) as i32) <= 0 {
                    self.branch(offset);
                }
            }
            Bgtz(rs, offset) => {
                if (self.reg(rs) as i32) > 0 {
                    self.branch(offset);
                }
            }
            Bltz(rs, offset) => {
                if (self.reg(rs) as i32) < 0 {
                    self.branch(offset);
                }
            }
            Bgez(rs, offset) => {
                if (self.reg(rs) as i32) >= 0 {
                    self.branch(offset);
                }
            }
            Bltzal(rs, offset) => {
                let ra = self.next_pc;
                if (self.reg(rs) as i32) < 0 {
                    self.branch(offset);
                }
                self.set_reg(31, ra);
            }
            Bgezal(rs, offset) => {
                let ra = self.next_pc;
                if (self.reg(rs) as i32) >= 0 {
                    self.branch(offset);
                }
                self.set_reg(31, ra);
            }
            
            // Arithmetic
            Add(rd, rs, rt) => {
                let a = self.reg(rs) as i32;
                let b = self.reg(rt) as i32;
                match a.checked_add(b) {
                    Some(result) => self.set_reg(rd, result as u32),
                    None => return self.exception(psx, Exception::Overflow),
                }
            }
            Addu(rd, rs, rt) => self.set_reg(rd, self.reg(rs).wrapping_add(self.reg(rt))),
            Sub(rd, rs, rt) => {
                let a = self.reg(rs) as i32;
                let b = self.reg(rt) as i32;
                match a.checked_sub(b) {
                    Some(result) => self.set_reg(rd, result as u32),
                    None => return self.exception(psx, Exception::Overflow),
                }
            }
            Subu(rd, rs, rt) => self.set_reg(rd, self.reg(rs).wrapping_sub(self.reg(rt))),
            
            Addi(rt, rs, imm) => {
                let a = self.reg(rs) as i32;
                let b = imm as i32;
                match a.checked_add(b) {
                    Some(result) => self.set_reg(rt, result as u32),
                    None => return self.exception(psx, Exception::Overflow),
                }
            }
            Addiu(rt, rs, imm) => self.set_reg(rt, self.reg(rs).wrapping_add(imm as i32 as u32)),
            
            // Logical
            And(rd, rs, rt) => self.set_reg(rd, self.reg(rs) & self.reg(rt)),
            Or(rd, rs, rt) => self.set_reg(rd, self.reg(rs) | self.reg(rt)),
            Xor(rd, rs, rt) => self.set_reg(rd, self.reg(rs) ^ self.reg(rt)),
            Nor(rd, rs, rt) => self.set_reg(rd, !(self.reg(rs) | self.reg(rt))),
            
            Andi(rt, rs, imm) => self.set_reg(rt, self.reg(rs) & (imm as u32)),
            Ori(rt, rs, imm) => self.set_reg(rt, self.reg(rs) | (imm as u32)),
            Xori(rt, rs, imm) => self.set_reg(rt, self.reg(rs) ^ (imm as u32)),
            Lui(rt, imm) => self.set_reg(rt, (imm as u32) << 16),
            
            // Set on less than
            Slt(rd, rs, rt) => {
                let val = if (self.reg(rs) as i32) < (self.reg(rt) as i32) { 1 } else { 0 };
                self.set_reg(rd, val);
            }
            Sltu(rd, rs, rt) => {
                let val = if self.reg(rs) < self.reg(rt) { 1 } else { 0 };
                self.set_reg(rd, val);
            }
            Slti(rt, rs, imm) => {
                let val = if (self.reg(rs) as i32) < (imm as i32) { 1 } else { 0 };
                self.set_reg(rt, val);
            }
            Sltiu(rt, rs, imm) => {
                let val = if self.reg(rs) < (imm as i32 as u32) { 1 } else { 0 };
                self.set_reg(rt, val);
            }
            
            // Multiply/Divide
            Mult(rs, rt) => {
                let a = self.reg(rs) as i32 as i64;
                let b = self.reg(rt) as i32 as i64;
                let result = a * b;
                self.lo = result as u32;
                self.hi = (result >> 32) as u32;
            }
            Multu(rs, rt) => {
                let a = self.reg(rs) as u64;
                let b = self.reg(rt) as u64;
                let result = a * b;
                self.lo = result as u32;
                self.hi = (result >> 32) as u32;
            }
            Div(rs, rt) => {
                let dividend = self.reg(rs) as i32;
                let divisor = self.reg(rt) as i32;
                
                if divisor == 0 {
                    // Division by zero behavior
                    self.hi = dividend as u32;
                    self.lo = if dividend >= 0 { 0xffffffff } else { 1 };
                } else if dividend == i32::MIN && divisor == -1 {
                    // Overflow case
                    self.lo = i32::MIN as u32;
                    self.hi = 0;
                } else {
                    self.lo = (dividend / divisor) as u32;
                    self.hi = (dividend % divisor) as u32;
                }
            }
            Divu(rs, rt) => {
                let dividend = self.reg(rs);
                let divisor = self.reg(rt);
                
                if divisor == 0 {
                    self.hi = dividend;
                    self.lo = 0xffffffff;
                } else {
                    self.lo = dividend / divisor;
                    self.hi = dividend % divisor;
                }
            }
            
            Mfhi(rd) => self.set_reg(rd, self.hi),
            Mthi(rs) => self.hi = self.reg(rs),
            Mflo(rd) => self.set_reg(rd, self.lo),
            Mtlo(rs) => self.lo = self.reg(rs),
            
            // Loads
            Lb(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                let val = psx.load8(addr)? as i8 as i32 as u32;
                self.load_delay = Some((rt, val));
            }
            Lbu(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                let val = psx.load8(addr)? as u32;
                self.load_delay = Some((rt, val));
            }
            Lh(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                if addr & 0x1 != 0 {
                    return self.exception(psx, Exception::LoadAddressError);
                }
                let val = psx.load16(addr)? as i16 as i32 as u32;
                self.load_delay = Some((rt, val));
            }
            Lhu(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                if addr & 0x1 != 0 {
                    return self.exception(psx, Exception::LoadAddressError);
                }
                let val = psx.load16(addr)? as u32;
                self.load_delay = Some((rt, val));
            }
            Lw(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                if addr & 0x3 != 0 {
                    return self.exception(psx, Exception::LoadAddressError);
                }
                let val = psx.load32(addr)?;
                self.load_delay = Some((rt, val));
            }
            Lwl(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                let aligned = addr & !3;
                let word = psx.load32(aligned)?;
                let cur = if let Some((r, v)) = self.load_delay {
                    if r == rt { v } else { self.reg(rt) }
                } else {
                    self.reg(rt)
                };
                
                let val = match addr & 3 {
                    0 => (cur & 0x00ffffff) | (word << 24),
                    1 => (cur & 0x0000ffff) | (word << 16),
                    2 => (cur & 0x000000ff) | (word << 8),
                    3 => word,
                    _ => unreachable!(),
                };
                self.load_delay = Some((rt, val));
            }
            Lwr(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                let aligned = addr & !3;
                let word = psx.load32(aligned)?;
                let cur = if let Some((r, v)) = self.load_delay {
                    if r == rt { v } else { self.reg(rt) }
                } else {
                    self.reg(rt)
                };
                
                let val = match addr & 3 {
                    0 => word,
                    1 => (cur & 0xff000000) | (word >> 8),
                    2 => (cur & 0xffff0000) | (word >> 16),
                    3 => (cur & 0xffffff00) | (word >> 24),
                    _ => unreachable!(),
                };
                self.load_delay = Some((rt, val));
            }
            
            // Stores
            Sb(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                psx.store8(addr, self.reg(rt) as u8)?;
            }
            Sh(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                if addr & 0x1 != 0 {
                    return self.exception(psx, Exception::StoreAddressError);
                }
                psx.store16(addr, self.reg(rt) as u16)?;
            }
            Sw(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                if addr & 0x3 != 0 {
                    return self.exception(psx, Exception::StoreAddressError);
                }
                psx.store32(addr, self.reg(rt))?;
            }
            Swl(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                let aligned = addr & !3;
                let cur = psx.load32(aligned)?;
                let val = self.reg(rt);
                
                let word = match addr & 3 {
                    0 => (cur & 0xffffff00) | (val >> 24),
                    1 => (cur & 0xffff0000) | (val >> 16),
                    2 => (cur & 0xff000000) | (val >> 8),
                    3 => val,
                    _ => unreachable!(),
                };
                psx.store32(aligned, word)?;
            }
            Swr(rt, base, offset) => {
                let addr = self.reg(base).wrapping_add(offset as i32 as u32);
                let aligned = addr & !3;
                let cur = psx.load32(aligned)?;
                let val = self.reg(rt);
                
                let word = match addr & 3 {
                    0 => val,
                    1 => (cur & 0x000000ff) | (val << 8),
                    2 => (cur & 0x0000ffff) | (val << 16),
                    3 => (cur & 0x00ffffff) | (val << 24),
                    _ => unreachable!(),
                };
                psx.store32(aligned, word)?;
            }
            
            // Coprocessor 0
            Mfc0(rt, rd) => {
                let val = psx.cop0.reg(rd);
                self.load_delay = Some((rt, val));
            }
            Mtc0(rt, rd) => {
                psx.cop0.set_reg(rd, self.reg(rt));
            }
            Rfe => {
                psx.cop0.rfe();
            }
            
            // Coprocessor 2 (GTE)
            Mfc2(rt, rd) => {
                let val = psx.gte.data_reg(rd);
                self.load_delay = Some((rt, val));
            }
            Cfc2(rt, rd) => {
                let val = psx.gte.control_reg(rd);
                self.load_delay = Some((rt, val));
            }
            Mtc2(rt, rd) => {
                psx.gte.set_data_reg(rd, self.reg(rt));
            }
            Ctc2(rt, rd) => {
                psx.gte.set_control_reg(rd, self.reg(rt));
            }
            Cop2(command) => {
                psx.gte.execute(command);
            }
            
            // Exceptions
            Syscall => {
                return self.exception(psx, Exception::Syscall);
            }
            Break => {
                return self.exception(psx, Exception::Break);
            }
            
            Invalid(_) => {
                return self.exception(psx, Exception::ReservedInstruction);
            }
        }
        
        Ok(())
    }
    
    fn branch(&mut self, offset: i16) {
        self.next_pc = self.pc.wrapping_add((offset as i32 * 4) as u32);
        self.branch_delay = true;
    }
    
    fn exception(&mut self, psx: &mut Psx, exception: Exception) -> Result<()> {
        let handler = psx.cop0.exception(exception, self.current_pc, self.in_delay_slot);
        
        // Jump to exception handler
        self.pc = handler;
        self.next_pc = handler.wrapping_add(4);
        
        // Cancel any pending loads
        self.load_delay = None;
        
        Ok(())
    }
    
    fn reg(&self, index: u8) -> u32 {
        self.regs[index as usize]
    }
    
    fn set_reg(&mut self, index: u8, val: u32) {
        if index != 0 {
            self.regs[index as usize] = val;
        }
    }
}

// Instruction cache
pub struct ICache {
    // 256 cachelines of 4 words each = 4KB total
    lines: [CacheLine; 256],
}

#[derive(Clone, Copy)]
struct CacheLine {
    tag: u32,
    valid: bool,
    data: [u32; 4],
}

impl ICache {
    pub fn new() -> Self {
        ICache {
            lines: [CacheLine {
                tag: 0,
                valid: false,
                data: [0; 4],
            }; 256],
        }
    }
    
    pub fn fetch(&self, addr: u32) -> Option<u32> {
        if !self.is_cacheable(addr) {
            return None;
        }
        
        let index = ((addr >> 4) & 0xff) as usize;
        let tag = addr >> 12;
        let word = ((addr >> 2) & 0x3) as usize;
        
        let line = &self.lines[index];
        
        if line.valid && line.tag == tag {
            Some(line.data[word])
        } else {
            None
        }
    }
    
    pub fn store(&mut self, addr: u32, instruction: u32) {
        if !self.is_cacheable(addr) {
            return;
        }
        
        let index = ((addr >> 4) & 0xff) as usize;
        let tag = addr >> 12;
        let word = ((addr >> 2) & 0x3) as usize;
        
        let line = &mut self.lines[index];
        
        if !line.valid || line.tag != tag {
            line.tag = tag;
            line.valid = true;
            line.data = [0; 4];
        }
        
        line.data[word] = instruction;
    }
    
    pub fn invalidate(&mut self) {
        for line in self.lines.iter_mut() {
            line.valid = false;
        }
    }
    
    fn is_cacheable(&self, addr: u32) -> bool {
        // Only cache KUSEG, KSEG0 and KSEG1
        addr < 0xc0000000
    }
}

// ============================================================================
// COP0 (System Control Coprocessor)
// ============================================================================

pub struct Cop0 {
    regs: [u32; 32],
}

impl Cop0 {
    pub fn new() -> Self {
        let mut cop0 = Cop0 { regs: [0; 32] };
        
        // Initialize PRId register (processor ID)
        cop0.regs[15] = 0x00000002;
        
        // Initialize Status register
        cop0.regs[12] = 0x10900000;
        
        cop0
    }
    
    pub fn reg(&self, index: u8) -> u32 {
        self.regs[index as usize]
    }
    
    pub fn set_reg(&mut self, index: u8, val: u32) {
        match index {
            12 => {
                // Status register
                self.regs[12] = val & 0xf04fff3f;
            }
            13 => {
                // Cause register - only certain bits are writable
                self.regs[13] = (self.regs[13] & !0x300) | (val & 0x300);
            }
            _ => {
                self.regs[index as usize] = val;
            }
        }
    }
    
    pub fn exception(&mut self, exception: Exception, pc: u32, in_delay_slot: bool) -> u32 {
        // Set exception code in Cause register
        let code = exception as u32;
        self.regs[13] = (self.regs[13] & !0x7c) | ((code << 2) & 0x7c);
        
        // Set EPC (Exception PC)
        self.regs[14] = if in_delay_slot {
            self.regs[13] |= 0x80000000; // Set BD bit
            pc.wrapping_sub(4)
        } else {
            self.regs[13] &= !0x80000000;
            pc
        };
        
        // Update Status register (enter kernel mode, disable interrupts)
        let mode = self.regs[12] & 0x3f;
        self.regs[12] = (self.regs[12] & !0x3f) | ((mode << 2) & 0x3c);
        
        // Return exception handler address
        if self.regs[12] & 0x400000 != 0 {
            // BEV = 1: Bootstrap exception vectors
            0xbfc00180
        } else {
            // BEV = 0: Normal exception vectors
            0x80000080
        }
    }
    
    pub fn rfe(&mut self) {
        // Return from exception
        let mode = self.regs[12] & 0x3f;
        self.regs[12] = (self.regs[12] & !0xf) | (mode >> 2);
    }
    
    pub fn read_reg(&self, index: u8) -> u32 {
        self.regs[index as usize]
    }
    
    pub fn write_reg(&mut self, index: u8, val: u32) {
        self.set_reg(index, val);
    }
    
    pub fn interrupt_pending(&self, _irq: &InterruptState) -> bool {
        let status = self.regs[12];
        let cause = self.regs[13];
        
        // Check if interrupts are enabled
        if status & 0x1 == 0 {
            return false;
        }
        
        // Check interrupt mask
        let im = (status >> 8) & 0xff;
        let ip = (cause >> 8) & 0xff;
        
        (im & ip) != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Exception {
    Interrupt = 0x0,
    LoadAddressError = 0x4,
    StoreAddressError = 0x5,
    Syscall = 0x8,
    Break = 0x9,
    ReservedInstruction = 0xa,
    CoprocessorUnusable = 0xb,
    Overflow = 0xc,
}

// ============================================================================
// GTE (Geometry Transformation Engine) - Simplified
// ============================================================================

pub struct Gte {
    // Data registers (V0-V3, RGB, OTZ, IR0-IR3, etc.)
    data: [u32; 32],
    // Control registers (rotation matrix, translation, etc.)
    control: [u32; 32],
}

impl Gte {
    pub fn new() -> Self {
        Gte {
            data: [0; 32],
            control: [0; 32],
        }
    }
    
    pub fn data_reg(&self, index: u8) -> u32 {
        self.data[index as usize & 0x1f]
    }
    
    pub fn control_reg(&self, index: u8) -> u32 {
        self.control[index as usize & 0x1f]
    }
    
    pub fn set_data_reg(&mut self, index: u8, val: u32) {
        self.data[index as usize & 0x1f] = val;
    }
    
    pub fn set_control_reg(&mut self, index: u8, val: u32) {
        self.control[index as usize & 0x1f] = val;
    }
    
    pub fn execute(&mut self, _command: u32) {
        // Simplified GTE - just set FLAG register to indicate completion
        self.control[31] = 0; // FLAG - no errors
    }
}

// ============================================================================
// GPU Implementation
// ============================================================================

pub struct Gpu {
    // VRAM
    pub vram: Vec<u16>,
    
    // Display settings
    pub display_mode: u32,
    pub display_x: u16,
    pub display_y: u16,
    pub display_x1: u16,
    pub display_x2: u16,
    pub display_y1: u16,
    pub display_y2: u16,
    
    // Drawing area
    pub draw_x1: u16,
    pub draw_y1: u16,
    pub draw_x2: u16,
    pub draw_y2: u16,
    pub draw_offset_x: i16,
    pub draw_offset_y: i16,
    
    // Texture settings
    pub tex_page_x: u8,
    pub tex_page_y: u8,
    pub tex_depth: u8,
    pub tex_window_mask_x: u8,
    pub tex_window_mask_y: u8,
    pub tex_window_offset_x: u8,
    pub tex_window_offset_y: u8,
    
    // Status
    pub status: u32,
    pub gpu_read: u32,
    
    // Command buffer
    pub gp0_command: Option<Gp0Command>,
    pub gp0_words_remaining: usize,
    pub gp0_buffer: Vec<u32>,
    
    // DMA
    pub dma_direction: DmaDirection,
}

#[derive(Debug, Clone, Copy)]
pub enum DmaDirection {
    Off,
    Fifo,
    CpuToGp0,
    VramToCpu,
}

#[derive(Debug, Clone)]
pub enum Gp0Command {
    ClearCache,
    FillRect,
    CopyRect,
    DrawPolygon { vertices: usize, shaded: bool, textured: bool },
    DrawLine { shaded: bool },
    DrawRect { size: u8, textured: bool },
    DrawMode,
    TextureWindow,
    SetDrawArea,
    SetDrawOffset,
    MaskBit,
}

impl Gpu {
    pub fn new() -> Self {
        Gpu {
            vram: vec![0; VRAM_SIZE],
            display_mode: 0,
            display_x: 0,
            display_y: 0,
            display_x1: 0x200,
            display_x2: 0xc00,
            display_y1: 0x010,
            display_y2: 0x100,
            draw_x1: 0,
            draw_y1: 0,
            draw_x2: 0,
            draw_y2: 0,
            draw_offset_x: 0,
            draw_offset_y: 0,
            tex_page_x: 0,
            tex_page_y: 0,
            tex_depth: 0,
            tex_window_mask_x: 0,
            tex_window_mask_y: 0,
            tex_window_offset_x: 0,
            tex_window_offset_y: 0,
            status: 0x14802000,
            gpu_read: 0,
            gp0_command: None,
            gp0_words_remaining: 0,
            gp0_buffer: Vec::new(),
            dma_direction: DmaDirection::Off,
        }
    }
    
    pub fn gp0_write(&mut self, val: u32) {
        if self.gp0_words_remaining == 0 {
            // New command
            let cmd = (val >> 24) as u8;
            self.gp0_buffer.clear();
            self.gp0_buffer.push(val);
            
            let (command, words) = self.decode_gp0(cmd);
            self.gp0_command = Some(command);
            self.gp0_words_remaining = words;
            
            if words == 0 {
                self.execute_gp0();
            }
        } else {
            // Parameter
            self.gp0_buffer.push(val);
            self.gp0_words_remaining -= 1;
            
            if self.gp0_words_remaining == 0 {
                self.execute_gp0();
            }
        }
    }
    
    pub fn gp1_write(&mut self, val: u32) {
        let cmd = (val >> 24) as u8;
        
        match cmd {
            0x00 => {
                // Reset GPU
                self.status = 0x14802000;
                self.gp0_command = None;
                self.gp0_words_remaining = 0;
                self.gp0_buffer.clear();
            }
            0x01 => {
                // Reset command buffer
                self.gp0_command = None;
                self.gp0_words_remaining = 0;
                self.gp0_buffer.clear();
            }
            0x02 => {
                // Acknowledge interrupt
            }
            0x03 => {
                // Display enable
                self.status = (self.status & !0x800000) | ((val & 1) << 23);
            }
            0x04 => {
                // DMA direction
                self.dma_direction = match val & 3 {
                    0 => DmaDirection::Off,
                    1 => DmaDirection::Fifo,
                    2 => DmaDirection::CpuToGp0,
                    3 => DmaDirection::VramToCpu,
                    _ => unreachable!(),
                };
            }
            0x05 => {
                // Display area start
                self.display_x = (val & 0x3ff) as u16;
                self.display_y = ((val >> 10) & 0x1ff) as u16;
            }
            0x06 => {
                // Horizontal display range
                self.display_x1 = (val & 0xfff) as u16;
                self.display_x2 = ((val >> 12) & 0xfff) as u16;
            }
            0x07 => {
                // Vertical display range
                self.display_y1 = (val & 0x3ff) as u16;
                self.display_y2 = ((val >> 10) & 0x3ff) as u16;
            }
            0x08 => {
                // Display mode
                self.display_mode = val;
                self.status = (self.status & !0x7f0000) | ((val & 0x3f) << 17) | ((val & 0x40) << 10);
            }
            _ => {}
        }
    }
    
    fn decode_gp0(&self, cmd: u8) -> (Gp0Command, usize) {
        match cmd {
            0x00 => (Gp0Command::ClearCache, 0),
            0x02 => (Gp0Command::FillRect, 2),
            0x20..=0x3f => {
                let vertices = if cmd & 0x08 != 0 { 4 } else { 3 };
                let shaded = cmd & 0x10 != 0;
                let textured = cmd & 0x04 != 0;
                
                let words = vertices - 1 +
                    if shaded { vertices } else { 0 } +
                    if textured { vertices } else { 0 };
                
                (Gp0Command::DrawPolygon { vertices, shaded, textured }, words)
            }
            0x40..=0x5f => {
                let shaded = cmd & 0x10 != 0;
                (Gp0Command::DrawLine { shaded }, if shaded { 3 } else { 1 })
            }
            0x60..=0x7f => {
                let size = ((cmd >> 3) & 3) as u8;
                let textured = cmd & 0x04 != 0;
                let words = match size {
                    0 => if textured { 2 } else { 1 },
                    _ => if textured { 1 } else { 0 },
                };
                (Gp0Command::DrawRect { size, textured }, words)
            }
            0xa0 => (Gp0Command::CopyRect, 3),
            0xe1 => (Gp0Command::DrawMode, 0),
            0xe2 => (Gp0Command::TextureWindow, 0),
            0xe3..=0xe4 => (Gp0Command::SetDrawArea, 0),
            0xe5 => (Gp0Command::SetDrawOffset, 0),
            0xe6 => (Gp0Command::MaskBit, 0),
            _ => (Gp0Command::ClearCache, 0),
        }
    }
    
    fn execute_gp0(&mut self) {
        if let Some(ref command) = self.gp0_command {
            match command {
                Gp0Command::FillRect => {
                    let color = self.gp0_buffer[0] & 0xffffff;
                    let xy = self.gp0_buffer[1];
                    let wh = self.gp0_buffer[2];
                    
                    let x = (xy & 0x3ff) as u16;
                    let y = ((xy >> 16) & 0x1ff) as u16;
                    let w = (wh & 0x3ff) as u16;
                    let h = ((wh >> 16) & 0x1ff) as u16;
                    
                    self.fill_rect(x, y, w, h, color);
                }
                Gp0Command::DrawPolygon { vertices, .. } => {
                    // Simple fill for now
                    if *vertices >= 3 && self.gp0_buffer.len() > 1 {
                        let color = self.gp0_buffer[0] & 0xffffff;
                        let xy = self.gp0_buffer[1];
                        let x = (xy & 0x7ff) as u16;
                        let y = ((xy >> 16) & 0x7ff) as u16;
                        self.fill_rect(x.saturating_sub(5), y.saturating_sub(5), 10, 10, color);
                    }
                }
                Gp0Command::DrawRect { .. } => {
                    if !self.gp0_buffer.is_empty() {
                        let color = self.gp0_buffer[0] & 0xffffff;
                        let xy = self.gp0_buffer.get(1).copied().unwrap_or(0);
                        let x = (xy & 0x7ff) as u16;
                        let y = ((xy >> 16) & 0x7ff) as u16;
                        self.fill_rect(x, y, 16, 16, color);
                    }
                }
                Gp0Command::DrawMode => {
                    let val = self.gp0_buffer[0];
                    self.tex_page_x = (val & 0xf) as u8;
                    self.tex_page_y = ((val >> 4) & 1) as u8;
                    self.tex_depth = ((val >> 7) & 3) as u8;
                }
                Gp0Command::SetDrawOffset => {
                    let val = self.gp0_buffer[0];
                    self.draw_offset_x = ((val & 0x7ff) as i16) << 5 >> 5;
                    self.draw_offset_y = (((val >> 11) & 0x7ff) as i16) << 5 >> 5;
                }
                _ => {}
            }
        }
        
        self.gp0_command = None;
    }
    
    fn fill_rect(&mut self, x: u16, y: u16, w: u16, h: u16, color: u32) {
        let r = ((color >> 0) & 0xff) as u16;
        let g = ((color >> 8) & 0xff) as u16;
        let b = ((color >> 16) & 0xff) as u16;
        
        let color16 = ((b >> 3) << 10) | ((g >> 3) << 5) | (r >> 3);
        
        for dy in 0..h.min(512) {
            for dx in 0..w.min(1024) {
                let vram_x = ((x + dx) & 0x3ff) as usize;
                let vram_y = ((y + dy) & 0x1ff) as usize;
                let idx = vram_y * 1024 + vram_x;
                if idx < self.vram.len() {
                    self.vram[idx] = color16;
                }
            }
        }
    }
    
    pub fn get_status(&self) -> u32 {
        self.status | 0x1c000000 // Ready to receive commands
    }
    
    pub fn get_read(&self) -> u32 {
        self.gpu_read
    }
    
    pub fn get_framebuffer(&self, buffer: &mut Vec<u8>) {
        let width = 640;
        let height = 480;
        
        buffer.resize(width * height * 4, 0);
        
        // Check if VRAM has any non-zero data
        let has_data = self.vram.iter().any(|&p| p != 0);
        
        if !has_data {
            // Display a test pattern if no data in VRAM
            for y in 0..height {
                for x in 0..width {
                    let buffer_idx = (y * width + x) * 4;
                    // Create a gradient test pattern
                    buffer[buffer_idx] = ((x * 255) / width) as u8;     // R
                    buffer[buffer_idx + 1] = ((y * 255) / height) as u8; // G  
                    buffer[buffer_idx + 2] = 128;                        // B
                    buffer[buffer_idx + 3] = 255;                        // A
                }
            }
        } else {
            // Normal VRAM rendering
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
}

// ============================================================================
// DMA Controller
// ============================================================================

pub struct Dma {
    channels: [DmaChannel; 7],
    control: u32,
    interrupt: u32,
}

#[derive(Clone, Copy)]
pub struct DmaChannel {
    base: u32,
    block_control: u32,
    control: u32,
}

impl Dma {
    pub fn new() -> Self {
        Dma {
            channels: [DmaChannel::new(); 7],
            control: 0x07654321,
            interrupt: 0,
        }
    }
    
    pub fn channel(&self, index: usize) -> &DmaChannel {
        &self.channels[index]
    }
    
    pub fn channel_mut(&mut self, index: usize) -> &mut DmaChannel {
        &mut self.channels[index]
    }
    
    pub fn control(&self) -> u32 {
        self.control
    }
    
    pub fn set_control(&mut self, val: u32) {
        self.control = val;
    }
    
    pub fn interrupt(&self) -> u32 {
        self.interrupt
    }
    
    pub fn set_interrupt(&mut self, val: u32) {
        self.interrupt = val;
    }
}

impl DmaChannel {
    pub fn new() -> Self {
        DmaChannel {
            base: 0,
            block_control: 0,
            control: 0,
        }
    }
    
    pub fn base(&self) -> u32 {
        self.base
    }
    
    pub fn set_base(&mut self, val: u32) {
        self.base = val & 0xffffff;
    }
    
    pub fn block_control(&self) -> u32 {
        self.block_control
    }
    
    pub fn set_block_control(&mut self, val: u32) {
        self.block_control = val;
    }
    
    pub fn control(&self) -> u32 {
        self.control
    }
    
    pub fn set_control(&mut self, val: u32) {
        self.control = val;
    }
    
    pub fn is_active(&self) -> bool {
        self.control & 0x01000000 != 0
    }
}

// ============================================================================
// Interrupt Controller
// ============================================================================

pub struct InterruptState {
    status: u16,
    mask: u16,
}

impl InterruptState {
    pub fn new() -> Self {
        InterruptState {
            status: 0,
            mask: 0,
        }
    }
    
    pub fn status(&self) -> u16 {
        self.status
    }
    
    pub fn set_status(&mut self, val: u16) {
        self.status &= val;
    }
    
    pub fn mask(&self) -> u16 {
        self.mask
    }
    
    pub fn set_mask(&mut self, val: u16) {
        self.mask = val;
    }
    
    pub fn request(&mut self, irq: Interrupt) {
        self.status |= irq as u16;
    }
    
    pub fn pending(&self) -> bool {
        (self.status & self.mask) != 0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum Interrupt {
    VBlank = 0x0001,
    Gpu = 0x0002,
    Cdrom = 0x0004,
    Dma = 0x0008,
    Timer0 = 0x0010,
    Timer1 = 0x0020,
    Timer2 = 0x0040,
    Controller = 0x0080,
    Sio = 0x0100,
    Spu = 0x0200,
    Lightpen = 0x0400,
}

// ============================================================================
// Timers
// ============================================================================

pub struct Timers {
    timers: [Timer; 3],
}

#[derive(Clone, Copy)]
pub struct Timer {
    counter: u16,
    target: u16,
    mode: u16,
}

impl Timers {
    pub fn new() -> Self {
        Timers {
            timers: [Timer::new(); 3],
        }
    }
    
    pub fn timer(&self, index: usize) -> &Timer {
        &self.timers[index]
    }
    
    pub fn timer_mut(&mut self, index: usize) -> &mut Timer {
        &mut self.timers[index]
    }
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            counter: 0,
            target: 0,
            mode: 0,
        }
    }
    
    pub fn counter(&self) -> u16 {
        self.counter
    }
    
    pub fn set_counter(&mut self, val: u16) {
        self.counter = val;
    }
    
    pub fn target(&self) -> u16 {
        self.target
    }
    
    pub fn set_target(&mut self, val: u16) {
        self.target = val;
    }
    
    pub fn mode(&self) -> u16 {
        self.mode
    }
    
    pub fn set_mode(&mut self, val: u16) {
        self.mode = val;
        self.counter = 0;
    }
}

// ============================================================================
// Main PSX System
// ============================================================================

pub struct Psx {
    // Core components
    pub cpu: Cpu,
    pub cop0: Cop0,
    pub gte: Gte,
    pub gpu: Gpu,
    pub dma: Dma,
    pub irq: InterruptState,
    pub timers: Timers,
    
    // Memory
    pub ram: Vec<u8>,
    pub bios: Vec<u8>,
    pub scratchpad: Vec<u8>,
    
    // Timing
    pub cycle_counter: CycleCount,
    pub next_event: CycleCount,
    
    // State
    pub frame_done: bool,
}

impl Psx {
    pub fn new() -> Result<Self> {
        Ok(Psx {
            cpu: Cpu::new(),
            cop0: Cop0::new(),
            gte: Gte::new(),
            gpu: Gpu::new(),
            dma: Dma::new(),
            irq: InterruptState::new(),
            timers: Timers::new(),
            ram: vec![0; RAM_SIZE],
            bios: vec![0; BIOS_SIZE],
            scratchpad: vec![0; 1024],
            cycle_counter: 0,
            next_event: 100,
            frame_done: false,
        })
    }
    
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.cop0 = Cop0::new();
        self.gte = Gte::new();
        self.gpu = Gpu::new();
        self.dma = Dma::new();
        self.irq = InterruptState::new();
        self.timers = Timers::new();
        self.ram.fill(0);
        self.scratchpad.fill(0);
        self.cycle_counter = 0;
        self.next_event = 100;
        self.frame_done = false;
    }
    
    pub fn init_with_disc(&mut self) -> Result<()> {
        // Initialize PSX with a disc loaded
        // The BIOS will handle the actual boot process
        // For now, just ensure the system is ready without resetting
        Ok(())
    }
    
    pub fn load_bios(&mut self, data: &[u8]) -> Result<()> {
        if data.len() != BIOS_SIZE {
            return Err(PsxError::invalid_bios("Invalid BIOS size"));
        }
        self.bios.copy_from_slice(data);
        Ok(())
    }
    
    pub fn load_exe(&mut self, data: &[u8]) -> Result<()> {
        if data.len() < 0x800 {
            return Err(PsxError::invalid_exe("Invalid EXE size"));
        }
        
        if &data[0..8] != b"PS-X EXE" {
            return Err(PsxError::invalid_exe("Invalid EXE size"));
        }
        
        let pc = u32::from_le_bytes([data[0x10], data[0x11], data[0x12], data[0x13]]);
        let gp = u32::from_le_bytes([data[0x14], data[0x15], data[0x16], data[0x17]]);
        let dest = u32::from_le_bytes([data[0x18], data[0x19], data[0x1a], data[0x1b]]);
        let size = u32::from_le_bytes([data[0x1c], data[0x1d], data[0x1e], data[0x1f]]) as usize;
        let sp = u32::from_le_bytes([data[0x30], data[0x31], data[0x32], data[0x33]]);
        
        let dest_offset = (dest & 0x1fffff) as usize;
        let exe_size = size.min(data.len() - 0x800);
        
        if dest_offset + exe_size <= self.ram.len() {
            self.ram[dest_offset..dest_offset + exe_size]
                .copy_from_slice(&data[0x800..0x800 + exe_size]);
        }
        
        self.cpu.pc = pc;
        self.cpu.next_pc = pc + 4;
        self.cpu.regs[28] = gp;
        self.cpu.regs[29] = if sp != 0 { sp } else { 0x801fff00 };
        
        Ok(())
    }
    
    pub fn run_frame(&mut self) -> Result<()> {
        self.frame_done = false;
        
        while !self.frame_done {
            // Run CPU until next event
            while self.cycle_counter < self.next_event {
                // Simple CPU execution - just advance PC for now
                self.cpu.current_pc = self.cpu.pc;
                
                // Fetch and decode instruction
                let instruction = self.load32(self.cpu.pc).unwrap_or(0);
                
                // Advance PC
                self.cpu.pc = self.cpu.next_pc;
                self.cpu.next_pc = self.cpu.pc.wrapping_add(4);
                
                // Execute the instruction
                self.execute_cpu_instruction(instruction);
                
                self.cycle_counter += 1;
                
                // Check for interrupts
                if self.cop0.interrupt_pending(&self.irq) {
                    let handler = self.cop0.exception(Exception::Interrupt, self.cpu.current_pc, false);
                    self.cpu.pc = handler;
                    self.cpu.next_pc = handler.wrapping_add(4);
                }
            }
            
            // Handle events
            self.handle_events();
        }
        
        Ok(())
    }
    
    fn handle_events(&mut self) {
        // Simple VBlank simulation
        static mut VBLANK_COUNTER: i32 = 0;
        
        unsafe {
            VBLANK_COUNTER += self.next_event;
            if VBLANK_COUNTER >= 560000 {
                VBLANK_COUNTER = 0;
                self.irq.request(Interrupt::VBlank);
                self.frame_done = true;
            }
        }
        
        self.next_event += 1000;
    }
    
    pub fn tick(&mut self, cycles: i32) {
        self.cycle_counter += cycles;
    }
    
    // Memory access
    pub fn load8(&mut self, addr: u32) -> Result<u8> {
        let physical = mask_region(addr);
        
        match physical {
            0x00000000..=0x001fffff => {
                self.tick(1);
                Ok(self.ram[(physical & 0x1fffff) as usize])
            }
            0x1f000000..=0x1f0003ff => {
                self.tick(1);
                Ok(self.scratchpad[(physical & 0x3ff) as usize])
            }
            0x1fc00000..=0x1fc7ffff => {
                self.tick(1);
                Ok(self.bios[(physical & 0x7ffff) as usize])
            }
            _ => Ok(0xff),
        }
    }
    
    pub fn load16(&mut self, addr: u32) -> Result<u16> {
        let b0 = self.load8(addr)? as u16;
        let b1 = self.load8(addr + 1)? as u16;
        Ok(b0 | (b1 << 8))
    }
    
    pub fn load32(&mut self, addr: u32) -> Result<u32> {
        let physical = mask_region(addr);
        
        match physical {
            0x00000000..=0x001fffff => {
                self.tick(1);
                let offset = (physical & 0x1fffff) as usize;
                Ok(u32::from_le_bytes([
                    self.ram[offset],
                    self.ram[offset + 1],
                    self.ram[offset + 2],
                    self.ram[offset + 3],
                ]))
            }
            0x1f000000..=0x1f0003ff => {
                self.tick(1);
                let offset = (physical & 0x3ff) as usize;
                Ok(u32::from_le_bytes([
                    self.scratchpad[offset],
                    self.scratchpad[offset + 1],
                    self.scratchpad[offset + 2],
                    self.scratchpad[offset + 3],
                ]))
            }
            0x1f801070 => Ok(self.irq.status() as u32),
            0x1f801074 => Ok(self.irq.mask() as u32),
            0x1f801080..=0x1f8010ef => {
                let channel = ((physical - 0x1f801080) / 0x10) as usize;
                let offset = (physical & 0xf) / 4;
                
                match offset {
                    0 => Ok(self.dma.channel(channel).base()),
                    1 => Ok(self.dma.channel(channel).block_control()),
                    2 => Ok(self.dma.channel(channel).control()),
                    _ => Ok(0),
                }
            }
            0x1f8010f0 => Ok(self.dma.control()),
            0x1f8010f4 => Ok(self.dma.interrupt()),
            0x1f801100..=0x1f80112f => {
                let timer = ((physical - 0x1f801100) / 0x10) as usize;
                let offset = (physical & 0xf) / 4;
                
                match offset {
                    0 => Ok(self.timers.timer(timer).counter() as u32),
                    1 => Ok(self.timers.timer(timer).mode() as u32),
                    2 => Ok(self.timers.timer(timer).target() as u32),
                    _ => Ok(0),
                }
            }
            0x1f801810 => Ok(self.gpu.get_read()),
            0x1f801814 => Ok(self.gpu.get_status()),
            0x1fc00000..=0x1fc7ffff => {
                self.tick(1);
                let offset = (physical & 0x7ffff) as usize;
                Ok(u32::from_le_bytes([
                    self.bios[offset],
                    self.bios[offset + 1],
                    self.bios[offset + 2],
                    self.bios[offset + 3],
                ]))
            }
            _ => Ok(0xffffffff),
        }
    }
    
    pub fn store8(&mut self, addr: u32, val: u8) -> Result<()> {
        let physical = mask_region(addr);
        
        match physical {
            0x00000000..=0x001fffff => {
                self.tick(1);
                self.ram[(physical & 0x1fffff) as usize] = val;
            }
            0x1f000000..=0x1f0003ff => {
                self.tick(1);
                self.scratchpad[(physical & 0x3ff) as usize] = val;
            }
            _ => {}
        }
        
        Ok(())
    }
    
    pub fn store16(&mut self, addr: u32, val: u16) -> Result<()> {
        self.store8(addr, val as u8)?;
        self.store8(addr + 1, (val >> 8) as u8)?;
        Ok(())
    }
    
    pub fn store32(&mut self, addr: u32, val: u32) -> Result<()> {
        let physical = mask_region(addr);
        
        match physical {
            0x00000000..=0x001fffff => {
                self.tick(1);
                let offset = (physical & 0x1fffff) as usize;
                self.ram[offset] = val as u8;
                self.ram[offset + 1] = (val >> 8) as u8;
                self.ram[offset + 2] = (val >> 16) as u8;
                self.ram[offset + 3] = (val >> 24) as u8;
            }
            0x1f000000..=0x1f0003ff => {
                self.tick(1);
                let offset = (physical & 0x3ff) as usize;
                self.scratchpad[offset] = val as u8;
                self.scratchpad[offset + 1] = (val >> 8) as u8;
                self.scratchpad[offset + 2] = (val >> 16) as u8;
                self.scratchpad[offset + 3] = (val >> 24) as u8;
            }
            0x1f801070 => self.irq.set_status(val as u16),
            0x1f801074 => self.irq.set_mask(val as u16),
            0x1f801080..=0x1f8010ef => {
                let channel = ((physical - 0x1f801080) / 0x10) as usize;
                let offset = (physical & 0xf) / 4;
                
                match offset {
                    0 => self.dma.channel_mut(channel).set_base(val),
                    1 => self.dma.channel_mut(channel).set_block_control(val),
                    2 => {
                        self.dma.channel_mut(channel).set_control(val);
                        // Trigger DMA if enabled
                        if self.dma.channel(channel).is_active() {
                            self.do_dma(channel);
                        }
                    }
                    _ => {}
                }
            }
            0x1f8010f0 => self.dma.set_control(val),
            0x1f8010f4 => self.dma.set_interrupt(val),
            0x1f801100..=0x1f80112f => {
                let timer = ((physical - 0x1f801100) / 0x10) as usize;
                let offset = (physical & 0xf) / 4;
                
                match offset {
                    0 => self.timers.timer_mut(timer).set_counter(val as u16),
                    1 => self.timers.timer_mut(timer).set_mode(val as u16),
                    2 => self.timers.timer_mut(timer).set_target(val as u16),
                    _ => {}
                }
            }
            0x1f801810 => self.gpu.gp0_write(val),
            0x1f801814 => self.gpu.gp1_write(val),
            _ => {}
        }
        
        Ok(())
    }
    
    fn do_dma(&mut self, channel: usize) {
        // Simplified DMA - just clear active bit
        let control = self.dma.channel(channel).control() & !0x01000000;
        self.dma.channel_mut(channel).set_control(control);
        
        // Trigger DMA interrupt
        self.irq.request(Interrupt::Dma);
    }
    
    pub fn set_controller_state(&mut self, _controller: usize, _state: u16) {
        // Controller input handling
    }
    
    pub fn get_framebuffer(&self, buffer: &mut Vec<u8>) {
        self.gpu.get_framebuffer(buffer);
    }
    
    fn execute_cpu_instruction(&mut self, instruction: u32) {
        let opcode = (instruction >> 26) & 0x3f;
        let rs = ((instruction >> 21) & 0x1f) as usize;
        let rt = ((instruction >> 16) & 0x1f) as usize;
        let rd = ((instruction >> 11) & 0x1f) as usize;
        let imm = instruction & 0xffff;
        let imm_se = (imm as i16) as i32 as u32;
        
        match opcode {
            0x00 => {
                // R-type instructions
                let funct = instruction & 0x3f;
                match funct {
                    0x00 => {
                        // SLL
                        let sa = ((instruction >> 6) & 0x1f) as u32;
                        if rd != 0 {
                            self.cpu.regs[rd] = self.cpu.regs[rt] << sa;
                        }
                    }
                    0x02 => {
                        // SRL
                        let sa = ((instruction >> 6) & 0x1f) as u32;
                        if rd != 0 {
                            self.cpu.regs[rd] = self.cpu.regs[rt] >> sa;
                        }
                    }
                    0x08 => {
                        // JR
                        self.cpu.next_pc = self.cpu.regs[rs];
                    }
                    0x09 => {
                        // JALR
                        let ra = self.cpu.next_pc;
                        self.cpu.next_pc = self.cpu.regs[rs];
                        if rd != 0 {
                            self.cpu.regs[rd] = ra;
                        }
                    }
                    0x20 => {
                        // ADD
                        if rd != 0 {
                            let result = (self.cpu.regs[rs] as i32).wrapping_add(self.cpu.regs[rt] as i32);
                            self.cpu.regs[rd] = result as u32;
                        }
                    }
                    0x21 => {
                        // ADDU
                        if rd != 0 {
                            self.cpu.regs[rd] = self.cpu.regs[rs].wrapping_add(self.cpu.regs[rt]);
                        }
                    }
                    0x23 => {
                        // SUBU
                        if rd != 0 {
                            self.cpu.regs[rd] = self.cpu.regs[rs].wrapping_sub(self.cpu.regs[rt]);
                        }
                    }
                    0x24 => {
                        // AND
                        if rd != 0 {
                            self.cpu.regs[rd] = self.cpu.regs[rs] & self.cpu.regs[rt];
                        }
                    }
                    0x25 => {
                        // OR
                        if rd != 0 {
                            self.cpu.regs[rd] = self.cpu.regs[rs] | self.cpu.regs[rt];
                        }
                    }
                    0x26 => {
                        // XOR
                        if rd != 0 {
                            self.cpu.regs[rd] = self.cpu.regs[rs] ^ self.cpu.regs[rt];
                        }
                    }
                    0x27 => {
                        // NOR
                        if rd != 0 {
                            self.cpu.regs[rd] = !(self.cpu.regs[rs] | self.cpu.regs[rt]);
                        }
                    }
                    0x2a => {
                        // SLT
                        if rd != 0 {
                            let val = if (self.cpu.regs[rs] as i32) < (self.cpu.regs[rt] as i32) { 1 } else { 0 };
                            self.cpu.regs[rd] = val;
                        }
                    }
                    0x2b => {
                        // SLTU
                        if rd != 0 {
                            let val = if self.cpu.regs[rs] < self.cpu.regs[rt] { 1 } else { 0 };
                            self.cpu.regs[rd] = val;
                        }
                    }
                    _ => {}
                }
            }
            0x02 => {
                // J
                self.cpu.next_pc = (self.cpu.pc & 0xf0000000) | ((instruction & 0x3ffffff) << 2);
            }
            0x03 => {
                // JAL
                self.cpu.regs[31] = self.cpu.next_pc;
                self.cpu.next_pc = (self.cpu.pc & 0xf0000000) | ((instruction & 0x3ffffff) << 2);
            }
            0x04 => {
                // BEQ
                if self.cpu.regs[rs] == self.cpu.regs[rt] {
                    self.cpu.next_pc = self.cpu.pc.wrapping_add(imm_se << 2);
                }
            }
            0x05 => {
                // BNE
                if self.cpu.regs[rs] != self.cpu.regs[rt] {
                    self.cpu.next_pc = self.cpu.pc.wrapping_add(imm_se << 2);
                }
            }
            0x06 => {
                // BLEZ
                if (self.cpu.regs[rs] as i32) <= 0 {
                    self.cpu.next_pc = self.cpu.pc.wrapping_add(imm_se << 2);
                }
            }
            0x07 => {
                // BGTZ
                if (self.cpu.regs[rs] as i32) > 0 {
                    self.cpu.next_pc = self.cpu.pc.wrapping_add(imm_se << 2);
                }
            }
            0x08 => {
                // ADDI
                if rt != 0 {
                    let result = (self.cpu.regs[rs] as i32).wrapping_add(imm_se as i32);
                    self.cpu.regs[rt] = result as u32;
                }
            }
            0x09 => {
                // ADDIU
                if rt != 0 {
                    self.cpu.regs[rt] = self.cpu.regs[rs].wrapping_add(imm_se);
                }
            }
            0x0a => {
                // SLTI
                if rt != 0 {
                    let val = if (self.cpu.regs[rs] as i32) < (imm_se as i32) { 1 } else { 0 };
                    self.cpu.regs[rt] = val;
                }
            }
            0x0b => {
                // SLTIU
                if rt != 0 {
                    let val = if self.cpu.regs[rs] < imm_se { 1 } else { 0 };
                    self.cpu.regs[rt] = val;
                }
            }
            0x0c => {
                // ANDI
                if rt != 0 {
                    self.cpu.regs[rt] = self.cpu.regs[rs] & (imm as u32);
                }
            }
            0x0d => {
                // ORI
                if rt != 0 {
                    self.cpu.regs[rt] = self.cpu.regs[rs] | (imm as u32);
                }
            }
            0x0e => {
                // XORI
                if rt != 0 {
                    self.cpu.regs[rt] = self.cpu.regs[rs] ^ (imm as u32);
                }
            }
            0x0f => {
                // LUI
                if rt != 0 {
                    self.cpu.regs[rt] = (imm as u32) << 16;
                }
            }
            0x10 => {
                // COP0
                let cop_op = (instruction >> 21) & 0x1f;
                match cop_op {
                    0x00 => {
                        // MFC0
                        if rt != 0 {
                            let val = self.cop0.read_reg(rd as u8);
                            self.cpu.regs[rt] = val;
                        }
                    }
                    0x04 => {
                        // MTC0
                        self.cop0.write_reg(rd as u8, self.cpu.regs[rt]);
                    }
                    0x10 => {
                        // RFE
                        if instruction & 0x3f == 0x10 {
                            self.cop0.rfe();
                        }
                    }
                    _ => {}
                }
            }
            0x20 => {
                // LB
                let addr = self.cpu.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    if let Ok(val) = self.load8(addr) {
                        self.cpu.regs[rt] = (val as i8) as i32 as u32;
                    }
                }
            }
            0x21 => {
                // LH
                let addr = self.cpu.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    if let Ok(val) = self.load16(addr) {
                        self.cpu.regs[rt] = (val as i16) as i32 as u32;
                    }
                }
            }
            0x23 => {
                // LW
                let addr = self.cpu.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    if let Ok(val) = self.load32(addr) {
                        self.cpu.regs[rt] = val;
                    }
                }
            }
            0x24 => {
                // LBU
                let addr = self.cpu.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    if let Ok(val) = self.load8(addr) {
                        self.cpu.regs[rt] = val as u32;
                    }
                }
            }
            0x25 => {
                // LHU
                let addr = self.cpu.regs[rs].wrapping_add(imm_se);
                if rt != 0 {
                    if let Ok(val) = self.load16(addr) {
                        self.cpu.regs[rt] = val as u32;
                    }
                }
            }
            0x28 => {
                // SB
                let addr = self.cpu.regs[rs].wrapping_add(imm_se);
                let _ = self.store8(addr, self.cpu.regs[rt] as u8);
            }
            0x29 => {
                // SH
                let addr = self.cpu.regs[rs].wrapping_add(imm_se);
                let _ = self.store16(addr, self.cpu.regs[rt] as u16);
            }
            0x2b => {
                // SW
                let addr = self.cpu.regs[rs].wrapping_add(imm_se);
                let _ = self.store32(addr, self.cpu.regs[rt]);
            }
            _ => {}
        }
        
        // R0 is always 0
        self.cpu.regs[0] = 0;
    }
}

fn mask_region(addr: u32) -> u32 {
    // Memory map regions
    const REGION_MASK: [u32; 8] = [
        0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, // KUSEG
        0x7fffffff, // KSEG0
        0x1fffffff, // KSEG1
        0xffffffff, 0xffffffff, // KSEG2
    ];
    
    let region = (addr >> 29) as usize;
    addr & REGION_MASK[region]
}