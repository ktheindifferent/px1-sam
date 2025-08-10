//! CPU Instruction Handlers using Command Pattern
//!
//! This module refactors the massive instruction execution functions
//! into smaller, manageable instruction handlers following the command pattern.
//! This improves code maintainability, testability, and reduces cyclomatic complexity.

use super::memory_map;

// ============================================================================
// Instruction Traits and Types
// ============================================================================

/// Result type for instruction execution
pub type InstructionResult = Result<(), InstructionException>;

/// CPU exception types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionException {
    Overflow,
    AddressError(u32),
    BusError(u32),
    Syscall,
    Break,
    ReservedInstruction,
    CoprocessorUnusable,
    IntegerOverflow,
}

/// Trait for CPU state access required by instructions
pub trait CpuState {
    /// Read general purpose register
    fn get_reg(&self, index: u32) -> u32;
    
    /// Write general purpose register
    fn set_reg(&mut self, index: u32, value: u32);
    
    /// Get program counter
    fn get_pc(&self) -> u32;
    
    /// Set program counter
    fn set_pc(&mut self, value: u32);
    
    /// Get next PC (for branch delay slot)
    fn get_next_pc(&self) -> u32;
    
    /// Set next PC (for branches/jumps)
    fn set_next_pc(&mut self, value: u32);
    
    /// Read from memory
    fn read_memory(&self, addr: u32, size: MemorySize) -> Result<u32, InstructionException>;
    
    /// Write to memory
    fn write_memory(&mut self, addr: u32, value: u32, size: MemorySize) -> Result<(), InstructionException>;
    
    /// Get HI register (for multiplication/division)
    fn get_hi(&self) -> u32;
    
    /// Set HI register
    fn set_hi(&mut self, value: u32);
    
    /// Get LO register
    fn get_lo(&self) -> u32;
    
    /// Set LO register
    fn set_lo(&mut self, value: u32);
    
    /// Check if we're in a branch delay slot
    fn in_delay_slot(&self) -> bool;
    
    /// Set delay slot flag
    fn set_delay_slot(&mut self, value: bool);
}

/// Memory access size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySize {
    Byte,
    HalfWord,
    Word,
}

/// Base trait for all CPU instructions
pub trait Instruction {
    /// Execute the instruction
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult;
    
    /// Get instruction mnemonic for debugging
    fn mnemonic(&self) -> &'static str;
    
    /// Get instruction disassembly
    fn disassemble(&self) -> String {
        self.mnemonic().to_string()
    }
}

// ============================================================================
// Instruction Decoder
// ============================================================================

/// Decoded instruction format
#[derive(Debug, Clone, Copy)]
pub struct DecodedInstruction {
    pub opcode: u32,
    pub rs: u32,
    pub rt: u32,
    pub rd: u32,
    pub shamt: u32,
    pub funct: u32,
    pub immediate: u16,
    pub target: u32,
    pub raw: u32,
}

impl DecodedInstruction {
    pub fn decode(instruction: u32) -> Self {
        Self {
            opcode: (instruction >> 26) & 0x3f,
            rs: (instruction >> 21) & 0x1f,
            rt: (instruction >> 16) & 0x1f,
            rd: (instruction >> 11) & 0x1f,
            shamt: (instruction >> 6) & 0x1f,
            funct: instruction & 0x3f,
            immediate: instruction as u16,
            target: instruction & memory_map::JUMP_TARGET_MASK,
            raw: instruction,
        }
    }
    
    pub fn sign_extend_immediate(&self) -> u32 {
        self.immediate as i16 as i32 as u32
    }
}

// ============================================================================
// Arithmetic Instructions
// ============================================================================

/// ADD - Add with overflow check
pub struct Add {
    rd: u32,
    rs: u32,
    rt: u32,
}

impl Add {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rd: decoded.rd,
            rs: decoded.rs,
            rt: decoded.rt,
        }
    }
}

impl Instruction for Add {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let a = cpu.get_reg(self.rs) as i32;
        let b = cpu.get_reg(self.rt) as i32;
        
        match a.checked_add(b) {
            Some(result) => {
                cpu.set_reg(self.rd, result as u32);
                Ok(())
            }
            None => Err(InstructionException::IntegerOverflow),
        }
    }
    
    fn mnemonic(&self) -> &'static str {
        "ADD"
    }
    
    fn disassemble(&self) -> String {
        format!("ADD ${}, ${}, ${}", self.rd, self.rs, self.rt)
    }
}

