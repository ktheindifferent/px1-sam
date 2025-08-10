// Performance monitoring and metrics for PSX emulator
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use serde::{Serialize, Deserialize};

const SAMPLE_WINDOW_SIZE: usize = 60; // Keep last 60 samples (1 second at 60 FPS)
const METRIC_HISTORY_SIZE: usize = 1000; // Keep last 1000 metric entries

/// Performance metrics for the emulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub cpu_usage_percent: f64,
    pub gpu_usage_percent: f64,
    pub spu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub vram_usage_percent: f64,
    pub audio_latency_ms: f64,
    pub input_latency_ms: f64,
    pub emulation_speed_percent: f64,
}

/// Component-specific timing information
#[derive(Debug, Clone)]
pub struct ComponentTiming {
    pub cpu_time: Duration,
    pub gpu_time: Duration,
    pub spu_time: Duration,
    pub dma_time: Duration,
    pub controller_time: Duration,
    pub total_time: Duration,
}

/// Frame timing statistics
#[derive(Debug, Clone)]
pub struct FrameStats {
    pub min_time: Duration,
    pub max_time: Duration,
    pub avg_time: Duration,
    pub std_dev: Duration,
    pub percentile_95: Duration,
    pub percentile_99: Duration,
}

/// Performance monitor for tracking emulator performance
pub struct PerformanceMonitor {
    // Timing
    frame_start: Option<Instant>,
    last_frame_time: Duration,
    frame_times: VecDeque<Duration>,
    component_timings: VecDeque<ComponentTiming>,
    
    // FPS tracking
    fps_counter: FpsCounter,
    target_fps: f64,
    
    // Performance counters
    cpu_cycles: u64,
    gpu_primitives: u64,
    dma_transfers: u64,
    audio_samples: u64,
    
    // Resource usage
    memory_usage: MemoryTracker,
    
    // Metrics history
    metrics_history: VecDeque<PerformanceMetrics>,
    
    // Alerts and thresholds
    performance_alerts: Vec<PerformanceAlert>,
    alert_thresholds: AlertThresholds,
}

/// FPS counter with smoothing
struct FpsCounter {
    frame_count: u64,
    start_time: Instant,
    recent_frames: VecDeque<Instant>,
    smoothed_fps: f64,
}

/// Memory usage tracker
struct MemoryTracker {
    main_ram_usage: usize,
    vram_usage: usize,
    spu_ram_usage: usize,
    total_allocated: usize,
    peak_usage: usize,
}

/// Performance alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceAlert {
    LowFps { current: f64, target: f64 },
    HighFrameTime { time_ms: f64, threshold_ms: f64 },
    HighCpuUsage { percent: f64 },
    HighMemoryUsage { mb: f64, threshold_mb: f64 },
    AudioUnderrun { buffer_level: usize },
    InputLatencySpike { latency_ms: f64 },
}

/// Alert thresholds configuration
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub min_fps: f64,
    pub max_frame_time_ms: f64,
    pub max_cpu_usage_percent: f64,
    pub max_memory_usage_mb: f64,
    pub max_input_latency_ms: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        AlertThresholds {
            min_fps: 55.0,
            max_frame_time_ms: 20.0,
            max_cpu_usage_percent: 90.0,
            max_memory_usage_mb: 512.0,
            max_input_latency_ms: 50.0,
        }
    }
}

impl PerformanceMonitor {
    pub fn new(target_fps: f64) -> Self {
        PerformanceMonitor {
            frame_start: None,
            last_frame_time: Duration::ZERO,
            frame_times: VecDeque::with_capacity(SAMPLE_WINDOW_SIZE),
            component_timings: VecDeque::with_capacity(SAMPLE_WINDOW_SIZE),
            fps_counter: FpsCounter::new(),
            target_fps,
            cpu_cycles: 0,
            gpu_primitives: 0,
            dma_transfers: 0,
            audio_samples: 0,
            memory_usage: MemoryTracker::new(),
            metrics_history: VecDeque::with_capacity(METRIC_HISTORY_SIZE),
            performance_alerts: Vec::new(),
            alert_thresholds: AlertThresholds::default(),
        }
    }

    /// Start timing a new frame
    pub fn begin_frame(&mut self) {
        self.frame_start = Some(Instant::now());
        self.performance_alerts.clear();
    }

    /// End frame timing and update metrics
    pub fn end_frame(&mut self) {
        if let Some(start) = self.frame_start.take() {
            let frame_time = start.elapsed();
            self.last_frame_time = frame_time;
            
            // Update frame time history
            if self.frame_times.len() >= SAMPLE_WINDOW_SIZE {
                self.frame_times.pop_front();
            }
            self.frame_times.push_back(frame_time);
            
            // Update FPS counter
            self.fps_counter.add_frame();
            
            // Check for performance alerts
            self.check_performance_alerts(frame_time);
            
            // Update metrics
            self.update_metrics();
        }
    }

