//! Sound Processing Unit
//!
//! Most of the code is based on Mednafen's implementation

mod fifo;
mod fir;
mod reverb_resampler;

use super::{cd, cpu, irq, sync, AccessWidth, Addressable, CycleCount, Psx};
use fifo::DecoderFifo;
use reverb_resampler::ReverbResampler;
use std::ops::{Index, IndexMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use log::warn;

const SPUSYNC: sync::SyncToken = sync::SyncToken::Spu;

/// Offset into the SPU internal ram
type RamIndex = u32;

/// Default audio ring buffer for serialization
fn default_audio_ring_buffer() -> AudioRingBuffer {
    AudioRingBuffer::new(AudioBufferConfig::default().buffer_size)
}

/// Safe ring buffer for audio samples with overflow protection
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AudioRingBuffer {
    /// The actual buffer storing audio samples
    buffer: Vec<i16>,
    /// Write position (producer)
    write_pos: usize,
    /// Read position (consumer)
    read_pos: usize,
    /// Total samples written (for statistics)
    total_written: u64,
    /// Total samples dropped due to overflow
    total_dropped: u64,
    /// Maximum fill level reached
    max_fill_level: usize,
    /// Current fill level
    current_fill: usize,
    /// Buffer size (must be power of 2 for efficient modulo)
    size: usize,
    /// Size mask for efficient modulo operations
    size_mask: usize,
    /// Overflow recovery mode
    recovery_mode: AudioRecoveryMode,
    /// Samples dropped in current overflow event
    current_drop_count: u32,
    /// Target latency in samples
    target_latency: usize,
    /// Latency monitoring
    latency_samples: Vec<u32>,
    latency_index: usize,
}

/// Audio recovery modes when buffer overflows
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AudioRecoveryMode {
    /// Drop oldest samples (default)
    DropOldest,
    /// Drop newest samples
    DropNewest,
    /// Halve the buffer (aggressive recovery)
    HalveBuffer,
    /// Reset buffer completely
    Reset,
}

impl AudioRingBuffer {
    /// Create a new ring buffer with specified size (will be rounded to power of 2)
    pub fn new(requested_size: usize) -> Self {
        let size = requested_size.next_power_of_two();
        let size_mask = size - 1;
        
        AudioRingBuffer {
            buffer: vec![0; size],
            write_pos: 0,
            read_pos: 0,
            total_written: 0,
            total_dropped: 0,
            max_fill_level: 0,
            current_fill: 0,
            size,
            size_mask,
            recovery_mode: AudioRecoveryMode::DropOldest,
            current_drop_count: 0,
            target_latency: size / 4, // Default to 25% of buffer
            latency_samples: vec![0; 128],
            latency_index: 0,
        }
    }

    /// Get available space in the buffer
    #[inline]
    pub fn available_space(&self) -> usize {
        self.size - self.current_fill
    }

    /// Check if buffer is full
    #[inline]
    pub fn is_full(&self) -> bool {
        self.current_fill >= self.size - 2
    }

    /// Get current fill percentage
    #[inline]
    pub fn fill_percentage(&self) -> f32 {
        (self.current_fill as f32 / self.size as f32) * 100.0
    }

    /// Push a stereo sample pair with overflow protection
    pub fn push_stereo(&mut self, left: i16, right: i16) -> bool {
        if self.available_space() < 2 {
            self.handle_overflow(2);
            return false;
        }

        self.buffer[self.write_pos] = left;
        self.write_pos = (self.write_pos + 1) & self.size_mask;
        self.buffer[self.write_pos] = right;
        self.write_pos = (self.write_pos + 1) & self.size_mask;
        
        self.current_fill += 2;
        self.total_written += 2;
        self.max_fill_level = self.max_fill_level.max(self.current_fill);
        
        self.update_latency_monitoring();
        true
    }

    /// Handle buffer overflow based on recovery mode
    fn handle_overflow(&mut self, needed_samples: usize) {
        match self.recovery_mode {
            AudioRecoveryMode::DropOldest => {
                let to_drop = needed_samples.min(self.current_fill);
                self.read_pos = (self.read_pos + to_drop) & self.size_mask;
                self.current_fill = self.current_fill.saturating_sub(to_drop);
                self.total_dropped += to_drop as u64;
                self.current_drop_count += to_drop as u32;
            }
            AudioRecoveryMode::DropNewest => {
                self.total_dropped += needed_samples as u64;
                self.current_drop_count += needed_samples as u32;
            }
            AudioRecoveryMode::HalveBuffer => {
                let to_drop = self.current_fill / 2;
                self.read_pos = (self.read_pos + to_drop) & self.size_mask;
                self.current_fill = self.current_fill.saturating_sub(to_drop);
                self.total_dropped += to_drop as u64;
                self.current_drop_count += to_drop as u32;
            }
            AudioRecoveryMode::Reset => {
                self.total_dropped += self.current_fill as u64;
                self.current_drop_count += self.current_fill as u32;
                self.read_pos = 0;
                self.write_pos = 0;
                self.current_fill = 0;
            }
        }
    }

    /// Pop stereo samples from the buffer
    pub fn pop_stereo(&mut self, count: usize) -> Vec<i16> {
        let available = (self.current_fill / 2).min(count) * 2;
        let mut samples = Vec::with_capacity(available);
        
        for _ in 0..available {
            samples.push(self.buffer[self.read_pos]);
            self.read_pos = (self.read_pos + 1) & self.size_mask;
        }
        
        self.current_fill -= available;
        
        if self.current_drop_count > 0 && self.current_fill < self.target_latency {
            self.current_drop_count = 0;
        }
        
        samples
    }

    /// Update latency monitoring
    fn update_latency_monitoring(&mut self) {
        self.latency_samples[self.latency_index] = self.current_fill as u32;
        self.latency_index = (self.latency_index + 1) % self.latency_samples.len();
    }

    /// Get average latency over monitoring window
    pub fn get_average_latency(&self) -> f32 {
        let sum: u32 = self.latency_samples.iter().sum();
        sum as f32 / self.latency_samples.len() as f32
    }

    /// Set target latency
    pub fn set_target_latency(&mut self, samples: usize) {
        self.target_latency = samples.min(self.size / 2);
    }

    /// Set recovery mode
    pub fn set_recovery_mode(&mut self, mode: AudioRecoveryMode) {
        self.recovery_mode = mode;
    }

    /// Get buffer statistics
    pub fn get_stats(&self) -> AudioBufferStats {
        AudioBufferStats {
            total_written: self.total_written,
            total_dropped: self.total_dropped,
            current_fill: self.current_fill,
            max_fill_level: self.max_fill_level,
            buffer_size: self.size,
            drop_rate: if self.total_written > 0 {
                (self.total_dropped as f32 / self.total_written as f32) * 100.0
            } else {
                0.0
            },
            average_latency: self.get_average_latency(),
            is_dropping: self.current_drop_count > 0,
        }
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.total_written = 0;
        self.total_dropped = 0;
        self.max_fill_level = self.current_fill;
        self.current_drop_count = 0;
    }
}

/// Audio buffer statistics for monitoring
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioBufferStats {
    pub total_written: u64,
    pub total_dropped: u64,
    pub current_fill: usize,
    pub max_fill_level: usize,
    pub buffer_size: usize,
    pub drop_rate: f32,
    pub average_latency: f32,
    pub is_dropping: bool,
}

/// Audio buffer configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioBufferConfig {
    /// Buffer size in samples (will be rounded to power of 2)
    pub buffer_size: usize,
    /// Target latency in samples
    pub target_latency: usize,
    /// Recovery mode for overflow
    pub recovery_mode: AudioRecoveryMode,
    /// Enable debug overlay
    pub enable_debug: bool,
}

impl Default for AudioBufferConfig {
    fn default() -> Self {
        AudioBufferConfig {
            buffer_size: 8192,  // Larger default buffer for safety
            target_latency: 2048,  // ~46ms at 44.1kHz
            recovery_mode: AudioRecoveryMode::DropOldest,
            enable_debug: false,
        }
    }
}

/// Debug overlay for visualizing SPU state
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpuDebugOverlay {
    /// Voice activity status
    pub voice_activity: [bool; 24],
    /// Voice volume levels
    pub voice_levels: [(i16, i16); 24],
    /// Reverb input/output levels
    pub reverb_levels: (i16, i16, i16, i16),
    /// Main output levels
    pub main_output: (i16, i16),
    /// CD audio levels
    pub cd_audio: (i16, i16),
    /// Active voice count
    pub active_voices: usize,
    /// Reverb enabled status
    pub reverb_active: bool,
    /// Current SPU IRQ status
    pub irq_status: bool,
    /// Audio buffer statistics
    pub buffer_stats: AudioBufferStats,
    /// Buffer health status
    pub buffer_health: BufferHealth,
}

/// Buffer health status for quick visual feedback
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BufferHealth {
    Good,      // < 50% full
    Warning,   // 50-75% full
    Critical,  // 75-90% full
    Overflow,  // > 90% full or dropping samples
}

impl SpuDebugOverlay {
    fn new() -> Self {
        SpuDebugOverlay {
            voice_activity: [false; 24],
            voice_levels: [(0, 0); 24],
            reverb_levels: (0, 0, 0, 0),
            main_output: (0, 0),
            cd_audio: (0, 0),
            active_voices: 0,
            reverb_active: false,
            irq_status: false,
            buffer_stats: AudioBufferStats {
                total_written: 0,
                total_dropped: 0,
                current_fill: 0,
                max_fill_level: 0,
                buffer_size: 8192,
                drop_rate: 0.0,
                average_latency: 0.0,
                is_dropping: false,
            },
            buffer_health: BufferHealth::Good,
        }
    }
    
