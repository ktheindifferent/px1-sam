// Performance Monitoring System for Rustation-NG
// Comprehensive performance tracking, profiling, and visualization

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ============================================================================
// Core Performance Monitor
// ============================================================================

/// Main performance monitoring system
pub struct PerformanceMonitor {
    // Timing measurements
    frame_timer: FrameTimer,
    component_timers: HashMap<ComponentId, ComponentTimer>,
    
    // Performance metrics
    cpu_monitor: CpuMonitor,
    gpu_monitor: GpuMonitor,
    memory_monitor: MemoryMonitor,
    
    // Profiling data
    profiler: Profiler,
    hotspot_detector: HotspotDetector,
    
    // Statistics
    stats_collector: StatsCollector,
    
    // Overlay rendering
    overlay: PerformanceOverlay,
    
    // Configuration
    config: MonitorConfig,
}

impl PerformanceMonitor {
    pub fn new(config: MonitorConfig) -> Self {
        PerformanceMonitor {
            frame_timer: FrameTimer::new(),
            component_timers: HashMap::new(),
            cpu_monitor: CpuMonitor::new(),
            gpu_monitor: GpuMonitor::new(),
            memory_monitor: MemoryMonitor::new(),
            profiler: Profiler::new(config.profiling_enabled),
            hotspot_detector: HotspotDetector::new(),
            stats_collector: StatsCollector::new(),
            overlay: PerformanceOverlay::new(config.overlay_config.clone()),
            config,
        }
    }
    
    /// Start frame timing
    pub fn begin_frame(&mut self) {
        self.frame_timer.begin_frame();
        
        if self.config.detailed_timing {
            for timer in self.component_timers.values_mut() {
                timer.reset();
            }
        }
    }
    
    /// End frame timing and collect statistics
    pub fn end_frame(&mut self) {
        self.frame_timer.end_frame();
        
        // Collect frame statistics
        let frame_stats = FrameStats {
            frame_time: self.frame_timer.last_frame_time(),
            fps: self.frame_timer.current_fps(),
            cpu_usage: self.cpu_monitor.get_usage(),
            gpu_usage: self.gpu_monitor.get_usage(),
            memory_usage: self.memory_monitor.get_usage(),
            component_times: self.collect_component_times(),
        };
        
        self.stats_collector.add_frame_stats(frame_stats);
        
        // Update hotspot detection
        if self.config.hotspot_detection {
            self.hotspot_detector.update(&self.profiler);
        }
    }
    
    /// Start timing a specific component
    pub fn start_component(&mut self, component: ComponentId) {
        if !self.config.detailed_timing {
            return;
        }
        
        self.component_timers
            .entry(component)
            .or_insert_with(ComponentTimer::new)
            .start();
    }
    
    /// End timing a specific component
    pub fn end_component(&mut self, component: ComponentId) {
        if let Some(timer) = self.component_timers.get_mut(&component) {
            timer.end();
        }
    }
    
    /// Profile a code section
    pub fn profile_section<F, R>(&mut self, name: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        if self.config.profiling_enabled {
            self.profiler.begin_section(name);
            let result = f();
            self.profiler.end_section(name);
            result
        } else {
            f()
        }
    }
    
    /// Get current performance statistics
    pub fn get_stats(&self) -> PerformanceStats {
        PerformanceStats {
            current_fps: self.frame_timer.current_fps(),
            average_fps: self.frame_timer.average_fps(),
            frame_time: self.frame_timer.average_frame_time(),
            frame_time_variance: self.frame_timer.frame_time_variance(),
            cpu_usage: self.cpu_monitor.get_detailed_usage(),
            gpu_usage: self.gpu_monitor.get_detailed_usage(),
            memory_info: self.memory_monitor.get_detailed_info(),
            hotspots: self.hotspot_detector.get_hotspots(),
            component_breakdown: self.get_component_breakdown(),
        }
    }
    
    /// Render performance overlay
    pub fn render_overlay(&mut self) -> Option<OverlayFrame> {
        if !self.config.overlay_enabled {
            return None;
        }
        
        let stats = self.get_stats();
        Some(self.overlay.render(&stats))
    }
    
    fn collect_component_times(&self) -> HashMap<ComponentId, Duration> {
        self.component_timers
            .iter()
            .map(|(id, timer)| (*id, timer.total_time()))
            .collect()
    }
    
