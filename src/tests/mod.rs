// Comprehensive test suite for PSX emulator enhancements

#[cfg(test)]
mod error_handling_tests;

#[cfg(test)]
mod error_traits_tests;

#[cfg(test)]
mod save_state_tests;

#[cfg(test)]
mod performance_monitor_tests;

#[cfg(test)]
mod input_validation_tests;

#[cfg(test)]
mod memory_safety_tests;

#[cfg(test)]
mod run_ahead_tests;

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod wasm_minimal_tests;

#[cfg(test)]
#[cfg(feature = "discord-rpc")]
mod discord_rpc_tests;

#[cfg(test)]
mod rewind_tests;