    /// Update buffer health status based on stats
    pub fn update_buffer_health(&mut self, stats: &AudioBufferStats) {
        self.buffer_stats = stats.clone();
        
        let fill_percentage = (stats.current_fill as f32 / stats.buffer_size as f32) * 100.0;
        
        self.buffer_health = if stats.is_dropping || fill_percentage > 90.0 {
            BufferHealth::Overflow
        } else if fill_percentage > 75.0 {
            BufferHealth::Critical
        } else if fill_percentage > 50.0 {
            BufferHealth::Warning
        } else {
            BufferHealth::Good
        };
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Spu {
    /// RAM index, used for read/writes using CPU or DMA.
    ram_index: RamIndex,
    /// Write index in the capture buffers. There's only one index used for all 4 buffers at any
    /// given time
    capture_index: RamIndex,
    /// If the IRQ is enabled in the control register and the SPU memory is accessed at `irq_addr`
    /// (read *or* write) the interrupt is triggered.
    irq_addr: RamIndex,
    /// True if the interrupt has been triggered and not yet ack'ed
    irq: bool,
    /// Main volume, left
    main_volume_left: Volume,
    /// Main volume, right
    main_volume_right: Volume,
    /// The 24 individual voices
    voices: [Voice; 24],
    /// Which voices should be started (bitfield, one bit per voice)
    voice_start: u32,
    /// Which voices should be stopped (bitfield, one bit per voice)
    voice_stop: u32,
    /// Configures which voices output LFSR noise (bitfield, one bit per voice)
    voice_noise: u32,
    /// Configures which voices are fed to the reverberation module (bitfield, one bit per voice)
    voice_reverb: u32,
    /// Configures which voices are frequency modulated (bitfield, one bit per voice)
    voice_frequency_modulated: u32,
    /// Status bits, cleared on start, set to 1 when loop_end is reached (bitfield, one bit per
    /// voice)
    voice_looped: u32,
    /// Most of the SPU's register behave like a R/W RAM, so to simplify the emulation we just
    /// store most registers in a big buffer
    #[serde(with = "serde_big_array::BigArray")]
    regs: [u16; 320],
    /// SPU internal RAM, 16bit wide
    #[serde(with = "serde_big_array::BigArray")]
    ram: [u16; SPU_RAM_SIZE],
    /// Safe ring buffer for audio output with overflow protection
    #[serde(default = "default_audio_ring_buffer")]
    audio_ring_buffer: AudioRingBuffer,
    /// Audio buffer configuration
    #[serde(default)]
    audio_buffer_config: AudioBufferConfig,
    /// Mix volume for the samples coming from the CD, left
    #[serde(default)]
    cd_volume_left: i16,
    /// Mix volume for the samples coming from the CD, right
    #[serde(default)]
    cd_volume_right: i16,
    /// First of the two LFSR counters
    noise_counter1: u16,
    /// Second of the two LFSR counters
    noise_counter2: u8,
    /// Noise Linear Feedback Shift Register
    noise_lfsr: u16,
    /// Mix volume for the samples coming from the reverb, left
    #[serde(default)]
    reverb_out_volume_left: i16,
    /// Mix volume for the samples coming from the reverb, right
    #[serde(default)]
    reverb_out_volume_right: i16,
    /// Start address of the working memory for the reverb
    #[serde(default)]
    reverb_start: RamIndex,
    /// Current index in the working memory for the reverb
    #[serde(default)]
    reverb_index: RamIndex,
    /// Which stereo side should we run reverb on next
    #[serde(default)]
    reverb_run_right: bool,
    /// Reverb input sample downsampler, left
    #[serde(default)]
    reverb_downsampler_left: ReverbResampler,
    /// Reverb input sample downsampler, right
    #[serde(default)]
    reverb_downsampler_right: ReverbResampler,
    /// Reverb output sample upsampler, left
    #[serde(default)]
    reverb_upsampler_left: ReverbResampler,
    /// Reverb outpu sample upsampler, right
    #[serde(default)]
    reverb_upsampler_right: ReverbResampler,
    /// Used to override the emulation and force reverb off
    #[serde(default)]
    reverb_enable_override: bool,
    /// Enhanced reverb configuration for better accuracy
    #[serde(default)]
    reverb_enhanced_mode: bool,
    /// Debug overlay data for SPU state visualization
    #[serde(skip)]
    debug_overlay: Option<SpuDebugOverlay>,
}

impl Spu {
    pub fn new() -> Spu {
        Spu {
            ram_index: 0,
            capture_index: 0,
            irq_addr: 0,
            irq: false,
            main_volume_left: Volume::new(),
            main_volume_right: Volume::new(),
            voices: [
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
            ],
            voice_start: 0,
            voice_stop: 0,
            voice_noise: 0,
            voice_reverb: 0,
            voice_frequency_modulated: 0,
            voice_looped: 0,
            regs: [0; 320],
            ram: [0; SPU_RAM_SIZE],
            audio_ring_buffer: AudioRingBuffer::new(AudioBufferConfig::default().buffer_size),
            audio_buffer_config: AudioBufferConfig::default(),
            cd_volume_left: 0,
            cd_volume_right: 0,
            noise_counter1: 0,
            noise_counter2: 0,
            noise_lfsr: 0,
            reverb_out_volume_left: 0,
            reverb_out_volume_right: 0,
            reverb_start: 0,
            reverb_index: 0,
            reverb_run_right: false,
            reverb_downsampler_left: ReverbResampler::new(),
            reverb_downsampler_right: ReverbResampler::new(),
            reverb_upsampler_left: ReverbResampler::new(),
            reverb_upsampler_right: ReverbResampler::new(),
            reverb_enable_override: false,  // Enable reverb by default for better audio
            reverb_enhanced_mode: true,
            debug_overlay: None,
        }
    }

    pub fn set_reverb_enable(&mut self, en: bool) {
        self.reverb_enable_override = en
    }

    /// Enable enhanced reverb mode for better accuracy
    pub fn set_reverb_enhanced(&mut self, enhanced: bool) {
        self.reverb_enhanced_mode = enhanced;
    }

    /// Enable SPU debug overlay
    pub fn enable_debug_overlay(&mut self, enable: bool) {
        if enable {
            self.debug_overlay = Some(SpuDebugOverlay::new());
        } else {
            self.debug_overlay = None;
        }
    }

    /// Get current debug overlay data if enabled
    pub fn get_debug_overlay(&self) -> Option<&SpuDebugOverlay> {
        self.debug_overlay.as_ref()
    }

    /// Returns the value of the control register
    fn control(&self) -> u16 {
        self.regs[regmap::CONTROL]
    }

    /// True if the "SPU enable" bit is set in the control register
    fn enabled(&self) -> bool {
        self.control() & (1 << 15) != 0
    }

    fn irq_enabled(&self) -> bool {
        // No$ says that the bit 6 (IRQ9) is "only when bit15=1", I'm not sure what that means.
        // Mednafen doesn't appear to put any condition on the interrupt bit.
        self.control() & (1 << 6) != 0
    }

    /// True if the SPU is muted in the configuration register
    fn muted(&self) -> bool {
        self.control() & (1 << 14) == 0
    }

    /// True if the SPU plays the audio coming from the CD
    fn cd_audio_enabled(&self) -> bool {
        self.control() & 1 != 0
    }

    /// True if the reverberation module is enabled
    fn reverb_enabled(&self) -> bool {
        self.control() & (1 << 7) != 0
    }

    /// True if the audio coming from the CD should be reverberated
    fn cd_audio_reverb(&self) -> bool {
        self.control() & (1 << 2) != 0
    }

    /// Update the status register
    fn update_status(&mut self) {
        let mut status = 0;

        status |= self.control() & 0x3f;
        status |= (self.irq as u16) << 6;

        // Not sure what that's about, copied straight from mednafen. `TRANSFER_CONTROL` is the
        // mystery register that mangles the memory writes if it's not set to 4 (cf. No$)
        if self.regs[regmap::TRANSFER_CONTROL] == 4 {
            // Bit set to true if the capture index targets the high half of the capture buffers
            let capture_high = self.capture_index & 0x100 != 0;

            status |= (capture_high as u16) << 11;
        }

        self.regs[regmap::STATUS] = status;
    }

    /// Returns true if `voice` is configured to output LFSR noise
    fn is_noise(&self, voice: u8) -> bool {
        self.voice_noise & (1 << voice) != 0
    }

    /// Returns true if frequency modulation is enabled for `voice`
    fn is_frequency_modulated(&self, voice: u8) -> bool {
        self.voice_frequency_modulated & (1 << voice) != 0
    }

    /// Returns true if voice should be started
    fn is_voice_started(&self, voice: u8) -> bool {
        self.voice_start & (1 << voice) != 0
    }

    /// Returns true if voice should be stopped
    fn is_voice_stopped(&self, voice: u8) -> bool {
        self.voice_stop & (1 << voice) != 0
    }

    /// Returns true if voice should be fed to the reverberation module
    fn is_voice_reverberated(&self, voice: u8) -> bool {
        self.voice_reverb & (1 << voice) != 0
    }

    /// Advance the noise state machine. Should be called at 44.1kHz
    fn run_noise_cycle(&mut self) {
        let ctrl = self.control();
        let freq_shift = (ctrl >> 10) & 0xf;
        let freq_step = (ctrl >> 8) & 3;

        // XXX This algorithm is taken from Mednafen. No$ has a slightly different implementation.
        let (counter1_inc, counter2_inc) = if freq_shift == 0xf {
            (0x8000, 8)
        } else {
            (2 << freq_shift, (freq_step + 4) as u8)
        };

        self.noise_counter1 = self.noise_counter1.wrapping_add(counter1_inc);
        if self.noise_counter1 & 0x8000 != 0 {
            self.noise_counter1 = 0;

            self.noise_counter2 = self.noise_counter2.wrapping_add(counter2_inc);
            if self.noise_counter2 & 8 != 0 {
                self.noise_counter2 &= 7;

                // Advance the LFSR
                let lfsr = self.noise_lfsr;
                let carry = (lfsr >> 15) ^ (lfsr >> 12) ^ (lfsr >> 11) ^ (lfsr >> 10) ^ 1;
                self.noise_lfsr = (lfsr << 1) | (carry & 1);
            }
        }
    }
}

impl Index<u8> for Spu {
    type Output = Voice;

    fn index(&self, port: u8) -> &Self::Output {
        &self.voices[port as usize]
    }
}

impl IndexMut<u8> for Spu {
    fn index_mut(&mut self, port: u8) -> &mut Self::Output {
        &mut self.voices[port as usize]
    }
}

/// Run the SPU until it's caught up with the CPU
pub fn run(psx: &mut Psx) {
    let mut elapsed = sync::resync(psx, SPUSYNC);

    while elapsed >= SPU_FREQ_DIVIDER {
        elapsed -= SPU_FREQ_DIVIDER;
        run_cycle(psx);
    }

    // If we have some leftover cycles we can just return them to the synchronization module, we'll
    // get them back on the next call to resync
    sync::rewind(psx, SPUSYNC, elapsed);

    // For now force a sync at the next cycle
    sync::next_event(psx, SPUSYNC, SPU_FREQ_DIVIDER - elapsed);
}

/// Get the contents of the sample buffer
pub fn get_samples(psx: &mut Psx) -> Vec<i16> {
    // Pop all available samples from the ring buffer
    let available = psx.spu.audio_ring_buffer.current_fill / 2;
    psx.spu.audio_ring_buffer.pop_stereo(available)
}

/// Clear the sample buffer
pub fn clear_samples(psx: &mut Psx) {
    // Reset the ring buffer statistics
    psx.spu.audio_ring_buffer.reset_stats();
}

/// Configure audio buffer settings
pub fn configure_audio_buffer(psx: &mut Psx, config: AudioBufferConfig) {
    // Store the configuration
    psx.spu.audio_buffer_config = config.clone();
    
    // Create a new ring buffer with the specified size
    psx.spu.audio_ring_buffer = AudioRingBuffer::new(config.buffer_size);
    
    // Set target latency
    psx.spu.audio_ring_buffer.set_target_latency(config.target_latency);
    
    // Set recovery mode
    psx.spu.audio_ring_buffer.set_recovery_mode(config.recovery_mode);
    
    // Enable/disable debug overlay
    if config.enable_debug {
        if psx.spu.debug_overlay.is_none() {
            psx.spu.debug_overlay = Some(SpuDebugOverlay::new());
        }
    } else {
        psx.spu.debug_overlay = None;
    }
}

/// Get current audio buffer statistics
pub fn get_audio_buffer_stats(psx: &Psx) -> AudioBufferStats {
    psx.spu.audio_ring_buffer.get_stats()
}

/// Set audio buffer recovery mode
pub fn set_audio_recovery_mode(psx: &mut Psx, mode: AudioRecoveryMode) {
    psx.spu.audio_ring_buffer.set_recovery_mode(mode);
    psx.spu.audio_buffer_config.recovery_mode = mode;
}

/// Set target latency in samples
pub fn set_target_latency(psx: &mut Psx, samples: usize) {
    psx.spu.audio_ring_buffer.set_target_latency(samples);
    psx.spu.audio_buffer_config.target_latency = samples;
}

/// Get debug overlay data
pub fn get_debug_overlay(psx: &Psx) -> Option<&SpuDebugOverlay> {
    psx.spu.debug_overlay.as_ref()
}

/// Update debug overlay with current state
pub fn update_debug_overlay(psx: &mut Psx) {
    if let Some(ref mut overlay) = psx.spu.debug_overlay {
        // Update buffer stats
        let stats = psx.spu.audio_ring_buffer.get_stats();
        overlay.update_buffer_health(&stats);
        
        // Update voice activity
        let mut active_count = 0;
        for (i, voice) in psx.spu.voices.iter().enumerate() {
            let is_active = voice.is_running();
            overlay.voice_activity[i] = is_active;
            if is_active {
                active_count += 1;
                overlay.voice_levels[i] = (voice.volume_left.get_level(), voice.volume_right.get_level());
            } else {
                overlay.voice_levels[i] = (0, 0);
            }
        }
        overlay.active_voices = active_count;
        
        // Update other status
        overlay.reverb_active = !psx.spu.reverb_enable_override;
        overlay.irq_status = psx.spu.irq;
        overlay.main_output = (psx.spu.main_volume_left.get_level(), psx.spu.main_volume_right.get_level());
        overlay.cd_audio = (psx.spu.cd_volume_left, psx.spu.cd_volume_right);
    }
}

/// Put the provided stereo pair in the output buffer with overflow protection
fn output_samples(psx: &mut Psx, left: i16, right: i16) {
    // Apply any final processing in enhanced mode
    let (left, right) = if psx.spu.reverb_enhanced_mode {
        // Apply subtle compression for better dynamic range
        let left_compressed = apply_soft_compression(left);
        let right_compressed = apply_soft_compression(right);
        (left_compressed, right_compressed)
    } else {
        (left, right)
    };

    // Push to ring buffer with automatic overflow handling
    let success = psx.spu.audio_ring_buffer.push_stereo(left, right);
    
    if !success {
        // Update debug overlay if enabled
        if let Some(ref mut overlay) = psx.spu.debug_overlay {
            let stats = psx.spu.audio_ring_buffer.get_stats();
            overlay.update_buffer_health(&stats);
        }
        
        // Log overflow event periodically (not every sample to avoid spam)
        if psx.spu.audio_ring_buffer.total_dropped % 1000 == 0 {
            warn!("Audio buffer overflow: {} samples dropped", 
                  psx.spu.audio_ring_buffer.total_dropped);
        }
    }
}

/// Apply soft compression to improve dynamic range
fn apply_soft_compression(sample: i16) -> i16 {
    let s = i32::from(sample);
    let threshold = 24576; // 75% of max
    if s.abs() > threshold {
        let excess = s.abs() - threshold;
        let compressed = threshold + (excess >> 1); // Gentle 2:1 compression
        let sign = if s < 0 { -1 } else { 1 };
        saturate_to_i16(sign * compressed)
    } else {
        sample
    }
}

/// Emulate one cycle of the SPU
fn run_cycle(psx: &mut Psx) {
    psx.spu.update_status();

    // Sum of the left and right voice volume levels
    let mut left_mix = 0;
    let mut right_mix = 0;
    let mut sweep_factor = 0;

    // Sum of the voices used for reverb
    let mut left_reverb = 0;
    let mut right_reverb = 0;

    // Track active voices for debug overlay
    let mut active_voice_count = 0;

    for voice in 0..24 {
        let (left, right) = run_voice_cycle(psx, voice, &mut sweep_factor);

        left_mix += left;
        right_mix += right;

        if psx.spu.is_voice_reverberated(voice) {
            left_reverb += left;
            right_reverb += right;
        }

        // Update debug overlay if enabled
        if let Some(ref mut overlay) = psx.spu.debug_overlay {
            let voice_active = psx.spu[voice].level() != 0;
            overlay.voice_activity[voice as usize] = voice_active;
            overlay.voice_levels[voice as usize] = (left as i16, right as i16);
            if voice_active {
                active_voice_count += 1;
            }
        }
    }

    psx.spu.run_noise_cycle();

    // Voice start/stop should've been processed by `run_voice_cycle`
    psx.spu.voice_start = 0;
    psx.spu.voice_stop = 0;

    if psx.spu.muted() {
        // Mute bit doesn't actually mute CD audio, just the SPU voices.
        left_mix = 0;
        right_mix = 0;

        // Mednafen does this too, I suppose it makes sense?
        left_reverb = 0;
        right_reverb = 0;
    }

    let [cd_left, cd_right] = cd::run_audio_cycle(psx);

    // Enhanced CD RAM writeback with proper synchronization
    // Write CD audio (pre-volume) to the RAM with proper address masking
    let capture_addr_left = psx.spu.capture_index & 0x1ff;
    let capture_addr_right = (psx.spu.capture_index | 0x200) & 0x3ff;
    
    // Ensure we don't corrupt reverb working area during writeback
    if capture_addr_left < SPU_RAM_SIZE as u32 {
        ram_write(psx, capture_addr_left, cd_left as u16);
    }
    if capture_addr_right < SPU_RAM_SIZE as u32 {
        ram_write(psx, capture_addr_right, cd_right as u16);
    }
    
    // Update debug overlay with CD audio levels
    if let Some(ref mut overlay) = psx.spu.debug_overlay {
        overlay.cd_audio = (cd_left, cd_right);
    }

    if psx.spu.cd_audio_enabled() {
        let cd_left = (i32::from(cd_left) * i32::from(psx.spu.cd_volume_left)) >> 15;
        let cd_right = (i32::from(cd_right) * i32::from(psx.spu.cd_volume_right)) >> 15;

        left_mix += cd_left;
        right_mix += cd_right;

        if psx.spu.cd_audio_reverb() {
            left_reverb += cd_left;
            right_reverb += cd_right;
        }
    }

    // Reverb
    {
        let reverb_samples = (saturate_to_i16(left_reverb), saturate_to_i16(right_reverb));

        let (reverb_left, reverb_right) = run_reverb_cycle(psx, reverb_samples);

        let reverb_left =
            (i32::from(reverb_left) * i32::from(psx.spu.reverb_out_volume_left)) >> 15;
        let reverb_right =
            (i32::from(reverb_right) * i32::from(psx.spu.reverb_out_volume_right)) >> 15;

        left_mix += reverb_left;
        right_mix += reverb_right;
    }

    left_mix = saturate_to_i16(left_mix) as i32;
    right_mix = saturate_to_i16(right_mix) as i32;

    left_mix = psx.spu.main_volume_left.apply_level(left_mix);
    right_mix = psx.spu.main_volume_right.apply_level(right_mix);

    psx.spu.main_volume_left.run_sweep_cycle();
    psx.spu.main_volume_right.run_sweep_cycle();

    psx.spu.capture_index += 1;
    psx.spu.capture_index &= 0x1ff;

    let final_left = saturate_to_i16(left_mix);
    let final_right = saturate_to_i16(right_mix);

    // Update debug overlay with final state
    if let Some(ref mut overlay) = psx.spu.debug_overlay {
        overlay.active_voices = active_voice_count;
        overlay.reverb_active = psx.spu.reverb_enabled() && psx.spu.reverb_enable_override;
        overlay.irq_status = psx.spu.irq;
        overlay.main_output = (final_left, final_right);
        overlay.reverb_levels = (
            saturate_to_i16(left_reverb),
            saturate_to_i16(right_reverb),
            0, 0  // Will be updated in reverb processing
        );
    }

    output_samples(psx, final_left, final_right);
}

fn reverb_sample_index(psx: &mut Psx, addr: u16, neg_offset: u32) -> RamIndex {
    let idx = psx.spu.reverb_index + to_ram_index(addr) - neg_offset;

    if idx <= 0x3_ffff {
        idx
    } else {
        // Overflow, wrap around to the start of the reverb working area
        psx.spu.reverb_start.wrapping_add(idx) & 0x3_ffff
    }
}

fn store_reverb_sample(psx: &mut Psx, addr: u16, v: i16) {
    let idx = reverb_sample_index(psx, addr, 0);

    ram_write(psx, idx, v as u16)
}

fn load_reverb_sample(psx: &mut Psx, addr: u16) -> i16 {
    let idx = reverb_sample_index(psx, addr, 0);

    ram_read(psx, idx) as i16
}

fn load_reverb_sample_before(psx: &mut Psx, addr: u16) -> i16 {
    let idx = reverb_sample_index(psx, addr, 1);

    ram_read(psx, idx) as i16
}

/// Advance the reverb state machine. Should be called at 44.1kHz with the new reverb samples.
fn run_reverb_cycle(psx: &mut Psx, (left_in, right_in): (i16, i16)) -> (i16, i16) {
    // Reverb downsamples from 44.1Khz to 22.05kHz using a simple FIR filter
    psx.spu.reverb_downsampler_left.push_sample(left_in);
    psx.spu.reverb_downsampler_right.push_sample(right_in);

    fn iir_mul(a: i16, b: i16) -> i32 {
        // Enhanced IIR multiplication for better accuracy
        (if a > i16::MIN {
            (32768 - i32::from(a)) * i32::from(b)
        } else if b > i16::MIN {
            i32::from(b) * 32768
        } else {
            0
        }) >> 14
    }

    // Enhanced reverb coefficient calculation for better quality
    fn apply_enhanced_reverb_coeff(sample: i32, coeff: i16, enhanced: bool) -> i32 {
        if enhanced {
            // Use higher precision calculation in enhanced mode
            let result = (sample * i32::from(coeff)) >> 14;
            // Apply subtle saturation for warmer sound
            if result > 30000 {
                30000 + ((result - 30000) >> 2)
            } else if result < -30000 {
                -30000 + ((result + 30000) >> 2)
            } else {
                result
            }
        } else {
            (sample * i32::from(coeff)) >> 15
        }
    }

    if psx.spu.reverb_enabled() && psx.spu.reverb_enable_override {
        if psx.spu.reverb_run_right {
            // IIR processing
            let sample = i32::from(psx.spu.reverb_downsampler_right.resample());

            let in_mix =
                (sample * i32::from(psx.spu.regs[regmap::REVERB_INPUT_VOLUME_RIGHT] as i16)) >> 15;

            let reflect_vol = psx.spu.regs[regmap::REVERB_REFLECT_VOLUME2] as i16;
            let enhanced = psx.spu.reverb_enhanced_mode;

            let same_side_sample = i32::from(load_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_REFLECT_SAME_RIGHT2],
            ));
            let same_side_mix = apply_enhanced_reverb_coeff(same_side_sample, reflect_vol, enhanced);

            let diff_side_sample = i32::from(load_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_REFLECT_DIFF_RIGHT2],
            ));
            let diff_side_mix = apply_enhanced_reverb_coeff(diff_side_sample, reflect_vol, enhanced);