    fn get_component_breakdown(&self) -> Vec<ComponentBreakdown> {
        let total_time = self.frame_timer.last_frame_time();
        
        self.component_timers
            .iter()
            .map(|(id, timer)| {
                let time = timer.total_time();
                ComponentBreakdown {
                    component: *id,
                    time,
                    percentage: (time.as_secs_f32() / total_time.as_secs_f32()) * 100.0,
                    call_count: timer.call_count(),
                }
            })
            .collect()
    }
}

// ============================================================================
// Frame Timer
// ============================================================================

/// High-precision frame timing
pub struct FrameTimer {
    frame_start: Option<Instant>,
    frame_times: VecDeque<Duration>,
    fps_history: VecDeque<f32>,
    last_fps_update: Instant,
    frame_count: u64,
}

impl FrameTimer {
    pub fn new() -> Self {
        FrameTimer {
            frame_start: None,
            frame_times: VecDeque::with_capacity(120),
            fps_history: VecDeque::with_capacity(60),
            last_fps_update: Instant::now(),
            frame_count: 0,
        }
    }
    
    pub fn begin_frame(&mut self) {
        self.frame_start = Some(Instant::now());
    }
    
    pub fn end_frame(&mut self) {
        if let Some(start) = self.frame_start.take() {
            let frame_time = start.elapsed();
            
            // Store frame time
            if self.frame_times.len() >= 120 {
                self.frame_times.pop_front();
            }
            self.frame_times.push_back(frame_time);
            
            // Update FPS counter
            self.frame_count += 1;
            if self.last_fps_update.elapsed() >= Duration::from_secs(1) {
                let fps = self.frame_count as f32;
                if self.fps_history.len() >= 60 {
                    self.fps_history.pop_front();
                }
                self.fps_history.push_back(fps);
                
                self.frame_count = 0;
                self.last_fps_update = Instant::now();
            }
        }
    }
    
    pub fn current_fps(&self) -> f32 {
        self.fps_history.back().copied().unwrap_or(0.0)
    }
    
    pub fn average_fps(&self) -> f32 {
        if self.fps_history.is_empty() {
            return 0.0;
        }
        
        let sum: f32 = self.fps_history.iter().sum();
        sum / self.fps_history.len() as f32
    }
    
    pub fn last_frame_time(&self) -> Duration {
        self.frame_times.back().copied().unwrap_or_default()
    }
    
    pub fn average_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::default();
        }
        
        let sum: Duration = self.frame_times.iter().sum();
        sum / self.frame_times.len() as u32
    }
    
    pub fn frame_time_variance(&self) -> f32 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }
        
        let mean = self.average_frame_time().as_secs_f32();
        let variance: f32 = self.frame_times
            .iter()
            .map(|t| {
                let diff = t.as_secs_f32() - mean;
                diff * diff
            })
            .sum();
        
        (variance / self.frame_times.len() as f32).sqrt()
    }
}

// ============================================================================
// CPU Monitor
// ============================================================================

/// CPU usage monitoring
pub struct CpuMonitor {
    emulated_cycles: u64,
    host_cycles: u64,
    instruction_counts: HashMap<InstructionType, u64>,
    pipeline_stalls: u64,
    cache_hits: u64,
    cache_misses: u64,
}

impl CpuMonitor {
    pub fn new() -> Self {
        CpuMonitor {
            emulated_cycles: 0,
            host_cycles: 0,
            instruction_counts: HashMap::new(),
            pipeline_stalls: 0,
            cache_hits: 0,
            cache_misses: 0,
        }
    }
    
    pub fn record_instruction(&mut self, instruction: InstructionType) {
        *self.instruction_counts.entry(instruction).or_insert(0) += 1;
    }
    
    pub fn record_cache_access(&mut self, hit: bool) {
        if hit {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }
    }
    
    pub fn get_usage(&self) -> f32 {
        // Calculate CPU usage percentage
        if self.host_cycles == 0 {
            return 0.0;
        }
        
        (self.emulated_cycles as f32 / self.host_cycles as f32) * 100.0
    }
    