    /// Time a specific component's execution
    pub fn time_component<F, R>(&mut self, component: Component, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let elapsed = start.elapsed();
        
        // Update component timing
        match component {
            Component::Cpu => self.update_cpu_time(elapsed),
            Component::Gpu => self.update_gpu_time(elapsed),
            Component::Spu => self.update_spu_time(elapsed),
            Component::Dma => self.update_dma_time(elapsed),
            Component::Controller => self.update_controller_time(elapsed),
        }
        
        result
    }

    fn update_cpu_time(&mut self, time: Duration) {
        if let Some(timing) = self.component_timings.back_mut() {
            timing.cpu_time += time;
            timing.total_time += time;
        }
    }

    fn update_gpu_time(&mut self, time: Duration) {
        if let Some(timing) = self.component_timings.back_mut() {
            timing.gpu_time += time;
            timing.total_time += time;
        }
    }

    fn update_spu_time(&mut self, time: Duration) {
        if let Some(timing) = self.component_timings.back_mut() {
            timing.spu_time += time;
            timing.total_time += time;
        }
    }

    fn update_dma_time(&mut self, time: Duration) {
        if let Some(timing) = self.component_timings.back_mut() {
            timing.dma_time += time;
            timing.total_time += time;
        }
    }

    fn update_controller_time(&mut self, time: Duration) {
        if let Some(timing) = self.component_timings.back_mut() {
            timing.controller_time += time;
            timing.total_time += time;
        }
    }

    /// Update performance counters
    pub fn increment_cpu_cycles(&mut self, cycles: u64) {
        self.cpu_cycles = self.cpu_cycles.wrapping_add(cycles);
    }

    pub fn increment_gpu_primitives(&mut self, count: u64) {
        self.gpu_primitives = self.gpu_primitives.wrapping_add(count);
    }

    pub fn increment_dma_transfers(&mut self, count: u64) {
        self.dma_transfers = self.dma_transfers.wrapping_add(count);
    }

    pub fn increment_audio_samples(&mut self, count: u64) {
        self.audio_samples = self.audio_samples.wrapping_add(count);
    }

    /// Update memory usage statistics
    pub fn update_memory_usage(&mut self, ram: usize, vram: usize, spu_ram: usize) {
        self.memory_usage.main_ram_usage = ram;
        self.memory_usage.vram_usage = vram;
        self.memory_usage.spu_ram_usage = spu_ram;
        
        let total = ram + vram + spu_ram;
        self.memory_usage.total_allocated = total;
        
        if total > self.memory_usage.peak_usage {
            self.memory_usage.peak_usage = total;
        }
    }

    /// Check for performance issues and generate alerts
    fn check_performance_alerts(&mut self, frame_time: Duration) {
        let frame_time_ms = frame_time.as_secs_f64() * 1000.0;
        let current_fps = self.fps_counter.get_fps();
        
        // Check FPS
        if current_fps < self.alert_thresholds.min_fps {
            self.performance_alerts.push(PerformanceAlert::LowFps {
                current: current_fps,
                target: self.target_fps,
            });
        }
        
        // Check frame time
        if frame_time_ms > self.alert_thresholds.max_frame_time_ms {
            self.performance_alerts.push(PerformanceAlert::HighFrameTime {
                time_ms: frame_time_ms,
                threshold_ms: self.alert_thresholds.max_frame_time_ms,
            });
        }
        
        // Check memory usage
        let memory_mb = self.memory_usage.total_allocated as f64 / (1024.0 * 1024.0);
        if memory_mb > self.alert_thresholds.max_memory_usage_mb {
            self.performance_alerts.push(PerformanceAlert::HighMemoryUsage {
                mb: memory_mb,
                threshold_mb: self.alert_thresholds.max_memory_usage_mb,
            });
        }
    }

    /// Update performance metrics
    fn update_metrics(&mut self) {
        let metrics = PerformanceMetrics {
            fps: self.fps_counter.get_fps(),
            frame_time_ms: self.last_frame_time.as_secs_f64() * 1000.0,
            cpu_usage_percent: self.calculate_cpu_usage(),
            gpu_usage_percent: self.calculate_gpu_usage(),
            spu_usage_percent: self.calculate_spu_usage(),
            memory_usage_mb: self.memory_usage.total_allocated as f64 / (1024.0 * 1024.0),
            vram_usage_percent: (self.memory_usage.vram_usage as f64 / (1024.0 * 512.0 * 2.0)) * 100.0,
            audio_latency_ms: 0.0, // TODO: Implement audio latency measurement
            input_latency_ms: 0.0, // TODO: Implement input latency measurement
            emulation_speed_percent: (self.fps_counter.get_fps() / self.target_fps) * 100.0,
        };
        
        if self.metrics_history.len() >= METRIC_HISTORY_SIZE {
            self.metrics_history.pop_front();
        }
        self.metrics_history.push_back(metrics);
    }

    fn calculate_cpu_usage(&self) -> f64 {
        if let Some(timing) = self.component_timings.back() {
            let cpu_percent = (timing.cpu_time.as_secs_f64() / timing.total_time.as_secs_f64()) * 100.0;
            cpu_percent.min(100.0)
        } else {
            0.0
        }
    }

    fn calculate_gpu_usage(&self) -> f64 {
        if let Some(timing) = self.component_timings.back() {
            let gpu_percent = (timing.gpu_time.as_secs_f64() / timing.total_time.as_secs_f64()) * 100.0;
            gpu_percent.min(100.0)
        } else {
            0.0
        }
    }

