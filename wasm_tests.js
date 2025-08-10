// Comprehensive test suite for PSX WASM emulator
import init, { PsxEmulator } from './wasm-pkg/rustation_wasm.js';

class PsxWasmTestSuite {
    constructor() {
        this.emulator = null;
        this.testResults = [];
        this.currentTest = null;
    }

    async initialize() {
        console.log('Initializing WASM test suite...');
        await init();
        return true;
    }

    log(message, type = 'info') {
        const timestamp = new Date().toISOString();
        const logEntry = { timestamp, message, type, test: this.currentTest };
        this.testResults.push(logEntry);
        
        const color = type === 'error' ? '\x1b[31m' : 
                     type === 'success' ? '\x1b[32m' : 
                     type === 'warning' ? '\x1b[33m' : '\x1b[36m';
        console.log(`${color}[${timestamp}] ${message}\x1b[0m`);
    }

    async runTest(name, testFn) {
        this.currentTest = name;
        this.log(`Starting test: ${name}`, 'info');
        
        try {
            await testFn();
            this.log(`✓ Test passed: ${name}`, 'success');
            return true;
        } catch (error) {
            this.log(`✗ Test failed: ${name} - ${error}`, 'error');
            return false;
        } finally {
            this.currentTest = null;
        }
    }

    // Test 1: Emulator initialization
    async testInitialization() {
        const canvas = document.createElement('canvas');
        canvas.id = 'test-canvas';
        document.body.appendChild(canvas);
        
        this.emulator = new PsxEmulator('test-canvas');
        if (!this.emulator) {
            throw new Error('Failed to create emulator instance');
        }
        
        document.body.removeChild(canvas);
        return true;
    }

    // Test 2: BIOS loading
    async testBiosLoading() {
        const canvas = document.createElement('canvas');
        canvas.id = 'test-canvas';
        document.body.appendChild(canvas);
        
        this.emulator = new PsxEmulator('test-canvas');
        
        // Create a valid 512KB BIOS
        const biosSize = 512 * 1024;
        const testBios = new Uint8Array(biosSize);
        
        // Add MIPS reset vector at 0xBFC00000
        // lui $t0, 0x1f80
        testBios[0] = 0x00;
        testBios[1] = 0x80;
        testBios[2] = 0x08;
        testBios[3] = 0x3c;
        
        // ori $t0, $t0, 0x1010
        testBios[4] = 0x10;
        testBios[5] = 0x10;
        testBios[6] = 0x08;
        testBios[7] = 0x35;
        
        // sw $zero, 0($t0)
        testBios[8] = 0x00;
        testBios[9] = 0x00;
        testBios[10] = 0x00;
        testBios[11] = 0xad;
        
        this.emulator.load_bios(testBios);
        
        document.body.removeChild(canvas);
        return true;
    }

    // Test 3: EXE loading
    async testExeLoading() {
        const canvas = document.createElement('canvas');
        canvas.id = 'test-canvas';
        document.body.appendChild(canvas);
        
        this.emulator = new PsxEmulator('test-canvas');
        
        // Load a minimal BIOS first
        const biosSize = 512 * 1024;
        const testBios = new Uint8Array(biosSize);
        this.emulator.load_bios(testBios);
        
        // Create a valid PS-X EXE
        const exeSize = 0x1000;
        const testExe = new Uint8Array(exeSize);
        
        // PS-X EXE header
        const header = new TextEncoder().encode('PS-X EXE');
        for (let i = 0; i < header.length; i++) {
            testExe[i] = header[i];
        }
        
        // Initial PC (0x80010000)
        testExe[0x10] = 0x00;
        testExe[0x11] = 0x00;
        testExe[0x12] = 0x01;
        testExe[0x13] = 0x80;
        
        // Initial GP (0x80010000)
        testExe[0x14] = 0x00;
        testExe[0x15] = 0x00;
        testExe[0x16] = 0x01;
        testExe[0x17] = 0x80;
        
        // Load address (0x80010000)
        testExe[0x18] = 0x00;
        testExe[0x19] = 0x00;
        testExe[0x1a] = 0x01;
        testExe[0x1b] = 0x80;
        
        // File size (0x100 bytes)
        testExe[0x1c] = 0x00;
        testExe[0x1d] = 0x01;
        testExe[0x1e] = 0x00;
        testExe[0x1f] = 0x00;
        
        // Initial SP (0x801fff00)
        testExe[0x30] = 0x00;
        testExe[0x31] = 0xff;
        testExe[0x32] = 0x1f;
        testExe[0x33] = 0x80;
        
        // Add test code at offset 0x800
        // NOP instruction
        testExe[0x800] = 0x00;
        testExe[0x801] = 0x00;
        testExe[0x802] = 0x00;
        testExe[0x803] = 0x00;
        
        this.emulator.load_game(testExe);
        
        document.body.removeChild(canvas);
        return true;
    }