/// ADDU - Add unsigned (no overflow check)
pub struct Addu {
    rd: u32,
    rs: u32,
    rt: u32,
}

impl Addu {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rd: decoded.rd,
            rs: decoded.rs,
            rt: decoded.rt,
        }
    }
}

impl Instruction for Addu {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let result = cpu.get_reg(self.rs).wrapping_add(cpu.get_reg(self.rt));
        cpu.set_reg(self.rd, result);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "ADDU"
    }
    
    fn disassemble(&self) -> String {
        format!("ADDU ${}, ${}, ${}", self.rd, self.rs, self.rt)
    }
}

/// ADDI - Add immediate with overflow check
pub struct Addi {
    rt: u32,
    rs: u32,
    immediate: u32,
}

impl Addi {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rt: decoded.rt,
            rs: decoded.rs,
            immediate: decoded.sign_extend_immediate(),
        }
    }
}

impl Instruction for Addi {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let a = cpu.get_reg(self.rs) as i32;
        let b = self.immediate as i32;
        
        match a.checked_add(b) {
            Some(result) => {
                cpu.set_reg(self.rt, result as u32);
                Ok(())
            }
            None => Err(InstructionException::IntegerOverflow),
        }
    }
    
    fn mnemonic(&self) -> &'static str {
        "ADDI"
    }
    
    fn disassemble(&self) -> String {
        format!("ADDI ${}, ${}, {:#x}", self.rt, self.rs, self.immediate)
    }
}

// ============================================================================
// Logical Instructions
// ============================================================================

/// AND - Logical AND
pub struct And {
    rd: u32,
    rs: u32,
    rt: u32,
}

impl And {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rd: decoded.rd,
            rs: decoded.rs,
            rt: decoded.rt,
        }
    }
}

impl Instruction for And {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let result = cpu.get_reg(self.rs) & cpu.get_reg(self.rt);
        cpu.set_reg(self.rd, result);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "AND"
    }
    
    fn disassemble(&self) -> String {
        format!("AND ${}, ${}, ${}", self.rd, self.rs, self.rt)
    }
}

/// OR - Logical OR
pub struct Or {
    rd: u32,
    rs: u32,
    rt: u32,
}

impl Or {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rd: decoded.rd,
            rs: decoded.rs,
            rt: decoded.rt,
        }
    }
}

impl Instruction for Or {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let result = cpu.get_reg(self.rs) | cpu.get_reg(self.rt);
        cpu.set_reg(self.rd, result);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "OR"
    }
    
    fn disassemble(&self) -> String {
        format!("OR ${}, ${}, ${}", self.rd, self.rs, self.rt)
    }
}

// ============================================================================
// Branch Instructions
// ============================================================================

/// BEQ - Branch if equal
pub struct Beq {
    rs: u32,
    rt: u32,
    offset: u32,
}

impl Beq {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rs: decoded.rs,
            rt: decoded.rt,
            offset: decoded.sign_extend_immediate() << 2,
        }
    }
}

impl Instruction for Beq {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        if cpu.get_reg(self.rs) == cpu.get_reg(self.rt) {
            let target = cpu.get_pc().wrapping_add(self.offset);
            cpu.set_next_pc(target);
        }
        cpu.set_delay_slot(true);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "BEQ"
    }
    
    fn disassemble(&self) -> String {
        format!("BEQ ${}, ${}, {:#x}", self.rs, self.rt, self.offset)
    }
}

/// BNE - Branch if not equal
pub struct Bne {
    rs: u32,
    rt: u32,
    offset: u32,
}

impl Bne {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rs: decoded.rs,
            rt: decoded.rt,
            offset: decoded.sign_extend_immediate() << 2,
        }
    }
}

impl Instruction for Bne {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        if cpu.get_reg(self.rs) != cpu.get_reg(self.rt) {
            let target = cpu.get_pc().wrapping_add(self.offset);
            cpu.set_next_pc(target);
        }
        cpu.set_delay_slot(true);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "BNE"
    }
    
    fn disassemble(&self) -> String {
        format!("BNE ${}, ${}, {:#x}", self.rs, self.rt, self.offset)
    }
}

// ============================================================================
// Jump Instructions
// ============================================================================

/// J - Jump
pub struct J {
    target: u32,
}

impl J {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            target: decoded.target,
        }
    }
}

impl Instruction for J {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let pc = cpu.get_pc();
        let target = (pc & memory_map::PC_SEGMENT_MASK) | (self.target << 2);
        cpu.set_next_pc(target);
        cpu.set_delay_slot(true);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "J"
    }
    
    fn disassemble(&self) -> String {
        format!("J {:#x}", self.target << 2)
    }
}