    fn calculate_spu_usage(&self) -> f64 {
        if let Some(timing) = self.component_timings.back() {
            let spu_percent = (timing.spu_time.as_secs_f64() / timing.total_time.as_secs_f64()) * 100.0;
            spu_percent.min(100.0)
        } else {
            0.0
        }
    }

    /// Get current performance metrics
    pub fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics_history.back().cloned().unwrap_or_else(|| {
            PerformanceMetrics {
                fps: 0.0,
                frame_time_ms: 0.0,
                cpu_usage_percent: 0.0,
                gpu_usage_percent: 0.0,
                spu_usage_percent: 0.0,
                memory_usage_mb: 0.0,
                vram_usage_percent: 0.0,
                audio_latency_ms: 0.0,
                input_latency_ms: 0.0,
                emulation_speed_percent: 0.0,
            }
        })
    }

    /// Get frame timing statistics
    pub fn get_frame_stats(&self) -> FrameStats {
        if self.frame_times.is_empty() {
            return FrameStats {
                min_time: Duration::ZERO,
                max_time: Duration::ZERO,
                avg_time: Duration::ZERO,
                std_dev: Duration::ZERO,
                percentile_95: Duration::ZERO,
                percentile_99: Duration::ZERO,
            };
        }

        let mut sorted_times: Vec<Duration> = self.frame_times.iter().cloned().collect();
        sorted_times.sort();

        let min_time = sorted_times[0];
        let max_time = sorted_times[sorted_times.len() - 1];
        
        let sum: Duration = sorted_times.iter().sum();
        let avg_time = sum / sorted_times.len() as u32;
        
        // Calculate standard deviation
        let variance: f64 = sorted_times
            .iter()
            .map(|t| {
                let diff = t.as_secs_f64() - avg_time.as_secs_f64();
                diff * diff
            })
            .sum::<f64>() / sorted_times.len() as f64;
        let std_dev = Duration::from_secs_f64(variance.sqrt());
        
        // Calculate percentiles
        let p95_index = (sorted_times.len() as f64 * 0.95) as usize;
        let p99_index = (sorted_times.len() as f64 * 0.99) as usize;
        
        FrameStats {
            min_time,
            max_time,
            avg_time,
            std_dev,
            percentile_95: sorted_times[p95_index.min(sorted_times.len() - 1)],
            percentile_99: sorted_times[p99_index.min(sorted_times.len() - 1)],
        }
    }

    /// Get current performance alerts
    pub fn get_alerts(&self) -> &[PerformanceAlert] {
        &self.performance_alerts
    }

    /// Clear all metrics history
    pub fn reset(&mut self) {
        self.frame_times.clear();
        self.component_timings.clear();
        self.metrics_history.clear();
        self.performance_alerts.clear();
        self.cpu_cycles = 0;
        self.gpu_primitives = 0;
        self.dma_transfers = 0;
        self.audio_samples = 0;
        self.fps_counter = FpsCounter::new();
    }

    /// Export metrics history for analysis
    pub fn export_metrics(&self) -> Vec<PerformanceMetrics> {
        self.metrics_history.iter().cloned().collect()
    }
}

impl FpsCounter {
    fn new() -> Self {
        FpsCounter {
            frame_count: 0,
            start_time: Instant::now(),
            recent_frames: VecDeque::with_capacity(60),
            smoothed_fps: 0.0,
        }
    }

    fn add_frame(&mut self) {
        let now = Instant::now();
        self.frame_count += 1;
        
        // Add to recent frames
        if self.recent_frames.len() >= 60 {
            self.recent_frames.pop_front();
        }
        self.recent_frames.push_back(now);
        
        // Calculate smoothed FPS over recent frames
        if self.recent_frames.len() >= 2 {
            let duration = self.recent_frames.back().unwrap()
                .duration_since(*self.recent_frames.front().unwrap());
            
            if duration.as_secs_f64() > 0.0 {
                let fps = (self.recent_frames.len() - 1) as f64 / duration.as_secs_f64();
                self.smoothed_fps = self.smoothed_fps * 0.9 + fps * 0.1; // Exponential smoothing
            }
        }
    }

    fn get_fps(&self) -> f64 {
        self.smoothed_fps
    }
}

impl MemoryTracker {
    fn new() -> Self {
        MemoryTracker {
            main_ram_usage: 0,
            vram_usage: 0,
            spu_ram_usage: 0,
            total_allocated: 0,
            peak_usage: 0,
        }
    }
}

/// Component types for timing
#[derive(Debug, Clone, Copy)]
pub enum Component {
    Cpu,
    Gpu,
    Spu,
    Dma,
    Controller,
}

impl Default for ComponentTiming {
    fn default() -> Self {
        ComponentTiming {
            cpu_time: Duration::ZERO,
            gpu_time: Duration::ZERO,
            spu_time: Duration::ZERO,
            dma_time: Duration::ZERO,
            controller_time: Duration::ZERO,
            total_time: Duration::ZERO,
        }
    }
}