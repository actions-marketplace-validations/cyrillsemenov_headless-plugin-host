use hph_core::{ExtensionProbeResult, HostDiagnostics, PluginDescriptorInfo};
use hph_gui::GuiBenchmarkReport;
use hph_rt_check::RtAllocStats;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheckResult {
    pub category: String,
    pub name: String,
    pub passed: bool,
    pub duration_ms: f64,
    pub details: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub descriptor: PluginDescriptorInfo,
    pub total_checks: usize,
    pub passed_checks: usize,
    pub failed_checks: usize,
    pub checks: Vec<ValidationCheckResult>,
    pub extensions: Vec<ExtensionProbeResult>,
    pub diagnostics: HostDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DspBenchBlockStats {
    pub sample_rate: f64,
    pub block_size: usize,
    pub blocks_processed: usize,
    pub mean_us: f64,
    pub p50_us: f64,
    pub p90_us: f64,
    pub p99_us: f64,
    pub max_us: f64,
    pub buffer_deadline_us: f64,
    pub cpu_load_percent: f64,
    pub realtime_speedup_x: f64,
    pub rt_allocs: RtAllocStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub descriptor: PluginDescriptorInfo,
    pub dsp_configs: Vec<DspBenchBlockStats>,
    pub denormal_slowdown_ratio: f64,
    pub gui_benchmark: Option<GuiBenchmarkReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzConfig {
    pub seed: u64,
    pub iterations: usize,
    pub max_block_size: usize,
    pub corrupt_state: bool,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        Self {
            seed: 0xDEAD_BEEF_CAFE_BABE,
            iterations: 250,
            max_block_size: 1024,
            corrupt_state: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzFailure {
    pub iteration: usize,
    pub sample_rate: f64,
    pub block_size: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzReport {
    pub descriptor: PluginDescriptorInfo,
    pub seed: u64,
    pub iterations_completed: usize,
    pub total_frames_processed: u64,
    pub total_events_dispatched: u64,
    pub state_mutations_tested: usize,
    pub max_abs_sample: f64,
    pub nan_inf_samples: usize,
    pub runaway_samples: usize,
    pub rt_allocs: RtAllocStats,
    pub duration_ms: f64,
    pub passed: bool,
    pub failures: Vec<FuzzFailure>,
}
