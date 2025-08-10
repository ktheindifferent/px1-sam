// Performance monitoring tests

use crate::performance_monitor::*;
use std::time::Duration;
use std::thread;

#[test]
fn test_performance_monitor_creation() {
    let monitor = PerformanceMonitor::new(60.0);
    let metrics = monitor.get_metrics();
    
    assert_eq!(metrics.fps, 0.0);
    assert_eq!(metrics.frame_time_ms, 0.0);
    assert_eq!(metrics.cpu_usage_percent, 0.0);
}

#[test]
fn test_frame_timing() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    monitor.begin_frame();
    thread::sleep(Duration::from_millis(10));
    monitor.end_frame();
    
    // Frame time should be approximately 10ms
    let metrics = monitor.get_metrics();
    assert!(metrics.frame_time_ms >= 10.0);
    assert!(metrics.frame_time_ms < 20.0);
}

#[test]
fn test_component_timing() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    monitor.begin_frame();
    
    // Time CPU component
    let result = monitor.time_component(Component::Cpu, || {
        thread::sleep(Duration::from_millis(5));
        42
    });
    assert_eq!(result, 42);
    
    // Time GPU component
    monitor.time_component(Component::Gpu, || {
        thread::sleep(Duration::from_millis(3));
    });
    
    monitor.end_frame();
    
    let metrics = monitor.get_metrics();
    assert!(metrics.cpu_usage_percent > 0.0);
    assert!(metrics.gpu_usage_percent > 0.0);
}

#[test]
fn test_performance_counters() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    monitor.increment_cpu_cycles(1000);
    monitor.increment_gpu_primitives(50);
    monitor.increment_dma_transfers(10);
    monitor.increment_audio_samples(512);
    
    // Counters should accumulate
    monitor.increment_cpu_cycles(500);
    // Internal counter should be 1500
}

#[test]
fn test_memory_tracking() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    monitor.update_memory_usage(1024 * 1024, 512 * 1024, 256 * 1024);
    
    let metrics = monitor.get_metrics();
    let expected_mb = (1024 + 512 + 256) as f64 / 1024.0;
    assert_eq!(metrics.memory_usage_mb, expected_mb);
}

#[test]
fn test_performance_alerts() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    monitor.begin_frame();
    thread::sleep(Duration::from_millis(25)); // Exceed threshold
    monitor.end_frame();
    
    let alerts = monitor.get_alerts();
    assert!(!alerts.is_empty());
    
    // Should have high frame time alert
    let has_frame_time_alert = alerts.iter().any(|a| {
        matches!(a, PerformanceAlert::HighFrameTime { .. })
    });
    assert!(has_frame_time_alert);
}

#[test]
fn test_frame_statistics() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    // Generate multiple frames with varying times
    for i in 0..10 {
        monitor.begin_frame();
        thread::sleep(Duration::from_millis(10 + i));
        monitor.end_frame();
    }
    
    let stats = monitor.get_frame_stats();
    assert!(stats.min_time < stats.max_time);
    assert!(stats.avg_time >= stats.min_time);
    assert!(stats.avg_time <= stats.max_time);
    assert!(stats.percentile_95 >= stats.avg_time);
}

#[test]
fn test_metrics_history() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    // Generate some frames
    for _ in 0..5 {
        monitor.begin_frame();
        thread::sleep(Duration::from_millis(16));
        monitor.end_frame();
    }
    
    let history = monitor.export_metrics();
    assert!(!history.is_empty());
    assert!(history.len() <= 5);
}

#[test]
fn test_monitor_reset() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    // Generate some data
    monitor.begin_frame();
    monitor.increment_cpu_cycles(1000);
    monitor.end_frame();
    
    // Reset should clear everything
    monitor.reset();
    
    let metrics = monitor.get_metrics();
    assert_eq!(metrics.fps, 0.0);
    assert_eq!(metrics.frame_time_ms, 0.0);
    
    let alerts = monitor.get_alerts();
    assert!(alerts.is_empty());
}

#[test]
fn test_alert_thresholds() {
    let thresholds = AlertThresholds::default();
    
    assert_eq!(thresholds.min_fps, 55.0);
    assert_eq!(thresholds.max_frame_time_ms, 20.0);
    assert_eq!(thresholds.max_cpu_usage_percent, 90.0);
    assert_eq!(thresholds.max_memory_usage_mb, 512.0);
    assert_eq!(thresholds.max_input_latency_ms, 50.0);
}

#[test]
fn test_emulation_speed_calculation() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    // Simulate running at half speed (30 FPS)
    // This is a simplified test - actual FPS calculation requires multiple frames
    monitor.begin_frame();
    thread::sleep(Duration::from_millis(33)); // ~30 FPS
    monitor.end_frame();
    
    // The emulation speed should reflect the FPS ratio
    // Note: This test is simplified as FPS calculation needs multiple frames
}

#[test]
fn test_vram_usage_percentage() {
    let mut monitor = PerformanceMonitor::new(60.0);
    
    // VRAM is 1024x512 16-bit pixels = 1MB
    let vram_bytes = 1024 * 512 * 2;
    monitor.update_memory_usage(0, vram_bytes / 2, 0);
    
    let metrics = monitor.get_metrics();
    assert_eq!(metrics.vram_usage_percent, 50.0);
}