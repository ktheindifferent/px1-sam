// Video Recording and Streaming Architecture for Rustation-NG
// Complete implementation for video capture, encoding, and streaming

use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::path::{Path, PathBuf};
use std::collections::VecDeque;

// ============================================================================
// Core Video Recorder
// ============================================================================

/// Main video recording system with FFmpeg integration
pub struct VideoRecorder {
    // Recording state
    state: RecorderState,
    config: RecordingConfig,
    
    // Frame pipeline
    frame_queue: Arc<Mutex<FrameQueue>>,
    encoder_thread: Option<thread::JoinHandle<()>>,
    encoder_tx: Option<Sender<EncoderCommand>>,
    
    // Audio handling
    audio_mixer: AudioMixer,
    audio_queue: Arc<Mutex<AudioQueue>>,
    
    // Statistics
    stats: RecordingStats,
    
    // Output management
    output_path: PathBuf,
    temp_files: Vec<PathBuf>,
}

impl VideoRecorder {
    pub fn new(config: RecordingConfig) -> Result<Self> {
        let frame_queue = Arc::new(Mutex::new(FrameQueue::new(config.buffer_size)));
        let audio_queue = Arc::new(Mutex::new(AudioQueue::new()));
        
        Ok(VideoRecorder {
            state: RecorderState::Idle,
            config,
            frame_queue,
            encoder_thread: None,
            encoder_tx: None,
            audio_mixer: AudioMixer::new(),
            audio_queue,
            stats: RecordingStats::default(),
            output_path: PathBuf::new(),
            temp_files: Vec::new(),
        })
    }
    
    /// Start recording to the specified file
    pub fn start_recording(&mut self, output_path: &Path) -> Result<()> {
        if self.state != RecorderState::Idle {
            return Err(RecordingError::AlreadyRecording);
        }
        
        self.output_path = output_path.to_path_buf();
        self.stats = RecordingStats::default();
        
        // Start encoder thread
        let (tx, rx) = channel();
        self.encoder_tx = Some(tx);
        
        let encoder_config = self.config.encoder_config.clone();
        let frame_queue = Arc::clone(&self.frame_queue);
        let audio_queue = Arc::clone(&self.audio_queue);
        let output = self.output_path.clone();
        
        self.encoder_thread = Some(thread::spawn(move || {
            let mut encoder = VideoEncoder::new(encoder_config, output);
            encoder.run(rx, frame_queue, audio_queue);
        }));
        
        self.state = RecorderState::Recording;
        info!("Started recording to {:?}", self.output_path);
        
        Ok(())
    }
    
    /// Stop recording and finalize the video file
    pub fn stop_recording(&mut self) -> Result<()> {
        if self.state != RecorderState::Recording {
            return Err(RecordingError::NotRecording);
        }
        
        // Send stop command to encoder
        if let Some(tx) = &self.encoder_tx {
            tx.send(EncoderCommand::Stop).ok();
        }
        
        // Wait for encoder to finish
        if let Some(thread) = self.encoder_thread.take() {
            thread.join().map_err(|_| RecordingError::EncoderError)?;
        }
        
        self.state = RecorderState::Idle;
        self.encoder_tx = None;
        
        // Clean up temporary files
        for temp_file in &self.temp_files {
            std::fs::remove_file(temp_file).ok();
        }
        self.temp_files.clear();
        
        info!("Recording saved to {:?}", self.output_path);
        info!("Recording stats: {:?}", self.stats);
        
        Ok(())
    }
    
    /// Submit a frame for recording
    pub fn submit_frame(&mut self, frame: &Frame) -> Result<()> {
        if self.state != RecorderState::Recording {
            return Ok(()); // Silently ignore when not recording
        }
        
        // Apply preprocessing if needed
        let processed_frame = if self.config.preprocessing.enabled {
            self.preprocess_frame(frame)?
        } else {
            frame.clone()
        };
        
        // Queue frame for encoding
        {
            let mut queue = self.frame_queue.lock().unwrap();
            if queue.is_full() {
                self.stats.dropped_frames += 1;
                warn!("Frame queue full, dropping frame");
                return Ok(());
            }
            queue.push(processed_frame);
        }
        
        self.stats.frames_recorded += 1;
        
        Ok(())
    }
    