/// JAL - Jump and link
pub struct Jal {
    target: u32,
}

impl Jal {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            target: decoded.target,
        }
    }
}

impl Instruction for Jal {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let pc = cpu.get_pc();
        let target = (pc & memory_map::PC_SEGMENT_MASK) | (self.target << 2);
        
        // Store return address in $ra (register 31)
        cpu.set_reg(31, pc + 8);
        cpu.set_next_pc(target);
        cpu.set_delay_slot(true);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "JAL"
    }
    
    fn disassemble(&self) -> String {
        format!("JAL {:#x}", self.target << 2)
    }
}

/// JR - Jump register
pub struct Jr {
    rs: u32,
}

impl Jr {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rs: decoded.rs,
        }
    }
}

impl Instruction for Jr {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let target = cpu.get_reg(self.rs);
        
        if target & 0x3 != 0 {
            return Err(InstructionException::AddressError(target));
        }
        
        cpu.set_next_pc(target);
        cpu.set_delay_slot(true);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "JR"
    }
    
    fn disassemble(&self) -> String {
        format!("JR ${}", self.rs)
    }
}

// ============================================================================
// Load/Store Instructions
// ============================================================================

/// LW - Load word
pub struct Lw {
    rt: u32,
    rs: u32,
    offset: u32,
}

impl Lw {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rt: decoded.rt,
            rs: decoded.rs,
            offset: decoded.sign_extend_immediate(),
        }
    }
}

impl Instruction for Lw {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let addr = cpu.get_reg(self.rs).wrapping_add(self.offset);
        
        if addr & 0x3 != 0 {
            return Err(InstructionException::AddressError(addr));
        }
        
        let value = cpu.read_memory(addr, MemorySize::Word)?;
        cpu.set_reg(self.rt, value);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "LW"
    }
    
    fn disassemble(&self) -> String {
        format!("LW ${}, {:#x}(${})", self.rt, self.offset, self.rs)
    }
}

/// SW - Store word
pub struct Sw {
    rt: u32,
    rs: u32,
    offset: u32,
}

impl Sw {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rt: decoded.rt,
            rs: decoded.rs,
            offset: decoded.sign_extend_immediate(),
        }
    }
}

impl Instruction for Sw {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let addr = cpu.get_reg(self.rs).wrapping_add(self.offset);
        
        if addr & 0x3 != 0 {
            return Err(InstructionException::AddressError(addr));
        }
        
        let value = cpu.get_reg(self.rt);
        cpu.write_memory(addr, value, MemorySize::Word)?;
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "SW"
    }
    
    fn disassemble(&self) -> String {
        format!("SW ${}, {:#x}(${})", self.rt, self.offset, self.rs)
    }
}

/// LB - Load byte (sign-extended)
pub struct Lb {
    rt: u32,
    rs: u32,
    offset: u32,
}

impl Lb {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rt: decoded.rt,
            rs: decoded.rs,
            offset: decoded.sign_extend_immediate(),
        }
    }
}

impl Instruction for Lb {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let addr = cpu.get_reg(self.rs).wrapping_add(self.offset);
        let value = cpu.read_memory(addr, MemorySize::Byte)? as i8 as i32 as u32;
        cpu.set_reg(self.rt, value);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "LB"
    }
    
    fn disassemble(&self) -> String {
        format!("LB ${}, {:#x}(${})", self.rt, self.offset, self.rs)
    }
}

// ============================================================================
// Shift Instructions
// ============================================================================

/// SLL - Shift left logical
pub struct Sll {
    rd: u32,
    rt: u32,
    shamt: u32,
}

impl Sll {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rd: decoded.rd,
            rt: decoded.rt,
            shamt: decoded.shamt,
        }
    }
}

impl Instruction for Sll {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let result = cpu.get_reg(self.rt) << self.shamt;
        cpu.set_reg(self.rd, result);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "SLL"
    }
    
    fn disassemble(&self) -> String {
        if self.rd == 0 && self.rt == 0 && self.shamt == 0 {
            "NOP".to_string()
        } else {
            format!("SLL ${}, ${}, {}", self.rd, self.rt, self.shamt)
        }
    }
}

/// SRL - Shift right logical
pub struct Srl {
    rd: u32,
    rt: u32,
    shamt: u32,
}

impl Srl {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            rd: decoded.rd,
            rt: decoded.rt,
            shamt: decoded.shamt,
        }
    }
}

