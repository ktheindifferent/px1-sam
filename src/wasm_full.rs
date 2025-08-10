// Full PSX WASM implementation with CPU emulation
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

// MIPS R3000A CPU Opcodes
#[derive(Debug, Clone, Copy)]
enum Opcode {
    Special = 0x00,
    Bcond = 0x01,
    J = 0x02,
    Jal = 0x03,
    Beq = 0x04,
    Bne = 0x05,
    Blez = 0x06,
    Bgtz = 0x07,
    Addi = 0x08,
    Addiu = 0x09,
    Slti = 0x0a,
    Sltiu = 0x0b,
    Andi = 0x0c,
    Ori = 0x0d,
    Xori = 0x0e,
    Lui = 0x0f,
    Cop0 = 0x10,
    Cop2 = 0x12,
    Lb = 0x20,
    Lh = 0x21,
    Lwl = 0x22,
    Lw = 0x23,
    Lbu = 0x24,
    Lhu = 0x25,
    Lwr = 0x26,
    Sb = 0x28,
    Sh = 0x29,
    Swl = 0x2a,
    Sw = 0x2b,
    Swr = 0x2e,
}

// CPU implementation
struct MipsCpu {
    pc: u32,
    next_pc: u32,
    regs: [u32; 32],
    hi: u32,
    lo: u32,
    // COP0 registers
    cop0_sr: u32,     // Status register
    cop0_cause: u32,  // Cause register
    cop0_epc: u32,    // Exception PC
    cop0_badvaddr: u32, // Bad virtual address
    // Load delay slot
    load_delay_reg: u8,
    load_delay_value: u32,
    // Branch delay
    in_delay_slot: bool,
    // Memory regions
    ram: Vec<u8>,
    bios: Vec<u8>,
}

impl MipsCpu {
    fn new() -> Self {
        let mut cpu = MipsCpu {
            pc: 0xbfc00000,  // BIOS start
            next_pc: 0xbfc00004,
            regs: [0; 32],
            hi: 0,
            lo: 0,
            cop0_sr: 0x10900000,  // Initial status register value
            cop0_cause: 0,
            cop0_epc: 0,
            cop0_badvaddr: 0,
            load_delay_reg: 0,
            load_delay_value: 0,
            in_delay_slot: false,
            ram: vec![0; 2 * 1024 * 1024],  // 2MB RAM
            bios: vec![0; 512 * 1024],      // 512KB BIOS
        };
        // R0 is always zero
        cpu.regs[0] = 0;
        cpu
    }

    fn read32(&self, addr: u32) -> u32 {
        let addr = addr & 0x1fffffff;  // Remove cache control bits
        
        if addr >= 0x1fc00000 && addr < 0x1fc80000 {
            // BIOS region
            let offset = (addr - 0x1fc00000) as usize;
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
        } else if addr < 0x00200000 {
            // Main RAM
            let offset = addr as usize;
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
        } else {
            // Hardware registers - return dummy values for now
            match addr {
                0x1f801070 => 0x00000200,  // I_STAT
                0x1f801074 => 0x00000000,  // I_MASK
                _ => 0,
            }
        }
    }

    fn write32(&mut self, addr: u32, value: u32) {
        let addr = addr & 0x1fffffff;  // Remove cache control bits
        
        if addr < 0x00200000 {
            // Main RAM
            let offset = addr as usize;
            if offset + 3 < self.ram.len() {
                self.ram[offset] = value as u8;
                self.ram[offset + 1] = (value >> 8) as u8;
                self.ram[offset + 2] = (value >> 16) as u8;
                self.ram[offset + 3] = (value >> 24) as u8;
            }
        }
        // Ignore writes to hardware registers for now
    }

