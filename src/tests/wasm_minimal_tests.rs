#[cfg(test)]
mod wasm_minimal_tests {
    use crate::wasm_minimal::{InputState, PsxCore};

    #[test]
    fn test_input_state_creation() {
        let input = InputState::new();
        
        // Test initial state - all keys should be unpressed
        for i in 0..256 {
            assert!(!input.is_key_pressed(i as u8));
        }
        
        // Test initial gamepad state
        for i in 0..16 {
            assert!(!input.is_button_pressed(i as u8));
        }
    }

    #[test]
    fn test_input_state_key_press() {
        let input = InputState::new();
        
        // Test key press and release
        let test_key = 65; // 'A' key
        
        input.set_key_pressed(test_key, true);
        assert!(input.is_key_pressed(test_key));
        
        input.set_key_pressed(test_key, false);
        assert!(!input.is_key_pressed(test_key));
    }

    #[test]
    fn test_input_state_button_press() {
        let input = InputState::new();
        
        // Test button press for valid buttons
        for button in 0..16 {
            input.set_button_pressed(button, true);
            assert!(input.is_button_pressed(button));
            
            input.set_button_pressed(button, false);
            assert!(!input.is_button_pressed(button));
        }
    }

    #[test]
    fn test_input_state_axes() {
        let input = InputState::new();
        
        // Test setting and getting axes values
        input.set_axis(0, 0.5);
        assert_eq!(input.get_axis(0), 0.5);
        
        input.set_axis(1, -0.75);
        assert_eq!(input.get_axis(1), -0.75);
        
        input.set_axis(2, 1.0);
        assert_eq!(input.get_axis(2), 1.0);
        
        input.set_axis(3, -1.0);
        assert_eq!(input.get_axis(3), -1.0);
    }

    #[test]
    fn test_input_state_axes_out_of_bounds() {
        let input = InputState::new();
        
        // Test out of bounds axis access
        input.set_axis(4, 0.5); // Should be no-op
        assert_eq!(input.get_axis(4), 0.0); // Should return default
        
        input.set_axis(255, 0.5); // Should be no-op
        assert_eq!(input.get_axis(255), 0.0); // Should return default
    }

    #[test]
    fn test_input_state_clear() {
        let input = InputState::new();
        
        // Set some state
        input.set_key_pressed(65, true);
        input.set_button_pressed(0, true);
        input.set_axis(0, 0.5);
        
        // Clear all state
        input.clear();
        
        // Verify everything is cleared
        assert!(!input.is_key_pressed(65));
        assert!(!input.is_button_pressed(0));
        assert_eq!(input.get_axis(0), 0.0);
    }

    #[test]
    fn test_input_state_to_psx_buttons() {
        let input = InputState::new();
        
        // Map keyboard keys to PSX buttons
        let key_mappings = vec![
            (88, 0),  // X -> Cross
            (90, 1),  // Z -> Circle  
            (83, 2),  // S -> Square
            (65, 3),  // A -> Triangle
            (81, 4),  // Q -> L1
            (87, 5),  // W -> R1
            (69, 6),  // E -> L2
            (82, 7),  // R -> R2
            (13, 8),  // Enter -> Start
            (16, 9),  // Shift -> Select
        ];
        
        for (key, expected_button) in key_mappings {
            input.set_key_pressed(key, true);
            let buttons = input.to_psx_buttons();
            assert!(buttons & (1 << expected_button) != 0);
            input.set_key_pressed(key, false);
        }
    }

    #[test]
    fn test_psx_core_creation() {
        let core = PsxCore::new();
        
        assert!(!core.bios_loaded);
        assert!(!core.game_loaded);
        assert_eq!(core.frame_count, 0);
    }

    #[test]
    fn test_psx_core_bios_validation() {
        let mut core = PsxCore::new();
        
        // Test invalid BIOS size
        let small_bios = vec![0u8; 100];
        assert!(!core.validate_bios(&small_bios));
        assert!(!core.bios_loaded);
        
        // Test valid BIOS size (512KB)
        let valid_bios = vec![0u8; 512 * 1024];
        assert!(core.validate_bios(&valid_bios));
        assert!(core.bios_loaded);
    }

    #[test]
    fn test_psx_core_exe_validation() {
        let mut core = PsxCore::new();
        
        // Test invalid EXE (too small)
        let small_exe = vec![0u8; 100];
        assert!(!core.validate_exe(&small_exe));
        assert!(!core.game_loaded);
        
        // Test invalid EXE (wrong header)
        let mut invalid_exe = vec![0u8; 0x800];
        invalid_exe[0..8].copy_from_slice(b"NOTPSEXE");
        assert!(!core.validate_exe(&invalid_exe));
        assert!(!core.game_loaded);
        
        // Test valid EXE
        let mut valid_exe = vec![0u8; 0x800];
        valid_exe[0..8].copy_from_slice(b"PS-X EXE");
        assert!(core.validate_exe(&valid_exe));
        assert!(core.game_loaded);
    }