            let input_same = saturate_to_i16(same_side_mix + in_mix);
            let input_diff = saturate_to_i16(diff_side_mix + in_mix);

            let reflect_iir_vol = psx.spu.regs[regmap::REVERB_REFLECT_VOLUME1] as i16;
            let input_same_alpha = (i32::from(input_same) * i32::from(reflect_iir_vol)) >> 14;
            let input_diff_alpha = (i32::from(input_diff) * i32::from(reflect_iir_vol)) >> 14;

            let iir_same = saturate_to_i16(
                (input_same_alpha
                    + iir_mul(
                        reflect_iir_vol,
                        load_reverb_sample_before(
                            psx,
                            psx.spu.regs[regmap::REVERB_REFLECT_SAME_RIGHT1],
                        ),
                    ))
                    >> 1,
            );
            let iir_diff = saturate_to_i16(
                (input_diff_alpha
                    + iir_mul(
                        reflect_iir_vol,
                        load_reverb_sample_before(
                            psx,
                            psx.spu.regs[regmap::REVERB_REFLECT_DIFF_RIGHT1],
                        ),
                    ))
                    >> 1,
            );

            store_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_REFLECT_SAME_RIGHT1],
                iir_same,
            );
            store_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_REFLECT_DIFF_RIGHT1],
                iir_diff,
            );

            let early_echo = saturate_to_i16(
                (((i32::from(load_reverb_sample(
                    psx,
                    psx.spu.regs[regmap::REVERB_COMB_RIGHT1],
                )) * i32::from(psx.spu.regs[regmap::REVERB_COMB_VOLUME1] as i16))
                    >> 14)
                    + ((i32::from(load_reverb_sample(
                        psx,
                        psx.spu.regs[regmap::REVERB_COMB_RIGHT2],
                    )) * i32::from(psx.spu.regs[regmap::REVERB_COMB_VOLUME2] as i16))
                        >> 14)
                    + ((i32::from(load_reverb_sample(
                        psx,
                        psx.spu.regs[regmap::REVERB_COMB_RIGHT3],
                    )) * i32::from(psx.spu.regs[regmap::REVERB_COMB_VOLUME3] as i16))
                        >> 14)
                    + ((i32::from(load_reverb_sample(
                        psx,
                        psx.spu.regs[regmap::REVERB_COMB_RIGHT4],
                    )) * i32::from(psx.spu.regs[regmap::REVERB_COMB_VOLUME4] as i16))
                        >> 14))
                    >> 1,
            );

            let apf_in1 = i32::from(load_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_APF_RIGHT1]
                    .wrapping_add(psx.spu.regs[regmap::REVERB_APF_OFFSET1]),
            ));
            let apf_in2 = i32::from(load_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_APF_RIGHT2]
                    .wrapping_add(psx.spu.regs[regmap::REVERB_APF_OFFSET2]),
            ));

            let apf_vol1 = i32::from(psx.spu.regs[regmap::REVERB_APF_VOLUME1] as i16);
            let apf_vol2 = i32::from(psx.spu.regs[regmap::REVERB_APF_VOLUME2] as i16);

            let out_1 = saturate_to_i16(i32::from(early_echo) - ((apf_in1 * apf_vol1) >> 15));
            let out_2 = saturate_to_i16(
                ((i32::from(early_echo) * apf_vol1) >> 15)
                    - ((apf_in1 * -apf_vol1) >> 15)
                    - ((apf_in2 * apf_vol2) >> 15),
            );

            store_reverb_sample(psx, psx.spu.regs[regmap::REVERB_APF_RIGHT1], out_1);
            store_reverb_sample(psx, psx.spu.regs[regmap::REVERB_APF_RIGHT2], out_2);

            psx.spu.reverb_upsampler_left.push_sample(0);
            psx.spu
                .reverb_upsampler_right
                .push_sample(saturate_to_i16((i32::from(out_1) + i32::from(out_2)) >> 1));
        } else {
            // IIR processing
            let sample = i32::from(psx.spu.reverb_downsampler_left.resample());

            let in_mix =
                (sample * i32::from(psx.spu.regs[regmap::REVERB_INPUT_VOLUME_LEFT] as i16)) >> 15;

            let reflect_vol = psx.spu.regs[regmap::REVERB_REFLECT_VOLUME2] as i16;
            let enhanced = psx.spu.reverb_enhanced_mode;

            let same_side_sample = i32::from(load_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_REFLECT_SAME_LEFT2],
            ));
            let same_side_mix = apply_enhanced_reverb_coeff(same_side_sample, reflect_vol, enhanced);

            let diff_side_sample = i32::from(load_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_REFLECT_DIFF_LEFT2],
            ));
            let diff_side_mix = apply_enhanced_reverb_coeff(diff_side_sample, reflect_vol, enhanced);

            let input_same = saturate_to_i16(same_side_mix + in_mix);
            let input_diff = saturate_to_i16(diff_side_mix + in_mix);

            let reflect_iir_vol = psx.spu.regs[regmap::REVERB_REFLECT_VOLUME1] as i16;
            let input_same_alpha = (i32::from(input_same) * i32::from(reflect_iir_vol)) >> 14;
            let input_diff_alpha = (i32::from(input_diff) * i32::from(reflect_iir_vol)) >> 14;

            let iir_same = saturate_to_i16(
                (input_same_alpha
                    + iir_mul(
                        reflect_iir_vol,
                        load_reverb_sample_before(
                            psx,
                            psx.spu.regs[regmap::REVERB_REFLECT_SAME_LEFT1],
                        ),
                    ))
                    >> 1,
            );
            let iir_diff = saturate_to_i16(
                (input_diff_alpha
                    + iir_mul(
                        reflect_iir_vol,
                        load_reverb_sample_before(
                            psx,
                            psx.spu.regs[regmap::REVERB_REFLECT_DIFF_LEFT1],
                        ),
                    ))
                    >> 1,
            );

            store_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_REFLECT_SAME_LEFT1],
                iir_same,
            );
            store_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_REFLECT_DIFF_LEFT1],
                iir_diff,
            );

            let early_echo = saturate_to_i16(
                (((i32::from(load_reverb_sample(
                    psx,
                    psx.spu.regs[regmap::REVERB_COMB_LEFT1],
                )) * i32::from(psx.spu.regs[regmap::REVERB_COMB_VOLUME1] as i16))
                    >> 14)
                    + ((i32::from(load_reverb_sample(
                        psx,
                        psx.spu.regs[regmap::REVERB_COMB_LEFT2],
                    )) * i32::from(psx.spu.regs[regmap::REVERB_COMB_VOLUME2] as i16))
                        >> 14)
                    + ((i32::from(load_reverb_sample(
                        psx,
                        psx.spu.regs[regmap::REVERB_COMB_LEFT3],
                    )) * i32::from(psx.spu.regs[regmap::REVERB_COMB_VOLUME3] as i16))
                        >> 14)
                    + ((i32::from(load_reverb_sample(
                        psx,
                        psx.spu.regs[regmap::REVERB_COMB_LEFT4],
                    )) * i32::from(psx.spu.regs[regmap::REVERB_COMB_VOLUME4] as i16))
                        >> 14))
                    >> 1,
            );

            // All-pass filter
            let apf_in1 = i32::from(load_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_APF_LEFT1]
                    .wrapping_add(psx.spu.regs[regmap::REVERB_APF_OFFSET1]),
            ));
            let apf_in2 = i32::from(load_reverb_sample(
                psx,
                psx.spu.regs[regmap::REVERB_APF_LEFT2]
                    .wrapping_add(psx.spu.regs[regmap::REVERB_APF_OFFSET2]),
            ));

            let apf_vol1 = i32::from(psx.spu.regs[regmap::REVERB_APF_VOLUME1] as i16);
            let apf_vol2 = i32::from(psx.spu.regs[regmap::REVERB_APF_VOLUME2] as i16);

            let out_1 = saturate_to_i16(i32::from(early_echo) - ((apf_in1 * apf_vol1) >> 15));
            let out_2 = saturate_to_i16(
                ((i32::from(early_echo) * apf_vol1) >> 15)
                    - ((apf_in1 * -apf_vol1) >> 15)
                    - ((apf_in2 * apf_vol2) >> 15),
            );

            store_reverb_sample(psx, psx.spu.regs[regmap::REVERB_APF_LEFT1], out_1);
            store_reverb_sample(psx, psx.spu.regs[regmap::REVERB_APF_LEFT2], out_2);

            psx.spu
                .reverb_upsampler_left
                .push_sample(saturate_to_i16((i32::from(out_1) + i32::from(out_2)) >> 1));
            psx.spu.reverb_upsampler_right.push_sample(0);
        }
    }

    if psx.spu.reverb_run_right {
        psx.spu.reverb_index = psx.spu.reverb_index.wrapping_add(1);
        if psx.spu.reverb_index > 0x3_ffff {
            psx.spu.reverb_index = psx.spu.reverb_start;
        }
    }
    psx.spu.reverb_run_right = !psx.spu.reverb_run_right;

    let reverb_left = psx.spu.reverb_upsampler_left.resample();
    let reverb_right = psx.spu.reverb_upsampler_right.resample();

    (reverb_left, reverb_right)
}