    // Test 4: CPU instruction execution
    async testCpuExecution() {
        const canvas = document.createElement('canvas');
        canvas.id = 'test-canvas';
        document.body.appendChild(canvas);
        
        this.emulator = new PsxEmulator('test-canvas');
        
        // Load BIOS
        const biosSize = 512 * 1024;
        const testBios = new Uint8Array(biosSize);
        this.emulator.load_bios(testBios);
        
        // Create test program with various instructions
        const testExe = this.createTestProgram();
        this.emulator.load_game(testExe);
        
        // Start emulator
        this.emulator.start();
        
        // Run a few frames
        for (let i = 0; i < 5; i++) {
            this.emulator.run_frame();
        }
        
        // Check debug info
        const debugInfo = this.emulator.get_debug_info();
        this.log(`CPU state after execution: ${debugInfo}`, 'info');
        
        // Stop emulator
        this.emulator.stop();
        
        document.body.removeChild(canvas);
        return true;
    }

    // Test 5: Frame rendering
    async testFrameRendering() {
        const canvas = document.createElement('canvas');
        canvas.id = 'test-canvas';
        document.body.appendChild(canvas);
        
        this.emulator = new PsxEmulator('test-canvas');
        
        // Load BIOS and start
        const biosSize = 512 * 1024;
        const testBios = new Uint8Array(biosSize);
        this.emulator.load_bios(testBios);
        this.emulator.start();
        
        // Run frames and check framebuffer
        for (let i = 0; i < 10; i++) {
            this.emulator.run_frame();
        }
        
        const frameBuffer = this.emulator.get_frame_buffer();
        if (!frameBuffer || frameBuffer.length === 0) {
            throw new Error('Frame buffer is empty');
        }
        
        this.log(`Frame buffer size: ${frameBuffer.length} bytes`, 'info');
        
        this.emulator.stop();
        document.body.removeChild(canvas);
        return true;
    }

    // Test 6: Input handling
    async testInputHandling() {
        const canvas = document.createElement('canvas');
        canvas.id = 'test-canvas';
        document.body.appendChild(canvas);
        
        this.emulator = new PsxEmulator('test-canvas');
        
        // Simulate keyboard events
        const testEvent = new KeyboardEvent('keydown', {
            key: 'Enter',
            keyCode: 13,
            bubbles: true
        });
        
        this.emulator.handle_keyboard_event(testEvent, true);
        
        // Simulate key release
        const releaseEvent = new KeyboardEvent('keyup', {
            key: 'Enter',
            keyCode: 13,
            bubbles: true
        });
        
        this.emulator.handle_keyboard_event(releaseEvent, false);
        
        document.body.removeChild(canvas);
        return true;
    }

    // Test 7: Reset functionality
    async testReset() {
        const canvas = document.createElement('canvas');
        canvas.id = 'test-canvas';
        document.body.appendChild(canvas);
        
        this.emulator = new PsxEmulator('test-canvas');
        
        // Load and run
        const biosSize = 512 * 1024;
        const testBios = new Uint8Array(biosSize);
        this.emulator.load_bios(testBios);
        this.emulator.start();
        this.emulator.run_frame();
        
        // Reset
        this.emulator.reset();
        
        // Check state after reset
        const debugInfo = this.emulator.get_debug_info();
        if (!debugInfo.includes('PC: bfc00000')) {
            throw new Error('PC not reset to BIOS start address');
        }
        
        document.body.removeChild(canvas);
        return true;
    }

    // Test 8: Memory access patterns
    async testMemoryAccess() {
        const canvas = document.createElement('canvas');
        canvas.id = 'test-canvas';
        document.body.appendChild(canvas);
        
        this.emulator = new PsxEmulator('test-canvas');
        
        // Create program that tests different memory regions
        const testExe = this.createMemoryTestProgram();
        
        const biosSize = 512 * 1024;
        const testBios = new Uint8Array(biosSize);
        this.emulator.load_bios(testBios);
        this.emulator.load_game(testExe);
        
        this.emulator.start();
        
        // Execute memory test
        for (let i = 0; i < 20; i++) {
            this.emulator.run_frame();
        }
        
        this.emulator.stop();
        document.body.removeChild(canvas);
        return true;
    }

