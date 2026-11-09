// SPDX-License-Identifier: MIT
// SigmaOS-mangohud - Game Performance Overlay component
// MangoHud-style real-time performance overlay for games,
// inspired by Arch's mangohud package (MangoApp/Vulkan/OpenGL overlay).

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// PERFORMANCE METRICS
// ============================================================================

/// A single performance metric reading
#[derive(Debug, Clone)]
pub struct Metric {
    /// Metric name (e.g. "fps", "frametime", "gpu_usage")
    pub name: String,
    /// Current value
    pub value: f64,
    /// Unit ("fps", "ms", "%", "W", "C", "MB")
    pub unit: String,
    /// Minimum value for graph scaling
    pub min: f64,
    /// Maximum value for graph scaling
    pub max: f64,
    /// Whether higher is better
    pub higher_is_better: bool,
    /// History for graph (ring buffer)
    pub history: Vec<f64>,
}

impl Metric {
    pub fn new(name: &str, unit: &str, min: f64, max: f64, higher_is_better: bool) -> Self {
        Self {
            name: name.to_string(),
            value: 0.0,
            unit: unit.to_string(),
            min,
            max,
            higher_is_better,
            history: Vec::new(),
        }
    }

    /// Push a new value and update history
    pub fn update(&mut self, value: f64) {
        self.value = value;
        self.history.push(value);
        // Keep last 120 frames (~2s at 60fps)
        if self.history.len() > 120 {
            self.history.remove(0);
        }
    }

    /// Average of recent values
    pub fn average(&self, n: usize) -> f64 {
        let count = n.min(self.history.len());
        if count == 0 { return 0.0; }
        let sum: f64 = self.history[self.history.len() - count..].iter().sum();
        sum / count as f64
    }

    /// Min/max of history
    pub fn range(&self) -> (f64, f64) {
        if self.history.is_empty() { return (0.0, 0.0); }
        let min = self.history.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = self.history.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (min, max)
    }

    /// Format value for display
    pub fn display(&self) -> String {
        format!("{:.1}{}", self.value, self.unit)
    }
}

// ============================================================================
// GPU METRICS
// ============================================================================

/// GPU performance metrics
#[derive(Debug, Clone)]
pub struct GpuMetrics {
    pub usage_percent: f64,
    pub memory_used_mb: f64,
    pub memory_total_mb: f64,
    pub temperature_c: f64,
    pub power_w: f64,
    pub clock_mhz: f64,
    pub fan_percent: f64,
    pub vram_clock_mhz: f64,
}

impl Default for GpuMetrics {
    fn default() -> Self {
        Self {
            usage_percent: 0.0,
            memory_used_mb: 0.0,
            memory_total_mb: 0.0,
            temperature_c: 0.0,
            power_w: 0.0,
            clock_mhz: 0.0,
            fan_percent: 0.0,
            vram_clock_mhz: 0.0,
        }
    }
}

impl GpuMetrics {
    /// Memory usage percentage
    pub fn memory_percent(&self) -> f64 {
        if self.memory_total_mb == 0.0 { return 0.0; }
        self.memory_used_mb / self.memory_total_mb * 100.0
    }
}

/// CPU performance metrics
#[derive(Debug, Clone)]
pub struct CpuMetrics {
    pub usage_percent: f64,
    pub frequency_mhz: f64,
    pub temperature_c: f64,
    pub power_w: f64,
}

impl Default for CpuMetrics {
    fn default() -> Self {
        Self { usage_percent: 0.0, frequency_mhz: 0.0, temperature_c: 0.0, power_w: 0.0 }
    }
}

/// RAM metrics
#[derive(Debug, Clone)]
pub struct RamMetrics {
    pub used_mb: f64,
    pub total_mb: f64,
    pub swap_used_mb: f64,
    pub swap_total_mb: f64,
}

impl Default for RamMetrics {
    fn default() -> Self {
        Self { used_mb: 0.0, total_mb: 0.0, swap_used_mb: 0.0, swap_total_mb: 0.0 }
    }
}

impl RamMetrics {
    pub fn usage_percent(&self) -> f64 {
        if self.total_mb == 0.0 { return 0.0; }
        self.used_mb / self.total_mb * 100.0
    }
}

// ============================================================================
// FRAME METRICS
// ============================================================================