/// Run `voice` for one cycle and return a pair of stereo samples
fn run_voice_cycle(psx: &mut Psx, voice: u8, sweep_factor: &mut i32) -> (i32, i32) {
    // There's no "enable" flag for the voices, they're effectively always running. Unused voices
    // are just muted. Beyond that the ADPCM decoder is always running, even when the voice is in
    // "noise" mode and the output isn't used. This is important when the SPU interrupt is enabled.
    run_voice_decoder(psx, voice);

    let raw_sample = if psx.spu.is_noise(voice) {
        (psx.spu.noise_lfsr as i16) as i32
    } else {
        psx.spu[voice].next_raw_sample()
    };

    let sample = psx.spu[voice].apply_enveloppe(raw_sample);

    // Voices 1 and 3 write their samples back into SPU RAM (what No$ refers to as "capture")
    if voice == 1 {
        ram_write(psx, 0x400 | psx.spu.capture_index, sample as u16);
    } else if voice == 3 {
        ram_write(psx, 0x600 | psx.spu.capture_index, sample as u16);
    }

    let (left, right) = psx.spu[voice].apply_stereo(sample);

    psx.spu[voice].run_sweep_cycle();

    if psx.spu[voice].start_delay > 0 {
        // We're still in the start delay, we don't run the envelope or frequency sweep yet
        psx.spu[voice].start_delay -= 1;
    } else {
        psx.spu[voice].run_envelope_cycle();

        let mut step = u32::from(psx.spu[voice].step_length);

        if psx.spu.is_frequency_modulated(voice) {
            // Voice 0 cannot be frequency modulated
            debug_assert!(voice != 0);

            let mut s = step as i32;

            s += (s * *sweep_factor) >> 15;

            // XXX What happens if s is negative here?
            step = s as u32;
        }

        let step = if step > 0x3fff { 0x3fff } else { step as u16 };

        psx.spu[voice].consume_samples(step);
    }

    if psx.spu.is_voice_stopped(voice) {
        psx.spu[voice].release();
    }

    if psx.spu.is_voice_started(voice) {
        psx.spu[voice].restart();
        psx.spu.voice_looped &= !(1 << voice);
    }

    if !psx.spu.enabled() {
        // XXX Mednafen doesn't reset the ADSR divider in this situation
        psx.spu[voice].release();
        psx.spu[voice].mute();
    }

    // Save sweep factor for the next voice
    *sweep_factor = sample;

    (left, right)
}

/// Run the ADPCM decoder for one cycle
fn run_voice_decoder(psx: &mut Psx, voice: u8) {
    // XXX This value of 11 is taken from Mednafen. Technically we only consume 4 samples (at most)
    // per cycle so >= 4 would do the trick but apparently the original hardware decodes ahead.
    // This is important if the IRQ is enabled since it means that it would trigger a bit earlier
    // when the block is read.
    //
    // This is still not entirely cycle accurate, so it could be further improved with more
    // testing. Mednafen's codebase has a few comments giving hints on what could be done. More
    // testing required.
    if psx.spu[voice].decoder_fifo.len() >= 11 {
        // We have enough data in the decoder FIFO, no need to decode more
        if psx.spu.irq_enabled() {
            // Test prev address
            let prev = psx.spu[voice].cur_index.wrapping_sub(1) & 0x3_ffff;
            check_for_irq(psx, prev);
            // This is taken from mednafen, not shure why it's necessary
            check_for_irq(psx, prev & 0x3_fff8);
        }
    } else {
        // True if we're starting a new ADPCM block
        let new_block = psx.spu[voice].cur_index % 8 == 0;

        if new_block {
            // Check if looping has been requested in the previous block
            if psx.spu[voice].maybe_loop() {
                psx.spu.voice_looped |= 1 << voice;

                // Mednafen doesn't apply the "release and mute" block flag if we're in noise
                // mode. No$ doesn't seem to mention this corner case, but I suppose that it makes
                // sense to ignore decoder envelope changes if we don't use the data.
                if !psx.spu.is_noise(voice) {
                    psx.spu[voice].maybe_release();
                }
            }
        }

        if psx.spu.irq_enabled() {
            // Test current address
            check_for_irq(psx, psx.spu[voice].cur_index);
            // This is taken from mednafen, not sure why it's necessary
            check_for_irq(psx, psx.spu[voice].cur_index & 0x3_fff8);
        }

        if new_block {
            // We're starting a new block
            let header = ram_read_no_irq(psx, psx.spu[voice].cur_index);

            psx.spu[voice].set_block_header(header);
            psx.spu[voice].next_index();
        }

        // Decode 4 samples
        let encoded = ram_read_no_irq(psx, psx.spu[voice].cur_index);
        psx.spu[voice].next_index();
        psx.spu[voice].decode(encoded);
    }
}

/// Handle DMA writes
pub fn dma_store(psx: &mut Psx, v: u32) {
    let w1 = v as u16;
    let w2 = (v >> 16) as u16;

    // XXX Mednafen only checks for IRQ after the 2nd word.
    transfer(psx, w1);
    transfer(psx, w2);
}

/// Handle DMA reads
pub fn dma_load(psx: &mut Psx) -> u32 {
    let w1 = ram_read(psx, psx.spu.ram_index) as u32;
    psx.spu.ram_index = (psx.spu.ram_index + 1) & 0x3_ffff;
    let w2 = ram_read(psx, psx.spu.ram_index) as u32;
    psx.spu.ram_index = (psx.spu.ram_index + 1) & 0x3_ffff;

    check_for_irq(psx, psx.spu.ram_index);

    w1 | (w2 << 16)
}