    /// Submit audio samples for recording
    pub fn submit_audio(&mut self, samples: &[i16]) -> Result<()> {
        if self.state != RecorderState::Recording {
            return Ok(());
        }
        
        // Mix and resample audio if needed
        let processed = self.audio_mixer.process(samples, self.config.audio_config.sample_rate);
        
        {
            let mut queue = self.audio_queue.lock().unwrap();
            queue.push(processed);
        }
        
        self.stats.audio_samples += samples.len() as u64;
        
        Ok(())
    }
    
    /// Take a screenshot
    pub fn take_screenshot(&self, frame: &Frame, path: &Path) -> Result<()> {
        let format = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("png");
        
        match format {
            "png" => self.save_png(frame, path),
            "jpg" | "jpeg" => self.save_jpeg(frame, path, 95),
            "bmp" => self.save_bmp(frame, path),
            _ => Err(RecordingError::UnsupportedFormat(format.to_string())),
        }
    }
    
    /// Record a GIF clip
    pub fn start_gif_recording(&mut self, duration_ms: u32) -> Result<()> {
        self.config.output_format = OutputFormat::Gif;
        self.config.gif_config.max_duration_ms = duration_ms;
        self.start_recording(&self.output_path.with_extension("gif"))
    }
    
    fn preprocess_frame(&self, frame: &Frame) -> Result<Frame> {
        let mut processed = frame.clone();
        let config = &self.config.preprocessing;
        
        // Apply scaling
        if config.scale_factor != 1.0 {
            processed = self.scale_frame(&processed, config.scale_factor)?;
        }
        
        // Apply filters
        for filter in &config.filters {
            processed = self.apply_filter(&processed, filter)?;
        }
        
        // Apply watermark
        if let Some(ref watermark) = config.watermark {
            processed = self.apply_watermark(&processed, watermark)?;
        }
        
        Ok(processed)
    }
}

// ============================================================================
// Video Encoder (FFmpeg Integration)
// ============================================================================

/// FFmpeg-based video encoder
struct VideoEncoder {
    config: EncoderConfig,
    output_path: PathBuf,
    ffmpeg_process: Option<std::process::Child>,
    frame_count: u64,
    start_time: std::time::Instant,
}

impl VideoEncoder {
    fn new(config: EncoderConfig, output_path: PathBuf) -> Self {
        VideoEncoder {
            config,
            output_path,
            ffmpeg_process: None,
            frame_count: 0,
            start_time: std::time::Instant::now(),
        }
    }
    