/// Per-frame timing metrics
#[derive(Debug, Clone, Default)]
pub struct FrameMetrics {
    /// Current FPS
    pub fps: f64,
    /// Frame time in ms
    pub frametime_ms: f64,
    /// CPU frame time
    pub cpu_frametime_ms: f64,
    /// GPU frame time
    pub gpu_frametime_ms: f64,
    /// 1% low FPS
    pub fps_1_low: f64,
    /// 0.1% low FPS
    pub fps_01_low: f64,
    /// Frame count
    pub frame_count: u64,
}

impl FrameMetrics {
    /// Calculate FPS from frametime
    pub fn fps_from_frametime(&self) -> f64 {
        if self.frametime_ms == 0.0 { return 0.0; }
        1000.0 / self.frametime_ms
    }
}

// ============================================================================
// OVERLAY CONFIG
// ============================================================================

/// Which metrics to display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMetric {
    Fps,
    Frametime,
    Fps1Low,
    Fps01Low,
    CpuUsage,
    CpuFreq,
    CpuTemp,
    GpuUsage,
    GpuTemp,
    GpuPower,
    GpuClock,
    GpuMem,
    RamUsage,
    SwapUsage,
    VramUsage,
    NetworkRx,
    NetworkTx,
}

impl DisplayMetric {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fps => "FPS",
            Self::Frametime => "Frametime",
            Self::Fps1Low => "1% Low",
            Self::Fps01Low => "0.1% Low",
            Self::CpuUsage => "CPU",
            Self::CpuFreq => "CPU Freq",
            Self::CpuTemp => "CPU Temp",
            Self::GpuUsage => "GPU",
            Self::GpuTemp => "GPU Temp",
            Self::GpuPower => "GPU Power",
            Self::GpuClock => "GPU Clock",
            Self::GpuMem => "GPU Mem",
            Self::RamUsage => "RAM",
            Self::SwapUsage => "Swap",
            Self::VramUsage => "VRAM",
            Self::NetworkRx => "Net RX",
            Self::NetworkTx => "Net TX",
        }
    }
}

/// Overlay display position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

/// Overlay configuration
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Metrics to display
    pub metrics: Vec<DisplayMetric>,
    /// Position on screen
    pub position: OverlayPosition,
    /// Font size
    pub font_size: u32,
    /// Background opacity (0.0-1.0)
    pub background_alpha: f32,
    /// Text color (hex)
    pub text_color: String,
    /// Whether to show graphs
    pub show_graphs: bool,
    /// Graph height in pixels
    pub graph_height: u32,
    /// Update interval in ms
    pub update_interval_ms: u32,
    /// Whether overlay is enabled
    pub enabled: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            metrics: vec![
                DisplayMetric::Fps,
                DisplayMetric::Frametime,
                DisplayMetric::CpuUsage,
                DisplayMetric::GpuUsage,
                DisplayMetric::GpuTemp,
            ],
            position: OverlayPosition::TopLeft,
            font_size: 14,
            background_alpha: 0.5,
            text_color: "#00ff00".into(),
            show_graphs: true,
            graph_height: 30,
            update_interval_ms: 250,
            enabled: true,
        }
    }
}

// ============================================================================
// MANGOHUD MANAGER
//===========

/// The MangoHud overlay manager
pub struct MangoHud {
    /// Current metrics
    metrics: Mutex<HashMap<String, Metric>>,
    /// GPU metrics
    gpu: Mutex<GpuMetrics>,
    /// CPU metrics
    cpu: Mutex<CpuMetrics>,
    /// RAM metrics
    ram: Mutex<RamMetrics>,
    /// Frame metrics
    frame: Mutex<FrameMetrics>,
    /// Configuration
    config: Mutex<OverlayConfig>,
    /// Whether overlay is visible
    visible: Mutex<bool>,
    /// Toggle history
    toggle_count: Mutex<u64>,
}

impl MangoHud {
    pub fn new() -> Self {
        Self {
            metrics: Mutex::new(HashMap::new()),
            gpu: Mutex::new(GpuMetrics::default()),
            cpu: Mutex::new(CpuMetrics::default()),
            ram: Mutex::new(RamMetrics::default()),
            frame: Mutex::new(FrameMetrics::default()),
            config: Mutex::new(OverlayConfig::default()),
            visible: Mutex::new(false),
            toggle_count: Mutex::new(0),
        }
    }

    /// Register a metric
    pub fn register_metric(&self, metric: Metric) {
        self.metrics.lock().unwrap().insert(metric.name.clone(), metric);
    }