    pub fn get_detailed_usage(&self) -> CpuUsageInfo {
        CpuUsageInfo {
            usage_percent: self.get_usage(),
            emulated_mips: (self.emulated_cycles as f32) / 1_000_000.0,
            cache_hit_rate: if self.cache_hits + self.cache_misses > 0 {
                (self.cache_hits as f32 / (self.cache_hits + self.cache_misses) as f32) * 100.0
            } else {
                0.0
            },
            pipeline_efficiency: if self.emulated_cycles > 0 {
                ((self.emulated_cycles - self.pipeline_stalls) as f32 / self.emulated_cycles as f32) * 100.0
            } else {
                100.0
            },
            instruction_breakdown: self.get_instruction_breakdown(),
        }
    }
    
    fn get_instruction_breakdown(&self) -> Vec<(InstructionType, f32)> {
        let total: u64 = self.instruction_counts.values().sum();
        if total == 0 {
            return Vec::new();
        }
        
        self.instruction_counts
            .iter()
            .map(|(inst, count)| (*inst, (*count as f32 / total as f32) * 100.0))
            .collect()
    }
}

// ============================================================================
// GPU Monitor
// ============================================================================

/// GPU performance monitoring
pub struct GpuMonitor {
    draw_calls: u64,
    triangles_rendered: u64,
    pixels_drawn: u64,
    texture_uploads: u64,
    vram_reads: u64,
    vram_writes: u64,
    render_time: Duration,
}

impl GpuMonitor {
    pub fn new() -> Self {
        GpuMonitor {
            draw_calls: 0,
            triangles_rendered: 0,
            pixels_drawn: 0,
            texture_uploads: 0,
            vram_reads: 0,
            vram_writes: 0,
            render_time: Duration::default(),
        }
    }
    
    pub fn record_draw_call(&mut self, primitive_count: u64, pixel_count: u64) {
        self.draw_calls += 1;
        self.triangles_rendered += primitive_count;
        self.pixels_drawn += pixel_count;
    }
    
    pub fn get_usage(&self) -> f32 {
        // Estimate GPU usage based on render time vs frame time
        let target_frame_time = Duration::from_secs_f32(1.0 / 60.0);
        (self.render_time.as_secs_f32() / target_frame_time.as_secs_f32()) * 100.0
    }
    
    pub fn get_detailed_usage(&self) -> GpuUsageInfo {
        GpuUsageInfo {
            usage_percent: self.get_usage(),
            draw_calls: self.draw_calls,
            triangles_per_frame: self.triangles_rendered,
            pixels_per_frame: self.pixels_drawn,
            texture_bandwidth: self.texture_uploads * 2, // Assume 16-bit textures
            vram_bandwidth: (self.vram_reads + self.vram_writes) * 2,
            fillrate: self.pixels_drawn as f32 / 1_000_000.0, // Megapixels
        }
    }
}

// ============================================================================
// Memory Monitor
// ============================================================================

/// Memory usage monitoring
pub struct MemoryMonitor {
    ram_usage: usize,
    vram_usage: usize,
    spu_ram_usage: usize,
    texture_cache_size: usize,
    allocation_count: u64,
    deallocation_count: u64,
}

impl MemoryMonitor {
    pub fn new() -> Self {
        MemoryMonitor {
            ram_usage: 0,
            vram_usage: 0,
            spu_ram_usage: 0,
            texture_cache_size: 0,
            allocation_count: 0,
            deallocation_count: 0,
        }
    }
    
    pub fn get_usage(&self) -> f32 {
        let total = self.ram_usage + self.vram_usage + self.spu_ram_usage;
        (total as f32) / (1024.0 * 1024.0) // Convert to MB
    }
    
    pub fn get_detailed_info(&self) -> MemoryInfo {
        MemoryInfo {
            total_mb: self.get_usage(),
            ram_mb: self.ram_usage as f32 / (1024.0 * 1024.0),
            vram_mb: self.vram_usage as f32 / (1024.0 * 1024.0),
            spu_ram_kb: self.spu_ram_usage as f32 / 1024.0,
            texture_cache_mb: self.texture_cache_size as f32 / (1024.0 * 1024.0),
            allocations_per_frame: self.allocation_count,
            fragmentation_percent: self.calculate_fragmentation(),
        }
    }
    
    fn calculate_fragmentation(&self) -> f32 {
        // Simplified fragmentation calculation
        if self.allocation_count == 0 || self.deallocation_count == 0 {
            return 0.0;
        }
        
        let ratio = self.deallocation_count as f32 / self.allocation_count as f32;
        (1.0 - ratio) * 100.0
    }
}