    #[test]
    fn test_psx_core_frame_execution() {
        let mut core = PsxCore::new();
        
        // Execute a frame
        let initial_count = core.frame_count;
        core.execute_frame();
        assert_eq!(core.frame_count, initial_count + 1);
        
        // Execute multiple frames
        for _ in 0..10 {
            core.execute_frame();
        }
        assert_eq!(core.frame_count, initial_count + 11);
    }

    #[test]
    fn test_psx_core_generate_test_pattern() {
        let core = PsxCore::new();
        
        let pattern = core.generate_test_pattern();
        
        // Check size (320x240 in 15-bit color = 2 bytes per pixel)
        assert_eq!(pattern.len(), 320 * 240 * 2);
        
        // Verify pattern is not all zeros
        let non_zero = pattern.iter().any(|&b| b != 0);
        assert!(non_zero);
    }

    #[test]
    fn test_psx_core_reset() {
        let mut core = PsxCore::new();
        
        // Set some state
        core.bios_loaded = true;
        core.game_loaded = true;
        core.frame_count = 100;
        
        // Reset
        core.reset();
        
        // Verify state is cleared
        assert!(!core.bios_loaded);
        assert!(!core.game_loaded);
        assert_eq!(core.frame_count, 0);
    }

    #[test]
    fn test_convert_15bit_to_rgba() {
        use crate::wasm_minimal::convert_15bit_to_rgba;
        
        // Test black (0x0000)
        let black = vec![0x00, 0x00];
        let rgba = convert_15bit_to_rgba(&black);
        assert_eq!(rgba, vec![0, 0, 0, 255]);
        
        // Test red (0x001F)
        let red = vec![0x1F, 0x00];
        let rgba = convert_15bit_to_rgba(&red);
        assert_eq!(rgba[0], 248); // Red channel (31 * 8)
        assert_eq!(rgba[1], 0);   // Green
        assert_eq!(rgba[2], 0);   // Blue
        assert_eq!(rgba[3], 255); // Alpha
        
        // Test green (0x03E0)
        let green = vec![0xE0, 0x03];
        let rgba = convert_15bit_to_rgba(&green);
        assert_eq!(rgba[0], 0);   // Red
        assert_eq!(rgba[1], 248); // Green channel (31 * 8)
        assert_eq!(rgba[2], 0);   // Blue
        assert_eq!(rgba[3], 255); // Alpha
        
        // Test blue (0x7C00)
        let blue = vec![0x00, 0x7C];
        let rgba = convert_15bit_to_rgba(&blue);
        assert_eq!(rgba[0], 0);   // Red
        assert_eq!(rgba[1], 0);   // Green
        assert_eq!(rgba[2], 248); // Blue channel (31 * 8)
        assert_eq!(rgba[3], 255); // Alpha
    }

    #[test]
    fn test_map_keyboard_to_psx() {
        use crate::wasm_minimal::map_keyboard_to_psx;
        
        // Test arrow keys to D-pad
        assert_eq!(map_keyboard_to_psx("ArrowUp"), Some(12));
        assert_eq!(map_keyboard_to_psx("ArrowDown"), Some(14));
        assert_eq!(map_keyboard_to_psx("ArrowLeft"), Some(15));
        assert_eq!(map_keyboard_to_psx("ArrowRight"), Some(13));
        
        // Test face buttons
        assert_eq!(map_keyboard_to_psx("KeyX"), Some(0)); // Cross
        assert_eq!(map_keyboard_to_psx("KeyZ"), Some(1)); // Circle
        assert_eq!(map_keyboard_to_psx("KeyS"), Some(2)); // Square
        assert_eq!(map_keyboard_to_psx("KeyA"), Some(3)); // Triangle
        
        // Test shoulder buttons
        assert_eq!(map_keyboard_to_psx("KeyQ"), Some(4)); // L1
        assert_eq!(map_keyboard_to_psx("KeyW"), Some(5)); // R1
        assert_eq!(map_keyboard_to_psx("KeyE"), Some(6)); // L2
        assert_eq!(map_keyboard_to_psx("KeyR"), Some(7)); // R2
        
        // Test Start/Select
        assert_eq!(map_keyboard_to_psx("Enter"), Some(8));
        assert_eq!(map_keyboard_to_psx("ShiftLeft"), Some(9));
        assert_eq!(map_keyboard_to_psx("ShiftRight"), Some(9));
        
        // Test unmapped key
        assert_eq!(map_keyboard_to_psx("KeyP"), None);
    }
}