    /// Update a metric value
    pub fn update_metric(&self, name: &str, value: f64) {
        if let Some(m) = self.metrics.lock().unwrap().get_mut(name) {
            m.update(value);
        }
    }

    /// Get a metric
    pub fn get_metric(&self, name: &str) -> Option<Metric> {
        self.metrics.lock().unwrap().get(name).cloned()
    }

    /// Update GPU metrics
    pub fn update_gpu(&self, metrics: GpuMetrics) {
        *self.gpu.lock().unwrap() = metrics;
    }

    /// Update CPU metrics
    pub fn update_cpu(&self, metrics: CpuMetrics) {
        *self.cpu.lock().unwrap() = metrics;
    }

    /// Update RAM metrics
    pub fn update_ram(&self, metrics: RamMetrics) {
        *self.ram.lock().unwrap() = metrics;
    }

    /// Update frame metrics
    pub fn update_frame(&self, frametime_ms: f64) {
        let mut frame = self.frame.lock().unwrap();
        frame.frametime_ms = frametime_ms;
        frame.fps = frame.fps_from_frametime();
        frame.frame_count += 1;
    }

    /// Toggle overlay visibility
    pub fn toggle(&self) -> bool {
        let mut visible = self.visible.lock().unwrap();
        *visible = !*visible;
        *self.toggle_count.lock().unwrap() += 1;
        *visible
    }

    /// Show overlay
    pub fn show(&self) {
        *self.visible.lock().unwrap() = true;
    }

    /// Hide overlay
    pub fn hide(&self) {
        *self.visible.lock().unwrap() = false;
    }

    /// Whether overlay is visible
    pub fn is_visible(&self) -> bool {
        *self.visible.lock().unwrap()
    }

    /// Get current config
    pub fn config(&self) -> OverlayConfig {
        self.config.lock().unwrap().clone()
    }

    /// Update config
    pub fn set_config(&self, config: OverlayConfig) {
        *self.config.lock().unwrap() = config;
    }

    /// Get all current metric values for display
    pub fn current_values(&self) -> Vec<(String, String)> {
        let config = self.config.lock().unwrap();
        let gpu = self.gpu.lock().unwrap();
        let cpu = self.cpu.lock().unwrap();
        let ram = self.ram.lock().unwrap();
        let frame = self.frame.lock().unwrap();

        let mut values = Vec::new();
        for metric in &config.metrics {
            let (label, value) = match metric {
                DisplayMetric::Fps => (metric.label(), format!("{:.0}", frame.fps)),
                DisplayMetric::Frametime => (metric.label(), format!("{:.1}ms", frame.frametime_ms)),
                DisplayMetric::Fps1Low => (metric.label(), format!("{:.0}", frame.fps_1_low)),
                DisplayMetric::Fps01Low => (metric.label(), format!("{:.0}", frame.fps_01_low)),
                DisplayMetric::CpuUsage => (metric.label(), format!("{:.0}%", cpu.usage_percent)),
                DisplayMetric::CpuFreq => (metric.label(), format!("{:.0}MHz", cpu.frequency_mhz)),
                DisplayMetric::CpuTemp => (metric.label(), format!("{:.0}°C", cpu.temperature_c)),
                DisplayMetric::GpuUsage => (metric.label(), format!("{:.0}%", gpu.usage_percent)),
                DisplayMetric::GpuTemp => (metric.label(), format!("{:.0}°C", gpu.temperature_c)),
                DisplayMetric::GpuPower => (metric.label(), format!("{:.0}W", gpu.power_w)),
                DisplayMetric::GpuClock => (metric.label(), format!("{:.0}MHz", gpu.clock_mhz)),
                DisplayMetric::GpuMem => (metric.label(), format!("{:.0}/{:.0}MB", gpu.memory_used_mb, gpu.memory_total_mb)),
                DisplayMetric::RamUsage => (metric.label(), format!("{:.0}%", ram.usage_percent())),
                DisplayMetric::SwapUsage => (metric.label(), format!("{:.0}%", ram.swap_used_mb / ram.swap_total_mb.max(1.0) * 100.0)),
                DisplayMetric::VramUsage => (metric.label(), format!("{:.0}%", gpu.memory_percent())),
                DisplayMetric::NetworkRx => (metric.label(), "0 Mbps".to_string()),
                DisplayMetric::NetworkTx => (metric.label(), "0 Mbps".to_string()),
            };
            values.push((label.to_string(), value));
        }
        values
    }