// ============================================================================
// Profiler
// ============================================================================

/// Code profiling system
pub struct Profiler {
    enabled: bool,
    sections: HashMap<String, ProfileSection>,
    call_stack: Vec<(String, Instant)>,
}

impl Profiler {
    pub fn new(enabled: bool) -> Self {
        Profiler {
            enabled,
            sections: HashMap::new(),
            call_stack: Vec::new(),
        }
    }
    
    pub fn begin_section(&mut self, name: &str) {
        if !self.enabled {
            return;
        }
        
        self.call_stack.push((name.to_string(), Instant::now()));
    }
    
    pub fn end_section(&mut self, name: &str) {
        if !self.enabled {
            return;
        }
        
        if let Some((section_name, start_time)) = self.call_stack.pop() {
            if section_name == name {
                let elapsed = start_time.elapsed();
                
                let section = self.sections.entry(section_name).or_insert_with(|| {
                    ProfileSection {
                        total_time: Duration::default(),
                        call_count: 0,
                        min_time: Duration::MAX,
                        max_time: Duration::ZERO,
                    }
                });
                
                section.total_time += elapsed;
                section.call_count += 1;
                section.min_time = section.min_time.min(elapsed);
                section.max_time = section.max_time.max(elapsed);
            }
        }
    }
    
    pub fn get_sections(&self) -> &HashMap<String, ProfileSection> {
        &self.sections
    }
}

#[derive(Debug, Clone)]
pub struct ProfileSection {
    pub total_time: Duration,
    pub call_count: u64,
    pub min_time: Duration,
    pub max_time: Duration,
}

// ============================================================================
// Hotspot Detector
// ============================================================================

/// Identifies performance bottlenecks
pub struct HotspotDetector {
    hotspots: Vec<Hotspot>,
    threshold_percent: f32,
}

impl HotspotDetector {
    pub fn new() -> Self {
        HotspotDetector {
            hotspots: Vec::new(),
            threshold_percent: 5.0, // Flag sections taking >5% of frame time
        }
    }
    
    pub fn update(&mut self, profiler: &Profiler) {
        self.hotspots.clear();
        
        let total_time: Duration = profiler
            .get_sections()
            .values()
            .map(|s| s.total_time)
            .sum();
        
        if total_time.as_secs_f32() == 0.0 {
            return;
        }
        
        for (name, section) in profiler.get_sections() {
            let percent = (section.total_time.as_secs_f32() / total_time.as_secs_f32()) * 100.0;
            
            if percent >= self.threshold_percent {
                self.hotspots.push(Hotspot {
                    name: name.clone(),
                    percentage: percent,
                    total_time: section.total_time,
                    call_count: section.call_count,
                    average_time: section.total_time / section.call_count as u32,
                });
            }
        }
        
        // Sort by percentage (highest first)
        self.hotspots.sort_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap());
    }
    
    pub fn get_hotspots(&self) -> Vec<Hotspot> {
        self.hotspots.clone()
    }
}

#[derive(Debug, Clone)]
pub struct Hotspot {
    pub name: String,
    pub percentage: f32,
    pub total_time: Duration,
    pub call_count: u64,
    pub average_time: Duration,
}

// ============================================================================
// Performance Overlay
// ============================================================================

/// Visual performance overlay renderer
pub struct PerformanceOverlay {
    config: OverlayConfig,
    graph_data: GraphData,
}

impl PerformanceOverlay {
    pub fn new(config: OverlayConfig) -> Self {
        PerformanceOverlay {
            config,
            graph_data: GraphData::new(),
        }
    }
    
