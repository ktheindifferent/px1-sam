#[cfg(test)]
mod error_traits_tests {
    use crate::error_traits::*;
    use wasm_bindgen::JsValue;

    #[test]
    fn test_memory_error_creation_and_display() {
        let error = MemoryError {
            address: 0xdeadbeef,
            kind: MemoryErrorKind::InvalidAddress,
        };
        
        assert_eq!(error.category(), ErrorCategory::Memory);
        assert!(error.is_recoverable());
        
        let display = format!("{}", error);
        assert!(display.contains("0xdeadbeef"));
        assert!(display.contains("InvalidAddress"));
    }

    #[test]
    fn test_memory_error_kinds() {
        let test_cases = vec![
            (MemoryErrorKind::InvalidAddress, true),
            (MemoryErrorKind::UnalignedAccess, true),
            (MemoryErrorKind::ReadOnly, true),
            (MemoryErrorKind::OutOfBounds, true),
            (MemoryErrorKind::SegmentationFault, false),
        ];

        for (kind, expected_recoverable) in test_cases {
            let error = MemoryError {
                address: 0x1000,
                kind,
            };
            assert_eq!(error.is_recoverable(), expected_recoverable);
        }
    }

    #[test]
    fn test_cpu_error_creation() {
        let error = CpuError {
            pc: 0x80001000,
            instruction: 0x12345678,
            kind: CpuErrorKind::InvalidInstruction,
        };

        assert_eq!(error.category(), ErrorCategory::Cpu);
        assert!(error.is_recoverable());

        let display = format!("{}", error);
        assert!(display.contains("0x80001000"));
        assert!(display.contains("0x12345678"));
        assert!(display.contains("InvalidInstruction"));
    }

