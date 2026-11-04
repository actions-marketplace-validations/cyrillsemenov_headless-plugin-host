pub mod platform;
pub mod png;

pub use png::save_rgba_png;

use hph_core::{HostResult, PluginInstance};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiScreenshotResult {
    pub width: u32,
    pub height: u32,
    pub output_path: String,
    pub create_time_us: f64,
    pub show_time_us: f64,
    pub warmup_ms: u64,
    pub capture_time_us: f64,
    pub non_blank_pixels: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiBenchmarkReport {
    pub supported: bool,
    pub initial_size: (u32, u32),
    pub can_resize: bool,
    pub create_latency_us: f64,
    pub set_parent_latency_us: f64,
    pub show_latency_us: f64,
    pub avg_frame_pump_us: f64,
    pub max_frame_pump_us: f64,
    pub resize_avg_latency_us: Option<f64>,
    pub capture_rgba_latency_us: f64,
}

pub fn capture_plugin_screenshot(
    plugin: &mut dyn PluginInstance,
    output_path: &Path,
    warmup_ms: u64,
    custom_size: Option<(u32, u32)>,
) -> HostResult<GuiScreenshotResult> {
    #[cfg(target_os = "macos")]
    return platform::macos::capture_plugin_screenshot(plugin, output_path, warmup_ms, custom_size);

    #[cfg(target_os = "windows")]
    return platform::windows::capture_plugin_screenshot(
        plugin,
        output_path,
        warmup_ms,
        custom_size,
    );

    #[cfg(target_os = "linux")]
    return platform::linux::capture_plugin_screenshot(plugin, output_path, warmup_ms, custom_size);

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (plugin, output_path, warmup_ms, custom_size);
        Err(hph_core::HostError::Unsupported(
            "Offscreen GUI capture is currently supported on macOS, Windows, and Linux".into(),
        ))
    }
}

pub fn benchmark_plugin_gui(
    plugin: &mut dyn PluginInstance,
    frames_to_pump: usize,
) -> HostResult<GuiBenchmarkReport> {
    #[cfg(target_os = "macos")]
    return platform::macos::benchmark_plugin_gui(plugin, frames_to_pump);

    #[cfg(target_os = "windows")]
    return platform::windows::benchmark_plugin_gui(plugin, frames_to_pump);

    #[cfg(target_os = "linux")]
    return platform::linux::benchmark_plugin_gui(plugin, frames_to_pump);

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (plugin, frames_to_pump);
        Ok(GuiBenchmarkReport {
            supported: false,
            initial_size: (0, 0),
            can_resize: false,
            create_latency_us: 0.0,
            set_parent_latency_us: 0.0,
            show_latency_us: 0.0,
            avg_frame_pump_us: 0.0,
            max_frame_pump_us: 0.0,
            resize_avg_latency_us: None,
            capture_rgba_latency_us: 0.0,
        })
    }
}