    pub fn render(&mut self, stats: &PerformanceStats) -> OverlayFrame {
        self.graph_data.update(stats);
        
        let mut elements = Vec::new();
        
        // FPS counter
        if self.config.show_fps {
            elements.push(OverlayElement::Text {
                position: (10, 10),
                text: format!("FPS: {:.1} (avg: {:.1})", stats.current_fps, stats.average_fps),
                color: self.get_fps_color(stats.current_fps),
            });
        }
        
        // Frame time
        if self.config.show_frame_time {
            elements.push(OverlayElement::Text {
                position: (10, 30),
                text: format!("Frame: {:.2}ms (var: {:.2}ms)", 
                    stats.frame_time.as_secs_f32() * 1000.0,
                    stats.frame_time_variance * 1000.0),
                color: Color::WHITE,
            });
        }
        
        // CPU usage
        if self.config.show_cpu_usage {
            elements.push(OverlayElement::Bar {
                position: (10, 50),
                width: 200,
                height: 20,
                value: stats.cpu_usage.usage_percent / 100.0,
                label: format!("CPU: {:.1}%", stats.cpu_usage.usage_percent),
                color: self.get_usage_color(stats.cpu_usage.usage_percent),
            });
        }
        
        // GPU usage
        if self.config.show_gpu_usage {
            elements.push(OverlayElement::Bar {
                position: (10, 75),
                width: 200,
                height: 20,
                value: stats.gpu_usage.usage_percent / 100.0,
                label: format!("GPU: {:.1}%", stats.gpu_usage.usage_percent),
                color: self.get_usage_color(stats.gpu_usage.usage_percent),
            });
        }
        
        // Memory usage
        if self.config.show_memory {
            elements.push(OverlayElement::Text {
                position: (10, 100),
                text: format!("MEM: {:.1}MB (VRAM: {:.1}MB)", 
                    stats.memory_info.total_mb,
                    stats.memory_info.vram_mb),
                color: Color::WHITE,
            });
        }
        
        // Frame time graph
        if self.config.show_graph {
            elements.push(OverlayElement::Graph {
                position: (10, 125),
                width: 200,
                height: 75,
                data: self.graph_data.get_frame_times(),
                label: "Frame Time".to_string(),
            });
        }
        
        // Component breakdown
        if self.config.show_breakdown {
            let mut y = 210;
            for breakdown in &stats.component_breakdown {
                elements.push(OverlayElement::Text {
                    position: (10, y),
                    text: format!("{:?}: {:.1}%", breakdown.component, breakdown.percentage),
                    color: Color::GRAY,
                });
                y += 15;
            }
        }
        
        // Hotspots
        if self.config.show_hotspots && !stats.hotspots.is_empty() {
            elements.push(OverlayElement::Text {
                position: (220, 10),
                text: "HOTSPOTS:".to_string(),
                color: Color::YELLOW,
            });
            
            let mut y = 30;
            for (i, hotspot) in stats.hotspots.iter().take(5).enumerate() {
                elements.push(OverlayElement::Text {
                    position: (220, y),
                    text: format!("{}. {} ({:.1}%)", i + 1, hotspot.name, hotspot.percentage),
                    color: Color::ORANGE,
                });
                y += 15;
            }
        }
        
        OverlayFrame { elements }
    }
    
    fn get_fps_color(&self, fps: f32) -> Color {
        if fps >= 59.0 {
            Color::GREEN
        } else if fps >= 50.0 {
            Color::YELLOW
        } else {
            Color::RED
        }
    }
    
    fn get_usage_color(&self, usage: f32) -> Color {
        if usage <= 50.0 {
            Color::GREEN
        } else if usage <= 80.0 {
            Color::YELLOW
        } else {
            Color::RED
        }
    }
}

// ============================================================================
// Supporting Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentId {
    Cpu,
    Gpu,
    Spu,
    Dma,
    CdRom,
    Timers,
    GTE,
    Memory,
    Controllers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstructionType {
    Arithmetic,
    Logic,
    Branch,
    Load,
    Store,
    Multiply,
    Divide,
    Coprocessor,
}