    #[test]
    fn test_cpu_error_recoverability() {
        let recoverable_errors = vec![
            CpuErrorKind::InvalidInstruction,
            CpuErrorKind::UnalignedJump,
            CpuErrorKind::CoprocessorUnavailable,
            CpuErrorKind::ArithmeticOverflow,
            CpuErrorKind::DivisionByZero,
        ];

        for kind in recoverable_errors {
            let error = CpuError {
                pc: 0,
                instruction: 0,
                kind,
            };
            assert!(error.is_recoverable() || kind == CpuErrorKind::PrivilegeViolation);
        }

        // Test non-recoverable error
        let error = CpuError {
            pc: 0,
            instruction: 0,
            kind: CpuErrorKind::PrivilegeViolation,
        };
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_gpu_error() {
        let test_cases = vec![
            GpuErrorKind::InvalidCommand,
            GpuErrorKind::FifoOverflow,
            GpuErrorKind::VramOutOfBounds,
            GpuErrorKind::TextureNotFound,
            GpuErrorKind::InvalidResolution,
        ];

        for kind in test_cases {
            let error = GpuError {
                command: 0xE1000000,
                kind,
            };

            assert_eq!(error.category(), ErrorCategory::Gpu);
            assert!(error.is_recoverable());
            
            let display = format!("{}", error);
            assert!(display.contains("0xe1000000") || display.contains("0xE1000000"));
        }
    }

    #[test]
    fn test_disc_error_with_user_actions() {
        let error = DiscError {
            kind: DiscErrorKind::NoDiscInserted,
        };

        assert_eq!(error.category(), ErrorCategory::Disc);
        assert!(error.is_recoverable());
        assert_eq!(
            error.user_action(),
            Some("Please load a game disc or executable")
        );

        let error = DiscError {
            kind: DiscErrorKind::UnsupportedFormat("CHD".to_string()),
        };
        assert_eq!(
            error.user_action(),
            Some("This disc format is not supported. Try converting to BIN/CUE format")
        );

        let error = DiscError {
            kind: DiscErrorKind::ReadError("Sector 2048 corrupted".to_string()),
        };
        assert_eq!(error.user_action(), None);
    }

    #[test]
    fn test_disc_error_display() {
        let test_cases = vec![
            (
                DiscErrorKind::NoDiscInserted,
                "No disc inserted"
            ),
            (
                DiscErrorKind::InvalidFormat,
                "Invalid disc format"
            ),
            (
                DiscErrorKind::ReadError("Sector error".to_string()),
                "Disc read error: Sector error"
            ),
            (
                DiscErrorKind::SeekError,
                "Disc seek error"
            ),
            (
                DiscErrorKind::CrcMismatch,
                "Disc CRC mismatch"
            ),
            (
                DiscErrorKind::UnsupportedFormat("ISO".to_string()),
                "Unsupported disc format: ISO"
            ),
        ];

        for (kind, expected_msg) in test_cases {
            let error = DiscError { kind };
            assert_eq!(format!("{}", error), expected_msg);
        }
    }

    #[test]
    fn test_config_error() {
        let error = ConfigError {
            message: "Invalid BIOS region".to_string(),
        };

        assert_eq!(error.category(), ErrorCategory::Configuration);
        assert!(error.is_recoverable());
        assert_eq!(
            error.user_action(),
            Some("Please check your emulator configuration settings")
        );

        let display = format!("{}", error);
        assert!(display.contains("Invalid BIOS region"));
    }

    #[test]
    fn test_error_builder() {
        let error = ErrorBuilder::new(MemoryError {
            address: 0x1000,
            kind: MemoryErrorKind::InvalidAddress,
        })
        .with_context("During DMA transfer")
        .with_context("Channel 2")
        .build();

        assert_eq!(error.category(), ErrorCategory::Memory);
        assert!(error.is_recoverable());

        let display = format!("{}", error);
        assert!(display.contains("0x00001000"));
        assert!(display.contains("During DMA transfer"));
        assert!(display.contains("Channel 2"));
    }

    #[test]
    fn test_error_builder_non_recoverable() {
        let error = ErrorBuilder::new(CpuError {
            pc: 0x80000000,
            instruction: 0,
            kind: CpuErrorKind::InvalidInstruction,
        })
        .non_recoverable()
        .build();

        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_detailed_error_inheritance() {
        let base_error = GpuError {
            command: 0xE1000000,
            kind: GpuErrorKind::FifoOverflow,
        };

        let detailed = ErrorBuilder::new(base_error)
            .with_context("Frame 1234")
            .build();

        // Detailed error should inherit category from base error
        assert_eq!(detailed.category(), ErrorCategory::Gpu);
    }

    #[test]
    fn test_to_js_error() {
        let error = MemoryError {
            address: 0xbad00000,
            kind: MemoryErrorKind::OutOfBounds,
        };

        let js_error = error.to_js_error();
        // Note: Can't easily test JsValue content in unit tests, 
        // but we can verify it's created
        assert!(js_error.is_string());
    }

    #[test]
    fn test_error_category_equality() {
        assert_eq!(ErrorCategory::Memory, ErrorCategory::Memory);
        assert_ne!(ErrorCategory::Memory, ErrorCategory::Cpu);
        assert_ne!(ErrorCategory::Gpu, ErrorCategory::Audio);
    }

    #[test]
    fn test_js_result_conversion() {
        fn test_function_ok() -> Result<u32, MemoryError> {
            Ok(42)
        }

        fn test_function_err() -> Result<u32, MemoryError> {
            Err(MemoryError {
                address: 0x1234,
                kind: MemoryErrorKind::OutOfBounds,
            })
        }

        let result_ok = test_function_ok().to_js_result();
        assert!(result_ok.is_ok());
        assert_eq!(result_ok.unwrap(), 42);

        let result_err = test_function_err().to_js_result();
        assert!(result_err.is_err());
    }

    #[test]
    fn test_memory_error_macro() {
        use crate::memory_error;

        let error = memory_error!(0xdeadbeef, MemoryErrorKind::InvalidAddress);
        let built_error = error.build();
        
        assert_eq!(built_error.category(), ErrorCategory::Memory);
    }

    #[test]
    fn test_memory_error_macro_with_context() {
        use crate::memory_error;

        let error = memory_error!(
            0x1000,
            MemoryErrorKind::UnalignedAccess,
            "During instruction fetch",
            "PC alignment check"
        );
        
        let display = format!("{}", error);
        assert!(display.contains("During instruction fetch"));
        assert!(display.contains("PC alignment check"));
    }

    #[test]
    fn test_cpu_error_macro() {
        use crate::cpu_error;

        let error = cpu_error!(0x80001000, 0x12345678, CpuErrorKind::InvalidInstruction);
        let built_error = error.build();
        
        assert_eq!(built_error.category(), ErrorCategory::Cpu);
    }

    #[test]
    fn test_cpu_error_macro_with_context() {
        use crate::cpu_error;

        let error = cpu_error!(
            0x80001000,
            0x12345678,
            CpuErrorKind::DivisionByZero,
            "In arithmetic unit",
            "DIV instruction"
        );
        
        let display = format!("{}", error);
        assert!(display.contains("In arithmetic unit"));
        assert!(display.contains("DIV instruction"));
    }
}