impl Instruction for Srl {
    fn execute(&self, cpu: &mut dyn CpuState) -> InstructionResult {
        let result = cpu.get_reg(self.rt) >> self.shamt;
        cpu.set_reg(self.rd, result);
        Ok(())
    }
    
    fn mnemonic(&self) -> &'static str {
        "SRL"
    }
    
    fn disassemble(&self) -> String {
        format!("SRL ${}, ${}, {}", self.rd, self.rt, self.shamt)
    }
}

// ============================================================================
// System Instructions
// ============================================================================

/// SYSCALL - System call
pub struct Syscall {
    code: u32,
}

impl Syscall {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            code: (decoded.raw >> 6) & 0xfffff,
        }
    }
}

impl Instruction for Syscall {
    fn execute(&self, _cpu: &mut dyn CpuState) -> InstructionResult {
        Err(InstructionException::Syscall)
    }
    
    fn mnemonic(&self) -> &'static str {
        "SYSCALL"
    }
    
    fn disassemble(&self) -> String {
        if self.code != 0 {
            format!("SYSCALL {:#x}", self.code)
        } else {
            "SYSCALL".to_string()
        }
    }
}

/// BREAK - Breakpoint
pub struct Break {
    code: u32,
}

impl Break {
    pub fn new(decoded: &DecodedInstruction) -> Self {
        Self {
            code: (decoded.raw >> 6) & 0xfffff,
        }
    }
}

impl Instruction for Break {
    fn execute(&self, _cpu: &mut dyn CpuState) -> InstructionResult {
        Err(InstructionException::Break)
    }
    
    fn mnemonic(&self) -> &'static str {
        "BREAK"
    }
    
    fn disassemble(&self) -> String {
        if self.code != 0 {
            format!("BREAK {:#x}", self.code)
        } else {
            "BREAK".to_string()
        }
    }
}

// ============================================================================
// Instruction Factory
// ============================================================================

/// Factory for creating instruction handlers from decoded instructions
pub struct InstructionFactory;

impl InstructionFactory {
    /// Create an instruction handler from a decoded instruction
    pub fn create(decoded: &DecodedInstruction) -> Box<dyn Instruction> {
        match decoded.opcode {
            0x00 => Self::create_special(decoded),
            0x02 => Box::new(J::new(decoded)),
            0x03 => Box::new(Jal::new(decoded)),
            0x04 => Box::new(Beq::new(decoded)),
            0x05 => Box::new(Bne::new(decoded)),
            0x08 => Box::new(Addi::new(decoded)),
            0x20 => Box::new(Lb::new(decoded)),
            0x23 => Box::new(Lw::new(decoded)),
            0x2b => Box::new(Sw::new(decoded)),
            _ => Box::new(UnknownInstruction::new(decoded.raw)),
        }
    }
    
    fn create_special(decoded: &DecodedInstruction) -> Box<dyn Instruction> {
        match decoded.funct {
            0x00 => Box::new(Sll::new(decoded)),
            0x02 => Box::new(Srl::new(decoded)),
            0x08 => Box::new(Jr::new(decoded)),
            0x0c => Box::new(Syscall::new(decoded)),
            0x0d => Box::new(Break::new(decoded)),
            0x20 => Box::new(Add::new(decoded)),
            0x21 => Box::new(Addu::new(decoded)),
            0x24 => Box::new(And::new(decoded)),
            0x25 => Box::new(Or::new(decoded)),
            _ => Box::new(UnknownInstruction::new(decoded.raw)),
        }
    }
}

/// Unknown/unimplemented instruction
struct UnknownInstruction {
    raw: u32,
}

impl UnknownInstruction {
    fn new(raw: u32) -> Self {
        Self { raw }
    }
}

impl Instruction for UnknownInstruction {
    fn execute(&self, _cpu: &mut dyn CpuState) -> InstructionResult {
        Err(InstructionException::ReservedInstruction)
    }
    