    fn fetch_instruction(&mut self) -> u32 {
        let instruction = self.read32(self.pc);
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
        let imm_se = (instruction & 0xffff) as i16 as i32 as u32;  // Sign extended
        let target = instruction & 0x3ffffff;

        match opcode {
            0x00 => {
                // SPECIAL instructions
                let funct = instruction & 0x3f;
                match funct {
                    0x00 => {
                        // SLL
                        let sa = (instruction >> 6) & 0x1f;
                        if rd != 0 {
                            self.regs[rd] = self.regs[rt] << sa;
                        }
                    }
                    0x08 => {
                        // JR
                        self.next_pc = self.regs[rs];
                    }
                    0x09 => {
                        // JALR
                        let ret_addr = self.next_pc;
                        self.next_pc = self.regs[rs];
                        if rd != 0 {
                            self.regs[rd] = ret_addr;
                        }
                    }
                    0x20 => {
                        // ADD
                        if rd != 0 {
                            self.regs[rd] = self.regs[rs].wrapping_add(self.regs[rt]);
                        }
                    }
                    0x21 => {
                        // ADDU
                        if rd != 0 {
                            self.regs[rd] = self.regs[rs].wrapping_add(self.regs[rt]);
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
                    _ => {}  // Unimplemented
                }
            }
            0x02 => {
                // J
                self.next_pc = (self.pc & 0xf0000000) | (target << 2);
            }
            0x03 => {
                // JAL
                self.regs[31] = self.next_pc;  // Link register
                self.next_pc = (self.pc & 0xf0000000) | (target << 2);
            }
            0x04 => {
                // BEQ
                if self.regs[rs] == self.regs[rt] {
                    self.next_pc = self.pc.wrapping_add(imm_se << 2);
                }
            }
            0x05 => {
                // BNE
                if self.regs[rs] != self.regs[rt] {
                    self.next_pc = self.pc.wrapping_add(imm_se << 2);
                }
            }
            0x08 => {
                // ADDI
                if rt != 0 {
                    self.regs[rt] = self.regs[rs].wrapping_add(imm_se);
                }
            }
            0x09 => {
                // ADDIU
                if rt != 0 {
                    self.regs[rt] = self.regs[rs].wrapping_add(imm_se);
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
            0x0f => {
                // LUI
                if rt != 0 {
                    self.regs[rt] = (imm as u32) << 16;
                }
            }
            0x10 => {
                // COP0
                let cop_op = (instruction >> 21) & 0x1f;
                match cop_op {
                    0x00 => {
                        // MFC0
                        let cop_reg = rd;
                        if rt != 0 {
                            self.load_delay_reg = rt as u8;
                            self.load_delay_value = match cop_reg {
                                12 => self.cop0_sr,
                                13 => self.cop0_cause,
                                14 => self.cop0_epc,
                                _ => 0,
                            };
                        }
                    }
                    0x04 => {
                        // MTC0
                        let cop_reg = rd;
                        let value = self.regs[rt];
                        match cop_reg {
                            12 => self.cop0_sr = value,
                            13 => self.cop0_cause = value,
                            14 => self.cop0_epc = value,
                            _ => {}
                        }
                    }
                    _ => {}  // Other COP0 operations
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
            0x2b => {
                // SW
                let addr = self.regs[rs].wrapping_add(imm_se);
                self.write32(addr, self.regs[rt]);
            }
            _ => {}  // Unimplemented opcode
        }

        // R0 is always zero
        self.regs[0] = 0;
    }

    fn step(&mut self) {
        let instruction = self.fetch_instruction();
        self.execute_instruction(instruction);
    }
}

// Full PSX emulator with CPU
struct FullPsx {
    cpu: MipsCpu,
    vram: Vec<u16>,
    display_start_x: u16,
    display_start_y: u16,
    display_width: u16,
    display_height: u16,
    frame_count: u32,
    cycles_per_frame: u32,
}

impl FullPsx {
    fn new() -> Self {
        FullPsx {
            cpu: MipsCpu::new(),
            vram: vec![0; 1024 * 512],
            display_start_x: 0,
            display_start_y: 0,
            display_width: 320,
            display_height: 240,
            frame_count: 0,
            cycles_per_frame: 560000,  // Approximate cycles per frame at 60Hz
        }
    }
    
    fn load_bios(&mut self, bios_data: &[u8]) -> Result<(), String> {
        if bios_data.len() != 512 * 1024 {
            return Err("BIOS must be exactly 512KB".to_string());
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
        self.cpu.regs[28] = initial_gp; // GP
        self.cpu.regs[29] = initial_sp; // SP
        self.cpu.regs[30] = initial_sp; // FP
        
        Ok(())
    }
    
    fn run_frame(&mut self) -> Result<(), String> {
        self.frame_count += 1;
        
        // Execute CPU instructions for one frame
        for _ in 0..self.cycles_per_frame {
            self.cpu.step();
            
            // Check for infinite loops or crashes
            if self.cpu.pc == 0 || self.cpu.pc == 0xffffffff {
                console_error!("CPU crashed at PC {:08x}", self.cpu.pc);
                break;
            }
        }
        
        // Generate test pattern if no proper GPU rendering yet
        self.generate_test_pattern();
        
        Ok(())
    }
    
    fn generate_test_pattern(&mut self) {
        let offset = (self.frame_count * 2) as u16;
        for y in 0..self.display_height {
            for x in 0..self.display_width {
                let vram_x = (self.display_start_x + x) as usize % 1024;
                let vram_y = (self.display_start_y + y) as usize % 512;
                let idx = vram_y * 1024 + vram_x;
                
                let r = ((x + offset) & 0x1f) as u16;
                let g = ((y + offset / 2) & 0x1f) as u16;
                let b = ((self.cpu.pc as u16 >> 2) & 0x1f) as u16;  // Use PC to show CPU activity
                
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
    psx: FullPsx,
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    audio_context: Option<AudioContext>,
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

        let audio_context = AudioContext::new().ok();
        
        console_log!("Full PSX WASM Emulator with CPU initialized");
        
        Ok(PsxEmulator {
            psx: FullPsx::new(),
            canvas,
            context,
            audio_context,
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
        self.psx = FullPsx::new();
        console_log!("Emulator reset");
    }

    pub fn set_volume(&mut self, _volume: f32) {
        // Audio not implemented yet
    }

    pub fn get_debug_info(&self) -> String {
        format!("PC: {:08x}, SP: {:08x}, Frame: {}", 
                self.psx.cpu.pc, 
                self.psx.cpu.regs[29],
                self.psx.frame_count)
    }

    pub fn update_gamepad_state(&mut self, gamepad: &Gamepad) {
        self.input_state.update_gamepad(gamepad);
    }
}