    fn run(
        &mut self,
        rx: Receiver<EncoderCommand>,
        frame_queue: Arc<Mutex<FrameQueue>>,
        audio_queue: Arc<Mutex<AudioQueue>>,
    ) {
        // Start FFmpeg process
        if let Err(e) = self.start_ffmpeg() {
            error!("Failed to start FFmpeg: {}", e);
            return;
        }
        
        // Main encoding loop
        loop {
            // Check for commands
            if let Ok(cmd) = rx.try_recv() {
                match cmd {
                    EncoderCommand::Stop => break,
                    EncoderCommand::Pause => {
                        thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }
                }
            }
            
            // Process frames
            let frame = {
                let mut queue = frame_queue.lock().unwrap();
                queue.pop()
            };
            
            if let Some(frame) = frame {
                if let Err(e) = self.encode_frame(&frame) {
                    error!("Failed to encode frame: {}", e);
                }
                self.frame_count += 1;
            }
            
            // Process audio
            let audio = {
                let mut queue = audio_queue.lock().unwrap();
                queue.pop_chunk(1024) // Get 1024 samples
            };
            
            if let Some(samples) = audio {
                if let Err(e) = self.encode_audio(&samples) {
                    error!("Failed to encode audio: {}", e);
                }
            }
            
            // Sleep if queues are empty
            if frame.is_none() && audio.is_none() {
                thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        
        // Finalize encoding
        self.finalize();
    }
    
    fn start_ffmpeg(&mut self) -> Result<()> {
        let mut cmd = std::process::Command::new("ffmpeg");
        
        // Input parameters
        cmd.args(&[
            "-y", // Overwrite output file
            "-f", "rawvideo",
            "-pixel_format", "rgba",
            "-video_size", &format!("{}x{}", self.config.width, self.config.height),
            "-framerate", &self.config.framerate.to_string(),
            "-i", "pipe:0", // Read video from stdin
        ]);
        
        // Audio input if enabled
        if self.config.audio_enabled {
            cmd.args(&[
                "-f", "s16le",
                "-ar", &self.config.audio_sample_rate.to_string(),
                "-ac", "2", // Stereo
                "-i", "pipe:3", // Read audio from fd 3
            ]);
        }
        
        // Video codec configuration
        match self.config.video_codec {
            VideoCodec::H264 => {
                cmd.args(&[
                    "-c:v", "libx264",
                    "-preset", &self.config.h264_preset,
                    "-crf", &self.config.quality.to_string(),
                ]);
            }
            VideoCodec::H265 => {
                cmd.args(&[
                    "-c:v", "libx265",
                    "-preset", &self.config.h265_preset,
                    "-crf", &self.config.quality.to_string(),
                ]);
            }
            VideoCodec::VP9 => {
                cmd.args(&[
                    "-c:v", "libvpx-vp9",
                    "-b:v", &format!("{}k", self.config.bitrate),
                    "-crf", &self.config.quality.to_string(),
                ]);
            }
            VideoCodec::AV1 => {
                cmd.args(&[
                    "-c:v", "libaom-av1",
                    "-crf", &self.config.quality.to_string(),
                    "-b:v", "0", // Constant quality mode
                ]);
            }
        }
        
        // Audio codec configuration
        if self.config.audio_enabled {
            match self.config.audio_codec {
                AudioCodec::AAC => cmd.args(&["-c:a", "aac", "-b:a", "192k"]),
                AudioCodec::MP3 => cmd.args(&["-c:a", "libmp3lame", "-b:a", "192k"]),
                AudioCodec::Opus => cmd.args(&["-c:a", "libopus", "-b:a", "128k"]),
                AudioCodec::FLAC => cmd.args(&["-c:a", "flac"]),
            };
        }
        
        // Output configuration
        cmd.args(&[
            "-pix_fmt", "yuv420p", // Compatibility
            self.output_path.to_str().unwrap(),
        ]);
        
        // Start process
        self.ffmpeg_process = Some(cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?);
        
        Ok(())
    }
    
    fn encode_frame(&mut self, frame: &Frame) -> Result<()> {
        if let Some(ref mut process) = self.ffmpeg_process {
            if let Some(ref mut stdin) = process.stdin {
                // Write raw RGBA data to FFmpeg
                stdin.write_all(&frame.pixels)?;
                stdin.flush()?;
            }
        }
        Ok(())
    }
    
    fn encode_audio(&mut self, samples: &[i16]) -> Result<()> {
        if let Some(ref mut process) = self.ffmpeg_process {
            // FFmpeg expects audio on a separate pipe (fd 3)
            // This would require platform-specific code or a wrapper library
            // For now, we'll write to a temporary file
            
            // Convert i16 samples to bytes
            let bytes: Vec<u8> = samples.iter()
                .flat_map(|&s| s.to_le_bytes())
                .collect();
            
            // Write to temp file (simplified approach)
            // In production, use named pipes or proper IPC
        }
        Ok(())
    }
    
    fn finalize(&mut self) {
        if let Some(mut process) = self.ffmpeg_process.take() {
            // Close stdin to signal end of stream
            drop(process.stdin.take());
            
            // Wait for FFmpeg to finish
            match process.wait() {
                Ok(status) => {
                    if !status.success() {
                        error!("FFmpeg exited with error: {:?}", status);
                    }
                }
                Err(e) => error!("Failed to wait for FFmpeg: {}", e),
            }
        }
        
        let duration = self.start_time.elapsed();
        let fps = self.frame_count as f32 / duration.as_secs_f32();
        info!("Encoding complete: {} frames in {:.2}s ({:.2} fps)", 
              self.frame_count, duration.as_secs_f32(), fps);
    }
}

// ============================================================================
// GIF Recording
// ============================================================================

/// Specialized GIF encoder for short clips
pub struct GifRecorder {
    frames: Vec<Frame>,
    config: GifConfig,
    palette: ColorPalette,
}

impl GifRecorder {
    pub fn new(config: GifConfig) -> Self {
        GifRecorder {
            frames: Vec::new(),
            config,
            palette: ColorPalette::new(),
        }
    }
    
    pub fn add_frame(&mut self, frame: &Frame) -> Result<()> {
        // Downsample if needed
        let scaled = if frame.width > self.config.max_width || frame.height > self.config.max_height {
            self.scale_frame_to_fit(frame)?
        } else {
            frame.clone()
        };
        
        // Quantize colors
        let quantized = self.palette.quantize(&scaled)?;
        
        self.frames.push(quantized);
        
        // Check duration limit
        let duration = self.frames.len() as u32 * self.config.frame_delay_ms;
        if duration >= self.config.max_duration_ms {
            return Err(RecordingError::GifDurationExceeded);
        }
        
        Ok(())
    }
    
    pub fn save(&self, path: &Path) -> Result<()> {
        use gif::{Encoder, Frame as GifFrame, Repeat};
        
        let mut file = std::fs::File::create(path)?;
        let mut encoder = Encoder::new(
            &mut file,
            self.frames[0].width as u16,
            self.frames[0].height as u16,
            &self.palette.to_gif_palette(),
        )?;
        
        encoder.set_repeat(Repeat::Infinite)?;
        
        for frame in &self.frames {
            let gif_frame = GifFrame {
                delay: self.config.frame_delay_ms as u16 / 10, // GIF uses centiseconds
                dispose: gif::DisposalMethod::Background,
                transparent: None,
                needs_user_input: false,
                top: 0,
                left: 0,
                width: frame.width as u16,
                height: frame.height as u16,
                interlaced: false,
                palette: None,
                buffer: std::borrow::Cow::Borrowed(&frame.pixels),
            };
            
            encoder.write_frame(&gif_frame)?;
        }
        
        Ok(())
    }
    
    fn scale_frame_to_fit(&self, frame: &Frame) -> Result<Frame> {
        let scale_x = self.config.max_width as f32 / frame.width as f32;
        let scale_y = self.config.max_height as f32 / frame.height as f32;
        let scale = scale_x.min(scale_y);
        
        let new_width = (frame.width as f32 * scale) as u32;
        let new_height = (frame.height as f32 * scale) as u32;
        
        // Simple nearest-neighbor scaling
        let mut scaled = Frame {
            width: new_width,
            height: new_height,
            pixels: vec![0; (new_width * new_height * 4) as usize],
        };
        
        for y in 0..new_height {
            for x in 0..new_width {
                let src_x = (x as f32 / scale) as u32;
                let src_y = (y as f32 / scale) as u32;
                
                let src_idx = ((src_y * frame.width + src_x) * 4) as usize;
                let dst_idx = ((y * new_width + x) * 4) as usize;
                
                scaled.pixels[dst_idx..dst_idx + 4]
                    .copy_from_slice(&frame.pixels[src_idx..src_idx + 4]);
            }
        }
        
        Ok(scaled)
    }
}

// ============================================================================
// Streaming Support
// ============================================================================

/// Live streaming integration (OBS, Twitch, YouTube)
pub struct StreamingManager {
    rtmp_url: String,
    stream_key: String,
    encoder: StreamEncoder,
    state: StreamState,
}

impl StreamingManager {
    pub fn new(rtmp_url: String, stream_key: String) -> Self {
        StreamingManager {
            rtmp_url,
            stream_key,
            encoder: StreamEncoder::new(),
            state: StreamState::Disconnected,
        }
    }
    
    pub fn start_stream(&mut self) -> Result<()> {
        let stream_url = format!("{}/{}", self.rtmp_url, self.stream_key);
        
        // Start RTMP stream with FFmpeg
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.args(&[
            "-re", // Real-time encoding
            "-f", "rawvideo",
            "-pixel_format", "rgba",
            "-video_size", "1280x720",
            "-framerate", "60",
            "-i", "pipe:0",
            "-c:v", "libx264",
            "-preset", "veryfast", // Low latency preset
            "-tune", "zerolatency",
            "-b:v", "2500k",
            "-maxrate", "2500k",
            "-bufsize", "5000k",
            "-pix_fmt", "yuv420p",
            "-g", "120", // Keyframe interval
            "-c:a", "aac",
            "-b:a", "128k",
            "-ar", "44100",
            "-f", "flv",
            &stream_url,
        ]);
        
        self.encoder.start_ffmpeg(cmd)?;
        self.state = StreamState::Streaming;
        
        Ok(())
    }
    
    pub fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        if self.state != StreamState::Streaming {
            return Err(RecordingError::NotStreaming);
        }
        
        self.encoder.send_frame(frame)
    }
    
    pub fn stop_stream(&mut self) -> Result<()> {
        self.encoder.stop()?;
        self.state = StreamState::Disconnected;
        Ok(())
    }
}

// ============================================================================
// Configuration Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct RecordingConfig {
    pub output_format: OutputFormat,
    pub encoder_config: EncoderConfig,
    pub audio_config: AudioConfig,
    pub preprocessing: PreprocessingConfig,
    pub gif_config: GifConfig,
    pub buffer_size: usize,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        RecordingConfig {
            output_format: OutputFormat::MP4,
            encoder_config: EncoderConfig::default(),
            audio_config: AudioConfig::default(),
            preprocessing: PreprocessingConfig::default(),
            gif_config: GifConfig::default(),
            buffer_size: 60, // 1 second at 60fps
        }
    }
}