    fn mnemonic(&self) -> &'static str {
        "???"
    }
    
    fn disassemble(&self) -> String {
        format!("??? {:#010x}", self.raw)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    struct MockCpu {
        regs: [u32; 32],
        pc: u32,
        next_pc: u32,
        hi: u32,
        lo: u32,
        delay_slot: bool,
        memory: Vec<u8>,
    }
    
    impl MockCpu {
        fn new() -> Self {
            Self {
                regs: [0; 32],
                pc: 0,
                next_pc: 4,
                hi: 0,
                lo: 0,
                delay_slot: false,
                memory: vec![0; 0x200000],
            }
        }
    }
    
    impl CpuState for MockCpu {
        fn get_reg(&self, index: u32) -> u32 {
            if index < 32 {
                self.regs[index as usize]
            } else {
                0
            }
        }
        
        fn set_reg(&mut self, index: u32, value: u32) {
            if index < 32 && index != 0 {
                self.regs[index as usize] = value;
            }
        }
        
        fn get_pc(&self) -> u32 {
            self.pc
        }
        
        fn set_pc(&mut self, value: u32) {
            self.pc = value;
        }
        
        fn get_next_pc(&self) -> u32 {
            self.next_pc
        }
        
        fn set_next_pc(&mut self, value: u32) {
            self.next_pc = value;
        }
        
        fn read_memory(&self, addr: u32, size: MemorySize) -> Result<u32, InstructionException> {
            let addr = addr as usize;
            if addr >= self.memory.len() {
                return Err(InstructionException::BusError(addr as u32));
            }
            
            Ok(match size {
                MemorySize::Byte => self.memory[addr] as u32,
                MemorySize::HalfWord => {
                    u16::from_le_bytes([self.memory[addr], self.memory[addr + 1]]) as u32
                }
                MemorySize::Word => {
                    u32::from_le_bytes([
                        self.memory[addr],
                        self.memory[addr + 1],
                        self.memory[addr + 2],
                        self.memory[addr + 3],
                    ])
                }
            })
        }
        
        fn write_memory(&mut self, addr: u32, value: u32, size: MemorySize) -> Result<(), InstructionException> {
            let addr = addr as usize;
            if addr >= self.memory.len() {
                return Err(InstructionException::BusError(addr as u32));
            }
            
            match size {
                MemorySize::Byte => self.memory[addr] = value as u8,
                MemorySize::HalfWord => {
                    let bytes = (value as u16).to_le_bytes();
                    self.memory[addr..addr + 2].copy_from_slice(&bytes);
                }
                MemorySize::Word => {
                    let bytes = value.to_le_bytes();
                    self.memory[addr..addr + 4].copy_from_slice(&bytes);
                }
            }
            Ok(())
        }
        
        fn get_hi(&self) -> u32 {
            self.hi
        }
        
        fn set_hi(&mut self, value: u32) {
            self.hi = value;
        }
        
        fn get_lo(&self) -> u32 {
            self.lo
        }
        
        fn set_lo(&mut self, value: u32) {
            self.lo = value;
        }
        
        fn in_delay_slot(&self) -> bool {
            self.delay_slot
        }
        
        fn set_delay_slot(&mut self, value: bool) {
            self.delay_slot = value;
        }
    }
    
    #[test]
    fn test_addu_instruction() {
        let mut cpu = MockCpu::new();
        cpu.set_reg(1, 10);
        cpu.set_reg(2, 20);
        
        let decoded = DecodedInstruction {
            opcode: 0,
            rs: 1,
            rt: 2,
            rd: 3,
            shamt: 0,
            funct: 0x21,
            immediate: 0,
            target: 0,
            raw: 0,
        };
        
        let instr = Addu::new(&decoded);
        instr.execute(&mut cpu).unwrap();
        
        assert_eq!(cpu.get_reg(3), 30);
    }
    
    #[test]
    fn test_beq_taken() {
        let mut cpu = MockCpu::new();
        cpu.set_pc(0x1000);
        cpu.set_reg(1, 100);
        cpu.set_reg(2, 100);
        
        let decoded = DecodedInstruction {
            opcode: 0x04,
            rs: 1,
            rt: 2,
            rd: 0,
            shamt: 0,
            funct: 0,
            immediate: 0x0010, // Offset of 16 instructions
            target: 0,
            raw: 0,
        };
        
        let instr = Beq::new(&decoded);
        instr.execute(&mut cpu).unwrap();
        
        assert_eq!(cpu.get_next_pc(), 0x1040); // 0x1000 + (16 << 2)
        assert!(cpu.in_delay_slot());
    }
    
    #[test]
    fn test_lw_instruction() {
        let mut cpu = MockCpu::new();
        cpu.set_reg(1, 0x100);
        cpu.write_memory(0x104, 0x12345678, MemorySize::Word).unwrap();
        
        let decoded = DecodedInstruction {
            opcode: 0x23,
            rs: 1,
            rt: 2,
            rd: 0,
            shamt: 0,
            funct: 0,
            immediate: 0x0004,
            target: 0,
            raw: 0,
        };
        
        let instr = Lw::new(&decoded);
        instr.execute(&mut cpu).unwrap();
        
        assert_eq!(cpu.get_reg(2), 0x12345678);
    }
}