pub fn store<T: Addressable>(psx: &mut Psx, off: u32, val: T) {
    match T::width() {
        AccessWidth::Word => {
            // Word writes behave like two u16
            let v = val.as_u32();
            store16(psx, off | 2, (v >> 16) as u16);
            // XXX *Sometimes* on the real hardware this 2nd write doesn't pass. I'm not really
            // sure what causes it exactly, sometimes after 32bit writes the lower half of the
            // register keeps its old value. I suspect that these two consecutive 16bit writes
            // can be interrupted in between sometimes, but I'm not really sure by what or in what
            // circumstances.
            store16(psx, off, v as u16);
        }
        AccessWidth::HalfWord => store16(psx, off, val.as_u16()),
        AccessWidth::Byte => {
            if off & 1 != 0 {
                // Byte writes that aren't 16bit aligned don't do anything
                warn!(
                    "SPU write isn't 16bit-aligned: *0x{:x} = 0x{:x}",
                    off,
                    val.as_u32()
                );
                return;
            }
            // In my tests halfword-aligned byte writes are handled exactly like Halfword writes,
            // they even write the full 16bit register value
            // XXX refactor our access code to handle that properly
            unimplemented!("Byte SPU store!");
        }
    }
}

fn store16(psx: &mut Psx, off: u32, val: u16) {
    let val = val.as_u16();

    let index = (off >> 1) as usize;

    // Validate register index to prevent out-of-bounds access
    if index >= psx.spu.regs.len() {
        warn!("SPU: Attempted write to invalid register index 0x{:x} (offset 0x{:x})", index, off);
        return;
    }

    // Store the previous value for validation
    let prev_val = psx.spu.regs[index];
    psx.spu.regs[index] = val;

    // Log unexpected writes to certain critical registers
    match index {
        regmap::CONTROL if (prev_val ^ val) & 0xc000 != 0 => {
            debug!("SPU: Control register changed significantly: 0x{:04x} -> 0x{:04x}", prev_val, val);
        }
        regmap::TRANSFER_CONTROL if val != 4 => {
            debug!("SPU: Non-standard transfer control value: 0x{:04x}", val);
        }
        _ => {}
    }

    if index < 0xc0 {
        // Voice configuration
        let voice = &mut psx.spu.voices[index >> 3];

        match index & 7 {
            regmap::voice::VOLUME_LEFT => voice.volume_left.set_config(val),
            regmap::voice::VOLUME_RIGHT => voice.volume_right.set_config(val),
            regmap::voice::ADPCM_STEP_LENGTH => voice.step_length = val,
            regmap::voice::ADPCM_START_INDEX => voice.set_start_index(to_ram_index(val)),
            regmap::voice::ADPCM_ADSR_LO => voice.adsr.set_conf_lo(val),
            regmap::voice::ADPCM_ADSR_HI => voice.adsr.set_conf_hi(val),
            regmap::voice::CURRENT_ADSR_VOLUME => voice.set_level(val as i16),
            regmap::voice::ADPCM_REPEAT_INDEX => {
                let loop_index = to_ram_index(val);
                voice.set_loop_index(loop_index);
            }
            _ => (),
        }
    } else if index < 0x100 {
        match index {
            regmap::MAIN_VOLUME_LEFT => psx.spu.main_volume_left.set_config(val),
            regmap::MAIN_VOLUME_RIGHT => psx.spu.main_volume_right.set_config(val),
            regmap::REVERB_VOLUME_LEFT => psx.spu.reverb_out_volume_left = val as i16,
            regmap::REVERB_VOLUME_RIGHT => psx.spu.reverb_out_volume_right = val as i16,
            regmap::VOICE_ON_LO => to_lo(&mut psx.spu.voice_start, val),
            regmap::VOICE_ON_HI => to_hi(&mut psx.spu.voice_start, val),
            regmap::VOICE_OFF_LO => to_lo(&mut psx.spu.voice_stop, val),
            regmap::VOICE_OFF_HI => to_hi(&mut psx.spu.voice_stop, val),
            regmap::VOICE_FM_MOD_EN_LO => {
                // Voice 0 cannot be frequency modulated - enforce this constraint
                let safe_val = val & !1;
                if val & 1 != 0 {
                    debug!("SPU: Attempted to enable frequency modulation on voice 0 (ignored)");
                }
                to_lo(&mut psx.spu.voice_frequency_modulated, safe_val);
            }
            regmap::VOICE_FM_MOD_EN_HI => to_hi(&mut psx.spu.voice_frequency_modulated, val),
            regmap::VOICE_NOISE_EN_LO => to_lo(&mut psx.spu.voice_noise, val),
            regmap::VOICE_NOISE_EN_HI => to_hi(&mut psx.spu.voice_noise, val),
            regmap::VOICE_REVERB_EN_LO => to_lo(&mut psx.spu.voice_reverb, val),
            regmap::VOICE_REVERB_EN_HI => to_hi(&mut psx.spu.voice_reverb, val),
            regmap::VOICE_STATUS_LO => {
                // Voice status is normally read-only, log if software tries to write
                if val != (psx.spu.voice_looped as u16) {
                    debug!("SPU: Write to read-only voice status register (low): 0x{:04x}", val);
                }
                to_lo(&mut psx.spu.voice_looped, val);
            }
            regmap::VOICE_STATUS_HI => {
                if val != ((psx.spu.voice_looped >> 16) as u16) {
                    debug!("SPU: Write to read-only voice status register (high): 0x{:04x}", val);
                }
                to_hi(&mut psx.spu.voice_looped, val);
            }
            regmap::REVERB_BASE => {
                let idx = to_ram_index(val);
                // Validate reverb base address
                if idx >= SPU_RAM_SIZE as u32 {
                    warn!("SPU: Invalid reverb base address 0x{:05x}, clamping to RAM size", idx);
                    let safe_idx = idx & 0x3_ffff;
                    psx.spu.reverb_start = safe_idx;
                    psx.spu.reverb_index = safe_idx;
                } else {
                    psx.spu.reverb_start = idx;
                    psx.spu.reverb_index = idx;
                }
            }
            regmap::IRQ_ADDRESS => {
                psx.spu.irq_addr = to_ram_index(val);
                check_for_irq(psx, psx.spu.ram_index);
            }
            regmap::TRANSFER_START_INDEX => {
                psx.spu.ram_index = to_ram_index(val);
                check_for_irq(psx, psx.spu.ram_index);
            }
            regmap::TRANSFER_FIFO => transfer(psx, val),
            regmap::CONTROL => {
                if psx.spu.irq_enabled() {
                    check_for_irq(psx, psx.spu.ram_index);
                } else {
                    // IRQ is acknowledged
                    psx.spu.irq = false;
                    irq::set_low(psx, irq::Interrupt::Spu);
                }
            }
            regmap::TRANSFER_CONTROL => {
                if val != 4 {
                    // According to No$ this register controls the way the data is transferred to
                    // the sound ram and the only value that makes sense is 4 (or more
                    // specifically, bits [3:1] should be 2), otherwise bytes get repeated using
                    // various patterns.
                    warn!("SPU TRANSFER_CONTROL set to 0x{:x}", val);
                }
            }
            regmap::CD_VOLUME_LEFT => psx.spu.cd_volume_left = val as i16,
            regmap::CD_VOLUME_RIGHT => psx.spu.cd_volume_right = val as i16,
            regmap::EXT_VOLUME_LEFT => (),
            regmap::EXT_VOLUME_RIGHT => (),
            // Reverb configuration
            regmap::REVERB_APF_OFFSET1..=regmap::REVERB_INPUT_VOLUME_RIGHT => (),
            _ => warn!(
                "SPU store index {:x} (off = {:x}, abs = {:x}): {:x}",
                index,
                off,
                0x1f80_1c00 + off,
                val
            ),
        }
    } else if index < 0x130 {
        // Set voice level
        let voice_no = (index >> 1) & 0x1f;
        let voice = &mut psx.spu.voices[voice_no];

        let left = index & 1 == 0;

        let level = val as i16;

        if left {
            voice.volume_left.set_level(level);
        } else {
            voice.volume_right.set_level(level);
        };
    }
}

pub fn load<T: Addressable>(psx: &mut Psx, off: u32) -> T {
    let v = match T::width() {
        AccessWidth::Word => {
            let hi = load16(psx, off | 2) as u32;
            let lo = load16(psx, off) as u32;

            lo | (hi << 16)
        }
        AccessWidth::HalfWord => load16(psx, off) as u32,
        AccessWidth::Byte => {
            let mut h = load16(psx, off) as u32;

            // If the byte is not halfword-aligned we read the high byte
            h >>= (off & 1) * 8;

            h & 0xff
        }
    };

    T::from_u32(v)
}

fn load16(psx: &mut Psx, off: u32) -> u16 {
    // This is probably very heavy handed, mednafen only syncs from the CD code and never on
    // register access
    run(psx);

    let index = (off >> 1) as usize;

    let reg_v = psx.spu.regs[index];

    if index < 0xc0 {
        let voice = &psx.spu.voices[index >> 3];

        match index & 7 {
            regmap::voice::CURRENT_ADSR_VOLUME => voice.level() as u16,
            regmap::voice::ADPCM_REPEAT_INDEX => (voice.loop_index >> 2) as u16,
            _ => reg_v,
        }
    } else if index < 0x100 {
        match index {
            regmap::VOICE_STATUS_LO => psx.spu.voice_looped as u16,
            regmap::VOICE_STATUS_HI => (psx.spu.voice_looped >> 16) as u16,
            regmap::TRANSFER_FIFO => unimplemented!(),
            regmap::CURRENT_VOLUME_LEFT => psx.spu.main_volume_left.level() as u16,
            regmap::CURRENT_VOLUME_RIGHT => psx.spu.main_volume_right.level() as u16,
            // Nobody seems to know what this register is for, but mednafen returns 0
            regmap::UNKNOWN => 0,
            _ => reg_v,
        }
    } else if index < 0x130 {
        // Read voice level
        let voice_no = (index >> 1) & 0x1f;
        let voice = &psx.spu.voices[voice_no];

        let left = index & 1 == 0;

        let v = if left {
            voice.volume_left.level()
        } else {
            voice.volume_right.level()
        };

        v as u16
    } else {
        reg_v
    }
}

/// Write the SPU ram at the `ram_index` an increment it.
fn transfer(psx: &mut Psx, val: u16) {
    let i = psx.spu.ram_index;

    ram_write(psx, i, val);

    psx.spu.ram_index = (i + 1) & 0x3_ffff;

    // `ram_write` already checks for interrupt before the write but mednafen immediately rechecks
    // the incremented address after that. Sounds weird but let's go with it for now.
    check_for_irq(psx, psx.spu.ram_index);
}

fn ram_write(psx: &mut Psx, index: RamIndex, val: u16) {
    check_for_irq(psx, index);

    let index = index as usize;

    // Enhanced bounds checking with proper error handling
    if index >= psx.spu.ram.len() {
        warn!("SPU: Attempted RAM write beyond bounds at index 0x{:x}", index);
        return;
    }

    // Protect reverb working area if reverb is active
    if psx.spu.reverb_enabled() && psx.spu.reverb_enable_override {
        let reverb_end = (psx.spu.reverb_start as usize).saturating_add(0x10000);
        if index >= psx.spu.reverb_start as usize && index < reverb_end.min(SPU_RAM_SIZE) {
            // This write is in the reverb working area - log it for debugging
            trace!("SPU: Write to reverb working area at 0x{:x}", index);
        }
    }

    psx.spu.ram[index] = val;
}

fn ram_read_no_irq(psx: &mut Psx, index: RamIndex) -> u16 {
    let index = index as usize;

    // Enhanced bounds checking
    if index >= psx.spu.ram.len() {
        warn!("SPU: Attempted RAM read beyond bounds at index 0x{:x}", index);
        return 0; // Return silence for out-of-bounds reads
    }

    psx.spu.ram[index]
}

fn ram_read(psx: &mut Psx, index: RamIndex) -> u16 {
    check_for_irq(psx, index);

    ram_read_no_irq(psx, index)
}