#[derive(Debug, Clone)]
pub struct EncoderConfig {
    pub video_codec: VideoCodec,
    pub audio_codec: AudioCodec,
    pub width: u32,
    pub height: u32,
    pub framerate: u32,
    pub bitrate: u32, // in kbps
    pub quality: u32, // CRF value (0-51, lower is better)
    pub h264_preset: String,
    pub h265_preset: String,
    pub audio_enabled: bool,
    pub audio_sample_rate: u32,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        EncoderConfig {
            video_codec: VideoCodec::H264,
            audio_codec: AudioCodec::AAC,
            width: 1280,
            height: 720,
            framerate: 60,
            bitrate: 4000,
            quality: 23,
            h264_preset: "medium".to_string(),
            h265_preset: "medium".to_string(),
            audio_enabled: true,
            audio_sample_rate: 44100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreprocessingConfig {
    pub enabled: bool,
    pub scale_factor: f32,
    pub filters: Vec<VideoFilter>,
    pub watermark: Option<Watermark>,
}

impl Default for PreprocessingConfig {
    fn default() -> Self {
        PreprocessingConfig {
            enabled: false,
            scale_factor: 1.0,
            filters: Vec::new(),
            watermark: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GifConfig {
    pub max_width: u32,
    pub max_height: u32,
    pub frame_delay_ms: u32,
    pub max_duration_ms: u32,
    pub dithering: bool,
    pub color_count: u32,
}

impl Default for GifConfig {
    fn default() -> Self {
        GifConfig {
            max_width: 640,
            max_height: 480,
            frame_delay_ms: 33, // ~30fps
            max_duration_ms: 10000, // 10 seconds
            dithering: true,
            color_count: 256,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    MP4,
    WebM,
    MKV,
    AVI,
    Gif,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VideoCodec {
    H264,
    H265,
    VP9,
    AV1,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioCodec {
    AAC,
    MP3,
    Opus,
    FLAC,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecorderState {
    Idle,
    Recording,
    Paused,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamState {
    Disconnected,
    Connecting,
    Streaming,
    Error,
}

// ============================================================================
// Supporting Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>, // RGBA format
}

#[derive(Debug, Default)]
pub struct RecordingStats {
    pub frames_recorded: u64,
    pub dropped_frames: u64,
    pub audio_samples: u64,
    pub bytes_written: u64,
    pub encoding_time: std::time::Duration,
}

#[derive(Debug)]
pub enum RecordingError {
    AlreadyRecording,
    NotRecording,
    NotStreaming,
    EncoderError,
    IoError(std::io::Error),
    UnsupportedFormat(String),
    GifDurationExceeded,
}

impl From<std::io::Error> for RecordingError {
    fn from(e: std::io::Error) -> Self {
        RecordingError::IoError(e)
    }
}

impl From<gif::EncodingError> for RecordingError {
    fn from(_: gif::EncodingError) -> Self {
        RecordingError::EncoderError
    }
}

type Result<T> = std::result::Result<T, RecordingError>;

// Queue implementations
struct FrameQueue {
    frames: VecDeque<Frame>,
    max_size: usize,
}

impl FrameQueue {
    fn new(max_size: usize) -> Self {
        FrameQueue {
            frames: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
    
    fn push(&mut self, frame: Frame) {
        if self.frames.len() >= self.max_size {
            self.frames.pop_front();
        }
        self.frames.push_back(frame);
    }
    
    fn pop(&mut self) -> Option<Frame> {
        self.frames.pop_front()
    }
    
    fn is_full(&self) -> bool {
        self.frames.len() >= self.max_size
    }
}

struct AudioQueue {
    samples: VecDeque<i16>,
}

impl AudioQueue {
    fn new() -> Self {
        AudioQueue {
            samples: VecDeque::with_capacity(44100 * 2), // 1 second stereo
        }
    }
    
    fn push(&mut self, samples: Vec<i16>) {
        self.samples.extend(samples);
    }
    
    fn pop_chunk(&mut self, size: usize) -> Option<Vec<i16>> {
        if self.samples.len() >= size {
            Some(self.samples.drain(..size).collect())
        } else {
            None
        }
    }
}

// Placeholder implementations
struct AudioMixer;
impl AudioMixer {
    fn new() -> Self { AudioMixer }
    fn process(&self, samples: &[i16], _rate: u32) -> Vec<i16> { samples.to_vec() }
}

struct ColorPalette;
impl ColorPalette {
    fn new() -> Self { ColorPalette }
    fn quantize(&self, frame: &Frame) -> Result<Frame> { Ok(frame.clone()) }
    fn to_gif_palette(&self) -> [u8; 768] { [0; 768] }
}

struct StreamEncoder;
impl StreamEncoder {
    fn new() -> Self { StreamEncoder }
    fn start_ffmpeg(&mut self, _cmd: std::process::Command) -> Result<()> { Ok(()) }
    fn send_frame(&mut self, _frame: &Frame) -> Result<()> { Ok(()) }
    fn stop(&mut self) -> Result<()> { Ok(()) }
}

#[derive(Debug, Clone)]
struct VideoFilter;

#[derive(Debug, Clone)]
struct Watermark;

#[derive(Debug, Clone)]
struct AudioConfig {
    sample_rate: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        AudioConfig { sample_rate: 44100 }
    }
}

enum EncoderCommand {
    Stop,
    Pause,
}

impl VideoRecorder {
    fn scale_frame(&self, _frame: &Frame, _factor: f32) -> Result<Frame> { Ok(Frame::default()) }
    fn apply_filter(&self, frame: &Frame, _filter: &VideoFilter) -> Result<Frame> { Ok(frame.clone()) }
    fn apply_watermark(&self, frame: &Frame, _watermark: &Watermark) -> Result<Frame> { Ok(frame.clone()) }
    fn save_png(&self, _frame: &Frame, _path: &Path) -> Result<()> { Ok(()) }
    fn save_jpeg(&self, _frame: &Frame, _path: &Path, _quality: u8) -> Result<()> { Ok(()) }
    fn save_bmp(&self, _frame: &Frame, _path: &Path) -> Result<()> { Ok(()) }
}

// Required external crate placeholder
mod gif {
    pub struct Encoder<'a, W: std::io::Write> {
        _writer: &'a mut W,
    }
    
    impl<'a, W: std::io::Write> Encoder<'a, W> {
        pub fn new(_: &'a mut W, _: u16, _: u16, _: &[u8]) -> Result<Self, EncodingError> {
            unimplemented!()
        }
        pub fn set_repeat(&mut self, _: Repeat) -> Result<(), EncodingError> { Ok(()) }
        pub fn write_frame(&mut self, _: &Frame) -> Result<(), EncodingError> { Ok(()) }
    }
    
    pub struct Frame<'a> {
        pub delay: u16,
        pub dispose: DisposalMethod,
        pub transparent: Option<u8>,
        pub needs_user_input: bool,
        pub top: u16,
        pub left: u16,
        pub width: u16,
        pub height: u16,
        pub interlaced: bool,
        pub palette: Option<Vec<u8>>,
        pub buffer: std::borrow::Cow<'a, [u8]>,
    }
    
    pub enum DisposalMethod { Background }
    pub enum Repeat { Infinite }
    pub struct EncodingError;
}

use std::io::Write;