#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub overlay_enabled: bool,
    pub detailed_timing: bool,
    pub profiling_enabled: bool,
    pub hotspot_detection: bool,
    pub overlay_config: OverlayConfig,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        MonitorConfig {
            overlay_enabled: true,
            detailed_timing: false,
            profiling_enabled: false,
            hotspot_detection: false,
            overlay_config: OverlayConfig::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OverlayConfig {
    pub show_fps: bool,
    pub show_frame_time: bool,
    pub show_cpu_usage: bool,
    pub show_gpu_usage: bool,
    pub show_memory: bool,
    pub show_graph: bool,
    pub show_breakdown: bool,
    pub show_hotspots: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        OverlayConfig {
            show_fps: true,
            show_frame_time: true,
            show_cpu_usage: true,
            show_gpu_usage: true,
            show_memory: true,
            show_graph: false,
            show_breakdown: false,
            show_hotspots: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub current_fps: f32,
    pub average_fps: f32,
    pub frame_time: Duration,
    pub frame_time_variance: f32,
    pub cpu_usage: CpuUsageInfo,
    pub gpu_usage: GpuUsageInfo,
    pub memory_info: MemoryInfo,
    pub hotspots: Vec<Hotspot>,
    pub component_breakdown: Vec<ComponentBreakdown>,
}

#[derive(Debug, Clone)]
pub struct CpuUsageInfo {
    pub usage_percent: f32,
    pub emulated_mips: f32,
    pub cache_hit_rate: f32,
    pub pipeline_efficiency: f32,
    pub instruction_breakdown: Vec<(InstructionType, f32)>,
}

#[derive(Debug, Clone)]
pub struct GpuUsageInfo {
    pub usage_percent: f32,
    pub draw_calls: u64,
    pub triangles_per_frame: u64,
    pub pixels_per_frame: u64,
    pub texture_bandwidth: u64,
    pub vram_bandwidth: u64,
    pub fillrate: f32,
}

#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_mb: f32,
    pub ram_mb: f32,
    pub vram_mb: f32,
    pub spu_ram_kb: f32,
    pub texture_cache_mb: f32,
    pub allocations_per_frame: u64,
    pub fragmentation_percent: f32,
}

#[derive(Debug, Clone)]
pub struct ComponentBreakdown {
    pub component: ComponentId,
    pub time: Duration,
    pub percentage: f32,
    pub call_count: u64,
}

#[derive(Debug, Clone)]
pub struct FrameStats {
    pub frame_time: Duration,
    pub fps: f32,
    pub cpu_usage: f32,
    pub gpu_usage: f32,
    pub memory_usage: f32,
    pub component_times: HashMap<ComponentId, Duration>,
}

#[derive(Debug, Clone)]
pub struct OverlayFrame {
    pub elements: Vec<OverlayElement>,
}

#[derive(Debug, Clone)]
pub enum OverlayElement {
    Text {
        position: (i32, i32),
        text: String,
        color: Color,
    },
    Bar {
        position: (i32, i32),
        width: i32,
        height: i32,
        value: f32,
        label: String,
        color: Color,
    },
    Graph {
        position: (i32, i32),
        width: i32,
        height: i32,
        data: Vec<f32>,
        label: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 255 };
    pub const GREEN: Color = Color { r: 0, g: 255, b: 0, a: 255 };
    pub const YELLOW: Color = Color { r: 255, g: 255, b: 0, a: 255 };
    pub const RED: Color = Color { r: 255, g: 0, b: 0, a: 255 };
    pub const ORANGE: Color = Color { r: 255, g: 128, b: 0, a: 255 };
    pub const GRAY: Color = Color { r: 128, g: 128, b: 128, a: 255 };
}

// Helper implementations
struct ComponentTimer {
    start_time: Option<Instant>,
    total_time: Duration,
    call_count: u64,
}

impl ComponentTimer {
    fn new() -> Self {
        ComponentTimer {
            start_time: None,
            total_time: Duration::default(),
            call_count: 0,
        }
    }
    
    fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }
    
    fn end(&mut self) {
        if let Some(start) = self.start_time.take() {
            self.total_time += start.elapsed();
            self.call_count += 1;
        }
    }
    
    fn reset(&mut self) {
        self.total_time = Duration::default();
        self.call_count = 0;
    }
    
    fn total_time(&self) -> Duration {
        self.total_time
    }
    
    fn call_count(&self) -> u64 {
        self.call_count
    }
}

struct StatsCollector {
    frame_stats: VecDeque<FrameStats>,
}

impl StatsCollector {
    fn new() -> Self {
        StatsCollector {
            frame_stats: VecDeque::with_capacity(300),
        }
    }
    
    fn add_frame_stats(&mut self, stats: FrameStats) {
        if self.frame_stats.len() >= 300 {
            self.frame_stats.pop_front();
        }
        self.frame_stats.push_back(stats);
    }
}

struct GraphData {
    frame_times: VecDeque<f32>,
}

impl GraphData {
    fn new() -> Self {
        GraphData {
            frame_times: VecDeque::with_capacity(100),
        }
    }
    
    fn update(&mut self, stats: &PerformanceStats) {
        if self.frame_times.len() >= 100 {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(stats.frame_time.as_secs_f32() * 1000.0);
    }
    
    fn get_frame_times(&self) -> Vec<f32> {
        self.frame_times.iter().copied().collect()
    }
}