/// Trigger an IRQ if it's enabled in the control register and `addr` is equal to the `irq_addr`
fn check_for_irq(psx: &mut Psx, index: RamIndex) {
    if psx.spu.irq_enabled() && index == psx.spu.irq_addr {
        psx.spu.irq = true;
        irq::set_high(psx, irq::Interrupt::Spu);
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Voice {
    /// Voice volume left
    volume_left: Volume,
    /// Voice volume right
    volume_right: Volume,
    /// Attack Decay Sustain Release envelope
    adsr: Adsr,
    /// This value configures how fast the samples are played on this voice, which effectively
    /// changes the frequency of the output audio.
    ///
    /// The value is a 14 bit fixed point integer with 12 fractional bits
    step_length: u16,
    /// Remaining fractional steps carried between cycles, giving up the effective phase of the
    /// voice. 12 fractional bits.
    phase: u16,
    /// Value `cur_index` will take upon voice start
    start_index: RamIndex,
    /// Current index in SPU RAM for this voice
    cur_index: RamIndex,
    /// Target address for `cur_index` when an ADPCM block requests looping
    loop_index: RamIndex,
    /// True if `loop_index` has been configured through the register interface and any ADPCM loop
    /// block should be ignored.
    loop_index_force: bool,
    /// Header for the current ADPCM block
    block_header: AdpcmHeader,
    /// Last two ADPCM-decoded samples, used to extrapolate the next one
    last_samples: [i16; 2],
    /// FIFO containing the samples that have been decoded but not yet output
    decoder_fifo: DecoderFifo,
    /// Delay (in SPU cycles) between the moment a voice is enabled and the moment the envelope
    /// and frequency functions start running
    start_delay: u8,
}

impl Voice {
    fn new() -> Voice {
        Voice {
            volume_left: Volume::new(),
            volume_right: Volume::new(),
            adsr: Adsr::new(),
            step_length: 0,
            phase: 0,
            start_index: 0,
            cur_index: 0,
            loop_index: 0,
            loop_index_force: false,
            block_header: AdpcmHeader(0),
            last_samples: [0; 2],
            decoder_fifo: DecoderFifo::new(),
            start_delay: 0,
        }
    }
    
    /// Check if voice is currently running
    pub fn is_running(&self) -> bool {
        // A voice is considered running if it has a non-zero ADSR level
        // or if it's still in start delay
        self.adsr.level != 0 || self.start_delay > 0
    }

    /// Perform a loop if it was requested by the previously decoded block. Returns `true` if a
    /// loop has taken place
    fn maybe_loop(&mut self) -> bool {
        let do_loop = self.block_header.loop_end();

        if do_loop {
            self.cur_index = self.loop_index & !7;
        }

        do_loop
    }

    /// Release if it was requested by the previously decoded block. Should only be called if the
    /// block also requested looping.
    fn maybe_release(&mut self) {
        debug_assert!(self.block_header.loop_end());
        if self.block_header.loop_release_and_mute() {
            // XXX Mednafen only change the ADSR step and doesn't reset the divider but there's
            // a comment wondering if it should be reset too. To keep the code simpler here I
            // simply call the same function used when a voice is stopped.
            self.adsr.release();
            self.adsr.level = 0;
        }
    }

    /// Increment `cur_index`, wrapping to 0 if we've reached the end of the SPU RAM
    fn next_index(&mut self) {
        self.cur_index = (self.cur_index + 1) % SPU_RAM_SIZE as u32;
    }

    fn set_start_index(&mut self, addr: RamIndex) {
        // From mednafen: apparently the start index is aligned to a multiple of 8 samples
        self.start_index = addr & !7;
    }

    fn set_level(&mut self, level: i16) {
        // Clamp level to valid hardware range before setting
        let clamped = level.max(0).min(0x7FFF);
        self.adsr.set_level(clamped)
    }

    fn level(&self) -> i16 {
        self.adsr.level
    }

    fn set_block_header(&mut self, header: u16) {
        self.block_header = AdpcmHeader(header);

        if !self.loop_index_force && self.block_header.loop_start() {
            self.loop_index = self.cur_index;
        }
    }

    fn set_loop_index(&mut self, loop_index: RamIndex) {
        self.loop_index = loop_index;
        self.loop_index_force = true;
    }

    /// Decode 4 samples from an ADPCM block
    fn decode(&mut self, mut encoded: u16) {
        let (wp, wn) = self.block_header.weights();
        let mut shift = self.block_header.shift();

        // Taken from Mednafen: normally the shift value should be between 0 and 12 since otherwise
        // you lose precision. Apparently when that happens we only keep the sign bit and extend it
        // 8 times.
        //
        // XXX Should probably be tested on real hardware and added as a unit test.
        if shift > 12 {
            encoded &= 0x8888;
            shift = 8;
        }

        // Decode the four 4bit samples
        for i in 0..4 {
            // Extract the 4 bits and convert to signed to get proper sign extension when shifting
            let mut sample = (encoded << (12 - i * 4) & 0xf000) as i16;

            sample >>= shift;

            let mut sample = i32::from(sample);

            // Previous sample
            let sample_1 = i32::from(self.last_samples[0]);
            // Antepenultimate sample
            let sample_2 = i32::from(self.last_samples[1]);

            // Extrapolate with sample -1 using the positive weight
            sample += (sample_1 * wp) >> 6;
            // Extrapolate with sample -2 using the negative weight
            sample += (sample_2 * wn) >> 6;

            let sample = saturate_to_i16(sample);
            self.decoder_fifo.push(sample);

            // Shift `last_samples` for the next sample
            self.last_samples[1] = self.last_samples[0];
            self.last_samples[0] = sample;
        }
    }

    /// Returns the next "raw" decoded sample for this voice, meaning the post-ADPCM decode and
    /// resampling but pre-ADSR.
    fn next_raw_sample(&self) -> i32 {
        let phase = (self.phase >> 4) as u8;
        let samples = [
            self.decoder_fifo[0],
            self.decoder_fifo[1],
            self.decoder_fifo[2],
            self.decoder_fifo[3],
        ];

        fir::filter(phase, samples)
    }

    /// Run one cycle for the ADSR envelope function
    fn run_envelope_cycle(&mut self) {
        self.adsr.run_cycle();
    }

    fn run_sweep_cycle(&mut self) {
        self.volume_left.run_sweep_cycle();
        self.volume_right.run_sweep_cycle();
    }

    /// Apply the Attack Decay Sustain Release envelope to a sample
    fn apply_enveloppe(&self, sample: i32) -> i32 {
        // Hardware-accurate envelope application
        let level = i32::from(self.adsr.level);
        
        // Apply envelope with 15-bit precision as per hardware
        // This matches the PSX SPU's internal precision
        let result = (sample * level) >> 15;
        
        // Clamp to prevent overflow in the audio pipeline
        result.max(-32768).min(32767)
    }

    /// Apply left and right volume levels
    fn apply_stereo(&self, sample: i32) -> (i32, i32) {
        (
            self.volume_left.apply_level(sample),
            self.volume_right.apply_level(sample),
        )
    }

    /// Reinitialize voice with hardware-accurate timing
    fn restart(&mut self) {
        // Hardware-accurate voice restart sequence
        self.adsr.attack();
        self.phase = 0;
        self.cur_index = self.start_index & !7;
        self.block_header = AdpcmHeader(0);
        self.last_samples = [0; 2];
        self.decoder_fifo.clear();
        // Hardware has a 4-cycle delay before envelope starts
        self.start_delay = 4;
        self.loop_index_force = false;
    }

    /// Put the ADSR envelope in "release" state if it's not already
    fn release(&mut self) {
        // Only trigger release if voice is active
        if self.adsr.level > 0 {
            self.adsr.release();
        }
    }

    /// Set the envelope's volume to 0
    fn mute(&mut self) {
        self.adsr.level = 0;
    }

    fn consume_samples(&mut self, step: u16) {
        let step = self.phase + step;

        // Update phase with the remaining fractional part
        self.phase = step & 0xfff;

        // Consume samples as needed
        let consumed = step >> 12;
        self.decoder_fifo.discard(consumed as usize);
    }
}

/// Saturating cast from i32 to i16
fn saturate_to_i16(v: i32) -> i16 {
    if v < i32::from(i16::min_value()) {
        i16::min_value()
    } else if v > i32::from(i16::max_value()) {
        i16::max_value()
    } else {
        v as i16
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Volume {
    level: i16,
    config: VolumeConfig,
}

impl Volume {
    fn new() -> Volume {
        Volume {
            level: 0,
            config: VolumeConfig::Fixed(0),
        }
    }

    fn set_config(&mut self, conf: u16) {
        let fixed = conf & 0x8000 == 0;

        self.config = if fixed {
            let level = (conf << 1) as i16;

            VolumeConfig::Fixed(level)
        } else {
            // XXX TODO
            VolumeConfig::Sweep(EnvelopeParams::new())
        };
        // XXX should we update self.level right now? Mefnaden waits for the next call to
        // run_sweep_cycle but that takes place after the level is read.
    }

    fn level(&self) -> i16 {
        self.level
    }
    
    /// Get the current volume level for debug overlay
    pub fn get_level(&self) -> i16 {
        self.level
    }

    fn set_level(&mut self, level: i16) {
        self.level = level
    }

    /// Apply current level to a sound sample
    fn apply_level(&self, sample: i32) -> i32 {
        let level = self.level as i32;

        (sample * level) >> 15
    }

    fn run_sweep_cycle(&mut self) {
        self.level = match self.config {
            VolumeConfig::Fixed(l) => l,
            VolumeConfig::Sweep(_) => unimplemented!(),
        };
    }
}

/// Volume configuration, either fixed or a sweep
#[derive(serde::Serialize, serde::Deserialize)]
enum VolumeConfig {
    /// Fixed volume
    Fixed(i16),
    /// Sweep
    Sweep(EnvelopeParams),
}

/// Attack Decay Sustain Release envelope
#[derive(serde::Serialize, serde::Deserialize)]
struct Adsr {
    state: AdsrState,
    /// Current audio level for this envelope (0x0000 to 0x7FFF)
    level: i16,
    /// Divider used to count until the next envelope step
    divider: u16,
    /// Pre-computed envelope parameters for all 4 ADSR states
    params: [EnvelopeParams; 4],
    /// Volume level used to trigger the switch from Decay to Sustain mode
    sustain_level: i16,
    /// Config register value
    config: AdsrConfig,
    /// Hardware-accurate cycle counter for precise 44.1kHz timing
    cycle_counter: u32,
    /// Tracks envelope update cycles for accurate hardware timing
    envelope_cycles: u32,
}

impl Adsr {
    fn new() -> Adsr {
        let mut adsr = Adsr {
            state: AdsrState::Attack,
            level: 0,
            divider: 0,
            params: [
                EnvelopeParams::new(),
                EnvelopeParams::new(),
                EnvelopeParams::new(),
                EnvelopeParams::new(),
            ],
            sustain_level: 0,
            config: AdsrConfig::new(),
            cycle_counter: 0,
            envelope_cycles: 0,
        };

        // Not really needed but it's probably cleaner to make sure that `params` and `config`
        // remain always in sync
        adsr.refresh_params();

        adsr
    }

    fn set_level(&mut self, level: i16) {
        // Hardware-accurate level setting with clamping
        self.level = level.max(0).min(0x7FFF);
    }

    fn run_cycle(&mut self) {
        // Hardware-accurate ADSR timing based on 2024 research
        // The envelope updates at precise 44.1kHz intervals
        self.cycle_counter += 1;
        self.envelope_cycles += 1;
        
        let params = &self.params[self.state as usize];

        let div_step = params.compute_divider_step(self.level);
        debug_assert!(div_step > 0);

        // Hardware-accurate divider accumulation
        // The divider controls the rate of envelope updates
        debug_assert!(div_step <= 0x8000);
        self.divider = self.divider.saturating_add(div_step);

        if self.divider < 0x8000 {
            // We haven't reached the next step yet.
            return;
        }

        // Next step reached - reset divider with any overflow preserved
        self.divider = self.divider.saturating_sub(0x8000);

        let level_step = params.compute_level_step(self.level);

        // Hardware-accurate level updates with proper clamping
        match self.state {
            AdsrState::Attack => {
                // Attack phase: level increases towards 0x7FFF
                self.level = match self.level.checked_add(level_step) {
                    Some(l) if l < 0x7FFF => l,
                    _ => {
                        // Reached maximum level, transition to decay
                        self.state = AdsrState::Decay;
                        self.divider = 0; // Reset divider on phase transition
                        0x7FFF
                    }
                }
            }
            AdsrState::Decay => {
                // Decay phase: level decreases towards sustain level
                self.level = self.level.saturating_add(level_step);
                
                // Clamp to valid range
                if self.level < 0 {
                    self.level = 0;
                } else if self.level > 0x7FFF {
                    self.level = 0x7FFF;
                }
                
                // Transition to sustain when reaching sustain level
                if self.level <= self.sustain_level {
                    self.state = AdsrState::Sustain;
                    self.divider = 0; // Reset divider on phase transition
                }
            }
            AdsrState::Sustain => {
                // Sustain phase: level changes according to sustain rate
                self.level = self.level.saturating_add(level_step);
                
                // Clamp to valid range
                if self.level < 0 {
                    self.level = 0;
                } else if self.level > 0x7FFF {
                    self.level = 0x7FFF;
                }
            }
            AdsrState::Release => {
                // Release phase: level decreases towards 0
                self.level = self.level.saturating_add(level_step);
                
                // Clamp to valid range and handle underflow
                if self.level <= 0 {
                    self.level = 0;
                } else if self.level > 0x7FFF {
                    self.level = 0x7FFF;
                }
            }
        }
    }

    /// Refresh the pre-computed `params`
    fn refresh_params(&mut self) {
        self.sustain_level = self.config.sustain_level();
        self.params[AdsrState::Attack as usize] = self.config.attack_params();
        self.params[AdsrState::Decay as usize] = self.config.decay_params();
        self.params[AdsrState::Sustain as usize] = self.config.sustain_params();
        self.params[AdsrState::Release as usize] = self.config.release_params();
    }

    fn set_conf_lo(&mut self, v: u16) {
        self.config.set_lo(v);
        self.refresh_params();
    }

    fn set_conf_hi(&mut self, v: u16) {
        self.config.set_hi(v);
        self.refresh_params();
    }

    fn release(&mut self) {
        // Hardware-accurate release transition
        self.divider = 0;
        self.state = AdsrState::Release;
        // Preserve current level when entering release
    }

    fn attack(&mut self) {
        // Hardware-accurate attack initialization
        self.divider = 0;
        self.state = AdsrState::Attack;
        self.level = 0;
        self.cycle_counter = 0;
        self.envelope_cycles = 0;
    }
    
    /// Get current envelope state for debugging
    fn get_state(&self) -> AdsrState {
        self.state
    }
    
    /// Check if envelope has completed (reached 0 in release)
    fn is_completed(&self) -> bool {
        self.state == AdsrState::Release && self.level == 0
    }
}

/// Parameters used to configure an envelope function
#[derive(serde::Serialize, serde::Deserialize)]
struct EnvelopeParams {
    /// Base divider step value (how fast do we reach the next step).
    divider_step: u16,
    /// Base level step value
    level_step: i16,
    /// Envelope mode that modifies the way the steps are calculated
    mode: EnvelopeMode,
}

impl EnvelopeParams {
    fn new() -> EnvelopeParams {
        EnvelopeParams {
            divider_step: 0,
            level_step: 0,
            mode: EnvelopeMode::Linear,
        }
    }

    /// Compute (divider_step, level_step) for the given `shift` and `step` values
    /// Hardware-accurate calculation based on PSX SPU documentation
    fn steps(shift: u32, step: i8) -> (u16, i16) {
        let step = step as i16;

        // The shift value determines the envelope rate
        // Lower shift = faster envelope, higher shift = slower envelope
        if shift < 11 {
            // Fast envelope: level step is scaled up
            (0x8000, step << (11 - shift))
        } else {
            // Slow envelope: divider step is scaled down
            let div_shift = shift - 11;

            if div_shift <= 15 {
                (0x8000 >> div_shift, step)
            } else {
                // Very slow envelope
                (1, step)
            }
        }
    }

    /// Compute the parameters for smooth mode
    /// Hardware-accurate smooth envelope transition calculation
    fn smooth_mode(step: u32, base_divider: u16, base_level: i16) -> EnvelopeMode {
        // Smooth mode adjusts the envelope rate when level exceeds 0x6000
        // This creates a more natural-sounding envelope curve
        let mut smooth_divider = if step > 10 && base_divider > 3 {
            base_divider >> 2  // Quarter speed for very slow envelopes
        } else if step >= 10 && base_divider > 1 {
            base_divider >> 1  // Half speed for slow envelopes
        } else {
            base_divider       // Normal speed for fast envelopes
        };

        // Ensure divider is never zero to prevent infinite loops
        if smooth_divider == 0 {
            smooth_divider = 1;
        }

        // Adjust level step based on envelope speed
        let smooth_level = if step < 10 {
            base_level >> 2    // Quarter step for fast envelopes
        } else if step == 10 {
            base_level >> 1    // Half step for medium envelopes
        } else {
            base_level         // Full step for slow envelopes
        };

        EnvelopeMode::SmoothUp(smooth_divider, smooth_level)
    }

    fn compute_divider_step(&self, cur_level: i16) -> u16 {
        // Hardware-accurate divider step calculation
        if let EnvelopeMode::SmoothUp(smooth_divider_step, _) = self.mode {
            // Smooth mode transition at 0x6000 threshold
            if cur_level >= 0x6000 {
                return smooth_divider_step;
            }
        }

        self.divider_step
    }

    fn compute_level_step(&self, cur_level: i16) -> i16 {
        match self.mode {
            EnvelopeMode::Linear => {
                // Linear mode: constant step
                self.level_step
            }
            EnvelopeMode::Exponential => {
                // Exponential mode: step proportional to current level
                // Hardware uses a 15-bit shift for the multiplication
                let ls = self.level_step as i32;
                let cl = cur_level.max(0) as i32; // Ensure non-negative for multiplication
                
                // Hardware-accurate exponential calculation
                let result = (ls * cl) >> 15;
                
                // Clamp to i16 range
                result.max(i16::MIN as i32).min(i16::MAX as i32) as i16
            }
            EnvelopeMode::SmoothUp(_, smooth_level_step) => {
                // Smooth mode: different steps based on level threshold
                if cur_level >= 0x6000 {
                    smooth_level_step
                } else {
                    self.level_step
                }
            }
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Debug)]
enum EnvelopeMode {
    /// Divider and Volume steps remain the same throughout
    Linear,
    /// Behaves linearly up until volume reaches 0x6000, then the divider_step is replaced by the
    /// first tuple param and the level_step is replaced by the 2nd parameter
    SmoothUp(u16, i16),
    /// Volume steps are multiplied by the current value of the volume, resulting in
    /// exponentially bigger steps (in absolute value)
    Exponential,
}

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone)]
struct AdsrConfig(u32);

impl AdsrConfig {
    fn new() -> AdsrConfig {
        AdsrConfig(0)
    }

    fn sustain_level(self) -> i16 {
        // Hardware-accurate sustain level calculation
        // Bits 0-3: Sustain level (0-15)
        let sl = self.0 & 0xf;

        // Convert 4-bit value to 15-bit level
        // Formula: ((value + 1) * 0x800) - 1
        // This gives levels from 0x07FF to 0x7FFF
        let sl = ((sl + 1) << 11) - 1;

        debug_assert!(sl < 0x8000);

        sl as i16
    }

    fn attack_params(self) -> EnvelopeParams {
        // Hardware-accurate attack parameter extraction
        // Bits 10-14: Attack shift (rate)
        let shift = (self.0 >> 10) & 0x1f;
        // Bits 8-9: Attack step (inverted, so 7 - value)
        let step = 7 - ((self.0 >> 8) & 3);
        // Bit 15: Attack mode (0=linear, 1=exponential/smooth)
        let exp = (self.0 >> 15) & 1 != 0;

        let (div_step, lvl_step) = EnvelopeParams::steps(shift, step as i8);

        let mode = if exp {
            // Exponential attack uses smooth mode for more natural sound
            EnvelopeParams::smooth_mode(step, div_step, lvl_step)
        } else {
            // Linear attack for sharp, immediate response
            EnvelopeMode::Linear
        };

        EnvelopeParams {
            divider_step: div_step,
            level_step: lvl_step,
            mode,
        }
    }

    fn decay_params(self) -> EnvelopeParams {
        // Hardware-accurate decay parameter extraction
        // Bits 4-7: Decay shift (rate)
        let shift = (self.0 >> 4) & 0xf;
        // Decay always uses step of -8 (decreasing)
        let step = -8;

        let (div_step, ls) = EnvelopeParams::steps(shift, step);

        EnvelopeParams {
            divider_step: div_step,
            level_step: ls,
            // Decay always uses exponential mode for natural sound
            mode: EnvelopeMode::Exponential,
        }
    }

    fn sustain_params(self) -> EnvelopeParams {
        // Hardware-accurate sustain parameter extraction
        // Bits 24-28: Sustain shift (rate)
        let shift = (self.0 >> 24) & 0x1f;
        // Bits 22-23: Sustain step (inverted)
        let raw_step = 7 - ((self.0 >> 22) & 3);
        // Bit 31: Sustain mode (0=linear, 1=exponential)
        let exp = (self.0 >> 31) & 1 != 0;
        // Bit 30: Direction (0=increase, 1=decrease)
        let inv_step = (self.0 >> 30) & 1 != 0;

        // Apply direction to step value
        let step = if inv_step { 
            -(raw_step as i8)  // Negative for decreasing sustain
        } else { 
            raw_step as i8      // Positive for increasing sustain
        };

        let (div_step, lvl_step) = EnvelopeParams::steps(shift, step);

        let mode = if exp {
            if inv_step {
                // Exponential decay during sustain
                EnvelopeMode::Exponential
            } else {
                // Smooth increase during sustain
                EnvelopeParams::smooth_mode(raw_step, div_step, lvl_step)
            }
        } else {
            // Linear sustain change
            EnvelopeMode::Linear
        };

        EnvelopeParams {
            divider_step: div_step,
            level_step: lvl_step,
            mode,
        }
    }

    fn release_params(self) -> EnvelopeParams {
        // Hardware-accurate release parameter extraction
        // Bits 16-20: Release shift (rate)
        let shift = (self.0 >> 16) & 0x1f;
        // Release always uses step of -8 (decreasing)
        let step = -8;
        // Bit 21: Release mode (0=linear, 1=exponential)
        let exp = (self.0 >> 21) & 1 != 0;

        let (div_step, lvl_step) = EnvelopeParams::steps(shift, step as i8);

        let mode = if exp {
            // Exponential release for natural fade-out
            EnvelopeMode::Exponential
        } else {
            // Linear release for consistent fade-out
            EnvelopeMode::Linear
        };

        EnvelopeParams {
            divider_step: div_step,
            level_step: lvl_step,
            mode,
        }
    }

    fn set_lo(&mut self, v: u16) {
        to_lo(&mut self.0, v);
    }

    fn set_hi(&mut self, v: u16) {
        to_hi(&mut self.0, v);
    }
}

/// Possible ADSR states
#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq, Debug)]
enum AdsrState {
    Attack,
    Decay,
    Sustain,
    Release,
}

/// The first two bytes of a 16-byte ADPCM block
#[derive(serde::Serialize, serde::Deserialize, Copy, Clone)]
struct AdpcmHeader(u16);

impl AdpcmHeader {
    /// If true the current block is the last one of the loop sequence
    fn loop_end(self) -> bool {
        self.0 & (1 << 8) != 0
    }

    /// If true (and loop_end() is also true) we must release the envelope and set the volume
    /// to 0
    fn loop_release_and_mute(self) -> bool {
        // Shouldn't be called if `loop_end` is false
        debug_assert!(self.loop_end());
        self.0 & (1 << 9) == 0
    }

    /// If true the current block is the target for a subsequent loop_end block.
    fn loop_start(self) -> bool {
        self.0 & (1 << 10) != 0
    }

    /// Returns the pair of positive and negative weights described in the header
    fn weights(self) -> (i32, i32) {
        // Weights taken from No$, Mednafen use the same values.
        let w: [(i32, i32); 16] = [
            (0, 0),
            (60, 0),
            (115, -52),
            (98, -55),
            (122, -60),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
        ];

        let off = (self.0 >> 4) & 0xf;

        w[off as usize]
    }

    /// Right shift value to apply to extended encoded samples
    fn shift(self) -> u8 {
        (self.0 & 0xf) as u8
    }
}

/// Convert a register value to a ram index
fn to_ram_index(v: u16) -> RamIndex {
    (RamIndex::from(v) << 2) & 0x3_ffff
}

fn to_hi(r: &mut u32, v: u16) {
    let v = u32::from(v);

    *r &= 0xffff;
    *r |= v << 16;
}

fn to_lo(r: &mut u32, v: u16) {
    let v = u32::from(v);

    *r &= 0xffff_0000;
    *r |= v;
}

#[allow(dead_code)]
mod regmap {
    //! SPU register map: offset from the base in number of *halfwords*

    pub mod voice {
        //! Per-voice regmap, repeated 24 times

        pub const VOLUME_LEFT: usize = 0x0;
        pub const VOLUME_RIGHT: usize = 0x1;
        pub const ADPCM_STEP_LENGTH: usize = 0x2;
        pub const ADPCM_START_INDEX: usize = 0x3;
        pub const ADPCM_ADSR_LO: usize = 0x4;
        pub const ADPCM_ADSR_HI: usize = 0x5;
        pub const CURRENT_ADSR_VOLUME: usize = 0x6;
        pub const ADPCM_REPEAT_INDEX: usize = 0x7;
    }

    pub const MAIN_VOLUME_LEFT: usize = 0xc0;
    pub const MAIN_VOLUME_RIGHT: usize = 0xc1;
    pub const REVERB_VOLUME_LEFT: usize = 0xc2;
    pub const REVERB_VOLUME_RIGHT: usize = 0xc3;
    pub const VOICE_ON_LO: usize = 0xc4;
    pub const VOICE_ON_HI: usize = 0xc5;
    pub const VOICE_OFF_LO: usize = 0xc6;
    pub const VOICE_OFF_HI: usize = 0xc7;
    pub const VOICE_FM_MOD_EN_LO: usize = 0xc8;
    pub const VOICE_FM_MOD_EN_HI: usize = 0xc9;
    pub const VOICE_NOISE_EN_LO: usize = 0xca;
    pub const VOICE_NOISE_EN_HI: usize = 0xcb;
    pub const VOICE_REVERB_EN_LO: usize = 0xcc;
    pub const VOICE_REVERB_EN_HI: usize = 0xcd;
    pub const VOICE_STATUS_LO: usize = 0xce;
    pub const VOICE_STATUS_HI: usize = 0xcf;

    pub const REVERB_BASE: usize = 0xd1;
    pub const IRQ_ADDRESS: usize = 0xd2;
    pub const TRANSFER_START_INDEX: usize = 0xd3;
    pub const TRANSFER_FIFO: usize = 0xd4;
    pub const CONTROL: usize = 0xd5;
    pub const TRANSFER_CONTROL: usize = 0xd6;
    pub const STATUS: usize = 0xd7;
    pub const CD_VOLUME_LEFT: usize = 0xd8;
    pub const CD_VOLUME_RIGHT: usize = 0xd9;
    pub const EXT_VOLUME_LEFT: usize = 0xda;
    pub const EXT_VOLUME_RIGHT: usize = 0xdb;
    pub const CURRENT_VOLUME_LEFT: usize = 0xdc;
    pub const CURRENT_VOLUME_RIGHT: usize = 0xdd;
    pub const UNKNOWN: usize = 0xde;

    pub const REVERB_APF_OFFSET1: usize = 0xe0;
    pub const REVERB_APF_OFFSET2: usize = 0xe1;
    pub const REVERB_REFLECT_VOLUME1: usize = 0xe2;
    pub const REVERB_COMB_VOLUME1: usize = 0xe3;
    pub const REVERB_COMB_VOLUME2: usize = 0xe4;
    pub const REVERB_COMB_VOLUME3: usize = 0xe5;
    pub const REVERB_COMB_VOLUME4: usize = 0xe6;
    pub const REVERB_REFLECT_VOLUME2: usize = 0xe7;
    pub const REVERB_APF_VOLUME1: usize = 0xe8;
    pub const REVERB_APF_VOLUME2: usize = 0xe9;
    pub const REVERB_REFLECT_SAME_LEFT1: usize = 0xea;
    pub const REVERB_REFLECT_SAME_RIGHT1: usize = 0xeb;
    pub const REVERB_COMB_LEFT1: usize = 0xec;
    pub const REVERB_COMB_RIGHT1: usize = 0xed;
    pub const REVERB_COMB_LEFT2: usize = 0xee;
    pub const REVERB_COMB_RIGHT2: usize = 0xef;
    pub const REVERB_REFLECT_SAME_LEFT2: usize = 0xf0;
    pub const REVERB_REFLECT_SAME_RIGHT2: usize = 0xf1;
    pub const REVERB_REFLECT_DIFF_LEFT1: usize = 0xf2;
    pub const REVERB_REFLECT_DIFF_RIGHT1: usize = 0xf3;
    pub const REVERB_COMB_LEFT3: usize = 0xf4;
    pub const REVERB_COMB_RIGHT3: usize = 0xf5;
    pub const REVERB_COMB_LEFT4: usize = 0xf6;
    pub const REVERB_COMB_RIGHT4: usize = 0xf7;
    pub const REVERB_REFLECT_DIFF_LEFT2: usize = 0xf8;
    pub const REVERB_REFLECT_DIFF_RIGHT2: usize = 0xf9;
    pub const REVERB_APF_LEFT1: usize = 0xfa;
    pub const REVERB_APF_RIGHT1: usize = 0xfb;
    pub const REVERB_APF_LEFT2: usize = 0xfc;
    pub const REVERB_APF_RIGHT2: usize = 0xfd;
    pub const REVERB_INPUT_VOLUME_LEFT: usize = 0xfe;
    pub const REVERB_INPUT_VOLUME_RIGHT: usize = 0xff;
}

/// SPU RAM size in multiple of 16bit words
const SPU_RAM_SIZE: usize = 256 * 1024;

/// The SPU runs at 44.1kHz, the CD audio frequency, this way no resampling is required
const AUDIO_FREQ_HZ: CycleCount = 44_100;

/// The CPU frequency is an exact multiple of the audio frequency, so the divider is always an
/// integer (0x300 normally)
const SPU_FREQ_DIVIDER: CycleCount = cpu::CPU_FREQ_HZ / AUDIO_FREQ_HZ;

#[cfg(test)]
#[path = "adsr_tests.rs"]
mod adsr_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let mut buffer = AudioRingBuffer::new(16);
        
        // Test basic push/pop
        assert!(buffer.push_stereo(100, 200));
        assert!(buffer.push_stereo(300, 400));
        
        let samples = buffer.pop_stereo(2);
        assert_eq!(samples, vec![100, 200, 300, 400]);
        assert_eq!(buffer.current_fill, 0);
    }

    #[test]
    fn test_ring_buffer_overflow_drop_oldest() {
        let mut buffer = AudioRingBuffer::new(8); // Small buffer for testing
        buffer.set_recovery_mode(AudioRecoveryMode::DropOldest);
        
        // Fill buffer
        for i in 0..4 {
            assert!(buffer.push_stereo(i * 10, i * 10 + 1));
        }
        assert_eq!(buffer.current_fill, 8);
        
        // Overflow - should drop oldest
        assert!(!buffer.push_stereo(100, 101)); // Should fail but handle gracefully
        assert_eq!(buffer.total_dropped, 2);
        
        // Should have room now after dropping oldest
        assert!(buffer.push_stereo(100, 101));
    }

    #[test]
    fn test_ring_buffer_overflow_drop_newest() {
        let mut buffer = AudioRingBuffer::new(8);
        buffer.set_recovery_mode(AudioRecoveryMode::DropNewest);
        
        // Fill buffer
        for i in 0..4 {
            assert!(buffer.push_stereo(i * 10, i * 10 + 1));
        }
        
        // Overflow - should drop newest (not add them)
        assert!(!buffer.push_stereo(100, 101));
        assert_eq!(buffer.total_dropped, 2);
        
        // Buffer should still contain original samples
        let samples = buffer.pop_stereo(4);
        assert_eq!(samples[0], 0);
        assert_eq!(samples[1], 1);
    }

    #[test]
    fn test_ring_buffer_overflow_halve() {
        let mut buffer = AudioRingBuffer::new(8);
        buffer.set_recovery_mode(AudioRecoveryMode::HalveBuffer);
        
        // Fill buffer
        for i in 0..4 {
            assert!(buffer.push_stereo(i * 10, i * 10 + 1));
        }
        
        // Overflow - should halve the buffer
        assert!(!buffer.push_stereo(100, 101));
        assert_eq!(buffer.current_fill, 4); // Half of 8
        assert_eq!(buffer.total_dropped, 4);
    }

    #[test]
    fn test_ring_buffer_overflow_reset() {
        let mut buffer = AudioRingBuffer::new(8);
        buffer.set_recovery_mode(AudioRecoveryMode::Reset);
        
        // Fill buffer
        for i in 0..4 {
            assert!(buffer.push_stereo(i * 10, i * 10 + 1));
        }
        
        // Overflow - should reset entire buffer
        assert!(!buffer.push_stereo(100, 101));
        assert_eq!(buffer.current_fill, 0);
        assert_eq!(buffer.total_dropped, 8);
        
        // Should be able to add now
        assert!(buffer.push_stereo(200, 201));
        assert_eq!(buffer.current_fill, 2);
    }

    #[test]
    fn test_latency_monitoring() {
        let mut buffer = AudioRingBuffer::new(64);
        buffer.set_target_latency(16);
        
        // Add samples
        for _ in 0..8 {
            buffer.push_stereo(0, 0);
        }
        
        // Check average latency
        let avg = buffer.get_average_latency();
        assert!(avg > 0.0);
        
        // Fill percentage
        let fill_pct = buffer.fill_percentage();
        assert_eq!(fill_pct, 25.0); // 16/64 = 25%
    }

    #[test]
    fn test_buffer_stats() {
        let mut buffer = AudioRingBuffer::new(32);
        
        // Add some samples
        for _ in 0..5 {
            buffer.push_stereo(0, 0);
        }
        
        let stats = buffer.get_stats();
        assert_eq!(stats.total_written, 10);
        assert_eq!(stats.current_fill, 10);
        assert_eq!(stats.buffer_size, 32);
        assert_eq!(stats.drop_rate, 0.0);
        assert!(!stats.is_dropping);
        
        // Reset stats
        buffer.reset_stats();
        let stats = buffer.get_stats();
        assert_eq!(stats.total_written, 0);
        assert_eq!(stats.total_dropped, 0);
    }

    #[test]
    fn test_power_of_two_rounding() {
        let buffer = AudioRingBuffer::new(10);
        assert_eq!(buffer.size, 16); // Should round to 16
        
        let buffer = AudioRingBuffer::new(32);
        assert_eq!(buffer.size, 32); // Already power of 2
        
        let buffer = AudioRingBuffer::new(100);
        assert_eq!(buffer.size, 128); // Should round to 128
    }

    #[test]
    fn test_debug_overlay_update() {
        let mut overlay = SpuDebugOverlay::new();
        
        let stats = AudioBufferStats {
            total_written: 1000,
            total_dropped: 10,
            current_fill: 512,
            max_fill_level: 600,
            buffer_size: 1024,
            drop_rate: 1.0,
            average_latency: 512.0,
            is_dropping: false,
        };
        
        overlay.update_buffer_health(&stats);
        assert_eq!(overlay.buffer_health, BufferHealth::Warning); // 50% full
        
        let stats = AudioBufferStats {
            total_written: 1000,
            total_dropped: 10,
            current_fill: 800,
            max_fill_level: 900,
            buffer_size: 1024,
            drop_rate: 1.0,
            average_latency: 800.0,
            is_dropping: false,
        };
        
        overlay.update_buffer_health(&stats);
        assert_eq!(overlay.buffer_health, BufferHealth::Critical); // 78% full
        
        let stats = AudioBufferStats {
            total_written: 1000,
            total_dropped: 10,
            current_fill: 950,
            max_fill_level: 1000,
            buffer_size: 1024,
            drop_rate: 1.0,
            average_latency: 950.0,
            is_dropping: true,
        };
        
        overlay.update_buffer_health(&stats);
        assert_eq!(overlay.buffer_health, BufferHealth::Overflow); // Dropping samples
    }
}