    // Helper: Create test program with various CPU instructions
    createTestProgram() {
        const exeSize = 0x2000;
        const exe = new Uint8Array(exeSize);
        
        // PS-X EXE header
        const header = new TextEncoder().encode('PS-X EXE');
        for (let i = 0; i < header.length; i++) {
            exe[i] = header[i];
        }
        
        // Set up header fields
        exe[0x10] = 0x00; exe[0x11] = 0x00; exe[0x12] = 0x01; exe[0x13] = 0x80; // PC
        exe[0x14] = 0x00; exe[0x15] = 0x00; exe[0x16] = 0x01; exe[0x17] = 0x80; // GP
        exe[0x18] = 0x00; exe[0x19] = 0x00; exe[0x1a] = 0x01; exe[0x1b] = 0x80; // Load addr
        exe[0x1c] = 0x00; exe[0x1d] = 0x08; exe[0x1e] = 0x00; exe[0x1f] = 0x00; // Size
        exe[0x30] = 0x00; exe[0x31] = 0xff; exe[0x32] = 0x1f; exe[0x33] = 0x80; // SP
        
        // Test program at 0x800
        let offset = 0x800;
        
        // ADDIU $t0, $zero, 0x1234
        exe[offset++] = 0x34; exe[offset++] = 0x12; exe[offset++] = 0x08; exe[offset++] = 0x24;
        
        // ADDIU $t1, $zero, 0x5678
        exe[offset++] = 0x78; exe[offset++] = 0x56; exe[offset++] = 0x09; exe[offset++] = 0x24;
        
        // ADD $t2, $t0, $t1
        exe[offset++] = 0x20; exe[offset++] = 0x50; exe[offset++] = 0x09; exe[offset++] = 0x01;
        
        // SLL $t3, $t2, 2
        exe[offset++] = 0x80; exe[offset++] = 0x58; exe[offset++] = 0x0a; exe[offset++] = 0x00;
        
        // AND $t4, $t2, $t0
        exe[offset++] = 0x24; exe[offset++] = 0x60; exe[offset++] = 0x28; exe[offset++] = 0x01;
        
        // OR $t5, $t3, $t4
        exe[offset++] = 0x25; exe[offset++] = 0x68; exe[offset++] = 0x6c; exe[offset++] = 0x01;
        
        // Infinite loop
        exe[offset++] = 0xff; exe[offset++] = 0xff; exe[offset++] = 0x00; exe[offset++] = 0x10;
        
        return exe;
    }

    // Helper: Create memory test program
    createMemoryTestProgram() {
        const exeSize = 0x2000;
        const exe = new Uint8Array(exeSize);
        
        // PS-X EXE header
        const header = new TextEncoder().encode('PS-X EXE');
        for (let i = 0; i < header.length; i++) {
            exe[i] = header[i];
        }
        
        // Set up header
        exe[0x10] = 0x00; exe[0x11] = 0x00; exe[0x12] = 0x01; exe[0x13] = 0x80; // PC
        exe[0x18] = 0x00; exe[0x19] = 0x00; exe[0x1a] = 0x01; exe[0x1b] = 0x80; // Load
        exe[0x1c] = 0x00; exe[0x1d] = 0x10; exe[0x1e] = 0x00; exe[0x1f] = 0x00; // Size
        exe[0x30] = 0x00; exe[0x31] = 0xff; exe[0x32] = 0x1f; exe[0x33] = 0x80; // SP
        
        // Memory test code at 0x800
        let offset = 0x800;
        
        // LUI $t0, 0x8000 (RAM base)
        exe[offset++] = 0x00; exe[offset++] = 0x80; exe[offset++] = 0x08; exe[offset++] = 0x3c;
        
        // LI $t1, 0xDEADBEEF
        exe[offset++] = 0xef; exe[offset++] = 0xbe; exe[offset++] = 0x09; exe[offset++] = 0x3c;
        exe[offset++] = 0xad; exe[offset++] = 0xde; exe[offset++] = 0x29; exe[offset++] = 0x35;
        
        // SW $t1, 0x1000($t0)
        exe[offset++] = 0x00; exe[offset++] = 0x10; exe[offset++] = 0x09; exe[offset++] = 0xad;
        
        // LW $t2, 0x1000($t0)
        exe[offset++] = 0x00; exe[offset++] = 0x10; exe[offset++] = 0x0a; exe[offset++] = 0x8d;
        
        // Loop
        exe[offset++] = 0xff; exe[offset++] = 0xff; exe[offset++] = 0x00; exe[offset++] = 0x10;
        
        return exe;
    }

    // Run all tests
    async runAllTests() {
        console.log('\n=== PSX WASM Emulator Test Suite ===\n');
        
        const tests = [
            ['Initialization', () => this.testInitialization()],
            ['BIOS Loading', () => this.testBiosLoading()],
            ['EXE Loading', () => this.testExeLoading()],
            ['CPU Execution', () => this.testCpuExecution()],
            ['Frame Rendering', () => this.testFrameRendering()],
            ['Input Handling', () => this.testInputHandling()],
            ['Reset Functionality', () => this.testReset()],
            ['Memory Access', () => this.testMemoryAccess()]
        ];
        
        let passed = 0;
        let failed = 0;
        
        for (const [name, testFn] of tests) {
            const result = await this.runTest(name, testFn);
            if (result) {
                passed++;
            } else {
                failed++;
            }
        }
        
        console.log('\n=== Test Results ===');
        console.log(`✓ Passed: ${passed}`);
        console.log(`✗ Failed: ${failed}`);
        console.log(`Total: ${tests.length}`);
        
        return { passed, failed, total: tests.length };
    }
}

// Export for use in Node.js or browser
if (typeof module !== 'undefined' && module.exports) {
    module.exports = PsxWasmTestSuite;
} else {
    window.PsxWasmTestSuite = PsxWasmTestSuite;
}

// Auto-run tests if executed directly
if (typeof window !== 'undefined' && window.location) {
    window.addEventListener('load', async () => {
        const suite = new PsxWasmTestSuite();
        await suite.initialize();
        await suite.runAllTests();
    });
}