    /// Generate overlay text output
    pub fn render_text(&self) -> String {
        let values = self.current_values();
        let mut out = String::new();
        for (label, value) in values {
            out.push_str(&format!("{}: {}\n", label, value));
        }
        out
    }

    /// Get toggle count (for hotkey tracking)
    pub fn toggle_count(&self) -> u64 {
        *self.toggle_count.lock().unwrap()
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.metrics.lock().unwrap().clear();
        *self.gpu.lock().unwrap() = GpuMetrics::default();
        *self.cpu.lock().unwrap() = CpuMetrics::default();
        *self.ram.lock().unwrap() = RamMetrics::default();
        *self.frame.lock().unwrap() = FrameMetrics::default();
    }
}

impl Default for MangoHud {
    fn default() -> Self { Self::new() }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_update() {
        let mut m = Metric::new("fps", "", 0.0, 240.0, true);
        m.update(60.0);
        m.update(120.0);
        m.update(90.0);
        assert_eq!(m.value, 90.0);
        assert_eq!(m.history.len(), 3);
        assert!((m.average(2) - 105.0).abs() < 0.1);
    }

    #[test]
    fn test_metric_history_limit() {
        let mut m = Metric::new("test", "", 0.0, 100.0, true);
        for i in 0..200 {
            m.update(i as f64);
        }
        assert_eq!(m.history.len(), 120);
    }

    #[test]
    fn test_gpu_metrics() {
        let gpu = GpuMetrics {
            usage_percent: 85.0,
            memory_used_mb: 8000.0,
            memory_total_mb: 16384.0,
            temperature_c: 72.0,
            power_w: 320.0,
            clock_mhz: 2500.0,
            fan_percent: 80.0,
            vram_clock_mhz: 10000.0,
        };
        assert!((gpu.memory_percent() - 48.8).abs() < 0.1);
    }

    #[test]
    fn test_ram_metrics() {
        let ram = RamMetrics {
            used_mb: 8000.0,
            total_mb: 16000.0,
            swap_used_mb: 500.0,
            swap_total_mb: 2000.0,
        };
        assert_eq!(ram.usage_percent(), 50.0);
    }

    #[test]
    fn test_frame_fps() {
        let frame = FrameMetrics {
            frametime_ms: 16.67,
            ..Default::default()
        };
        assert!((frame.fps_from_frametime() - 60.0).abs() < 1.0);
    }

    #[test]
    fn test_overlay_toggle() {
        let hud = MangoHud::new();
        assert!(!hud.is_visible());
        assert!(hud.toggle()); // now visible
        assert!(hud.is_visible());
        assert!(!hud.toggle()); // now hidden
        assert!(!hud.is_visible());
        assert_eq!(hud.toggle_count(), 2);
    }

    #[test]
    fn test_current_values() {
        let hud = MangoHud::new();
        hud.update_gpu(GpuMetrics {
            usage_percent: 75.0,
            temperature_c: 65.0,
            power_w: 250.0,
            clock_mhz: 2100.0,
            memory_used_mb: 6000.0,
            memory_total_mb: 12000.0,
            fan_percent: 60.0,
            vram_clock_mhz: 9000.0,
        });
        hud.update_frame(8.33); // ~120fps

        let values = hud.current_values();
        assert!(values.iter().any(|(l, v)| l == "FPS" && v == "120"));
        assert!(values.iter().any(|(l, v)| l == "GPU" && v == "75%"));
        assert!(values.iter().any(|(l, v)| l == "GPU Temp" && v == "65°C"));
    }

    #[test]
    fn test_render_text() {
        let hud = MangoHud::new();
        hud.update_cpu(CpuMetrics {
            usage_percent: 45.0,
            frequency_mhz: 4500.0,
            temperature_c: 55.0,
            power_w: 65.0,
        });
        let text = hud.render_text();
        assert!(text.contains("CPU"));
        assert!(text.contains("45%"));
    }

    #[test]
    fn test_config() {
        let hud = MangoHud::new();
        let mut config = hud.config();
        config.position = OverlayPosition::BottomRight;
        config.font_size = 18;
        hud.set_config(config);
        assert_eq!(hud.config().position, OverlayPosition::BottomRight);
        assert_eq!(hud.config().font_size, 18);
    }
}
