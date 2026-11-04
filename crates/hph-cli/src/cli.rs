use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
#[cfg(target_os = "macos")]
use hph_au::AuBundle;
use hph_clap::ClapBundle;
use hph_core::*;
use hph_dsp::*;
use hph_vst3::Vst3Bundle;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn load_plugin_bundle(path: &Path) -> HostResult<Arc<dyn PluginBundle>> {
    #[cfg(not(target_os = "macos"))]
    {
        let path_str = path.to_string_lossy().to_ascii_lowercase();
        if path_str.ends_with(".component")
            || path_str.contains(".component/")
            || path_str.contains(".component\\")
        {
            return Err(HostError::Unsupported(
                "Audio Unit (.component) plugins are only supported on macOS".into(),
            ));
        }
    }
    let loaders: &[FormatLoader] = &[
        ClapBundle::LOADER,
        Vst3Bundle::LOADER,
        #[cfg(target_os = "macos")]
        AuBundle::LOADER,
    ];
    load_bundle_with_loaders(path, loaders)
}

#[derive(Parser, Debug)]
#[command(
    name = "hph",
    version,
    about = "Fast headless plugin validation, profiling, screenshot & DSP analysis host"
)]
pub struct Cli {
    /// Emit structured JSON output instead of human-readable tables
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inspect plugin descriptors, audio/note ports, parameters, and extensions
    Inspect {
        /// Path to `.clap` bundle or binary
        plugin: PathBuf,
        /// Optional plugin ID if the bundle contains multiple plugins
        #[arg(long)]
        id: Option<String>,
    },
    /// Run automated validation suite (ABI, extensions, params, state roundtrip, block invariance, RT-allocs, denormals)
    Validate {
        /// Path to `.clap` bundle or binary
        plugin: PathBuf,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, default_value_t = 48000.0)]
        sample_rate: f64,
    },
    /// Benchmark DSP processing across buffer sizes & sample rates, plus optional GUI profiling
    Bench {
        /// Path to `.clap` bundle or binary
        plugin: PathBuf,
        #[arg(long)]
        id: Option<String>,
        /// Number of blocks to process per configuration
        #[arg(long, default_value_t = 250)]
        blocks: usize,
        /// Also benchmark GUI creation, frame pumping, resize, and capture
        #[arg(long)]
        gui: bool,
    },
    /// Generate test signals or play an input WAV file through the plugin, measure 4x True-Peak/RMS, and render to WAV
    Render {
        /// Path to `.clap` bundle or binary
        plugin: PathBuf,
        #[arg(long)]
        id: Option<String>,
        /// Signal generator type (ignored if --input-wav is given)
        #[arg(long, value_enum, default_value_t = CliSignalType::Sine)]
        signal: CliSignalType,
        /// Signal frequency in Hz (for sine/saw/square/triangle/triggers)
        #[arg(long, default_value_t = 1000.0)]
        freq: f64,
        /// Signal amplitude (0.0 .. 1.0)
        #[arg(long, default_value_t = 0.5)]
        amp: f64,
        /// Duration in seconds (when generating signal)
        #[arg(long, default_value_t = 1.0)]
        duration: f64,
        /// Sample rate in Hz
        #[arg(long, default_value_t = 48000)]
        sample_rate: u32,
        /// Processing block size in frames
        #[arg(long, default_value_t = 256)]
        block_size: usize,
        /// Optional input WAV file to process through the plugin
        #[arg(short = 'i', long)]
        input_wav: Option<PathBuf>,
        /// Optional output WAV file path to write rendered audio
        #[arg(short = 'o', long)]
        output_wav: Option<PathBuf>,
        /// Parameter overrides in `ID=VALUE` format (can be specified multiple times)
        #[arg(short = 'p', long = "param")]
        params: Vec<String>,
        /// Trigger a MIDI/CLAP note key (0..127) at sample 0
        #[arg(long)]
        note: Option<i16>,
    },
    /// Run precision DSP analysis: Spectrum, Bode/Group-Delay, THD+N, Two-Tone IMD, RT60 Reverb, Delay/Latency, Dynamics, or Stereo
    Analyze {
        /// Path to plugin bundle or binary
        plugin: PathBuf,
        #[arg(long)]
        id: Option<String>,
        #[arg(value_enum, default_value_t = AnalysisMode::All)]
        mode: AnalysisMode,
        #[arg(long, default_value_t = 48000.0)]
        sample_rate: f64,
        /// Fundamental test frequency in Hz for Spectrum and THD+N
        #[arg(long, default_value_t = 1000.0)]
        freq: f64,
        /// Tail capture duration in seconds for RT60 and Delay analysis
        #[arg(long, default_value_t = 3.0)]
        tail_sec: f64,
        /// Parameter overrides in `ID=VALUE` format
        #[arg(short = 'p', long = "param")]
        params: Vec<String>,
    },
    /// Run deterministic, seeded chaos / fuzz torture test (prime block sizes, SR hopping, zipper storms, subnormals, overdrive, state corruption)
    Fuzz {
        /// Path to plugin bundle or binary
        plugin: PathBuf,
        #[arg(long)]
        id: Option<String>,
        /// Number of fuzz blocks/iterations to run
        #[arg(long, default_value_t = 300)]
        iterations: usize,
        /// 64-bit PRNG seed for deterministic reproduction
        #[arg(long, default_value_t = 0xDEAD_BEEF_CAFE_BABE)]
        seed: u64,
        /// Maximum block size during fuzzing
        #[arg(long, default_value_t = 1024)]
        max_block_size: usize,
        /// Also fuzz `load_state()` with truncated and bit-flipped state buffers
        #[arg(long)]
        corrupt_state: bool,
    },
    /// Open the plugin's GUI offscreen, pump runloop & timers, and save a PNG screenshot
    Screenshot {
        /// Path to `.clap` bundle or binary
        plugin: PathBuf,
        #[arg(long)]
        id: Option<String>,
        /// Output PNG file path
        #[arg(short = 'o', long)]
        output: PathBuf,
        /// Warmup time in milliseconds to pump main runloop and plugin timers before capture
        #[arg(long, default_value_t = 200)]
        warmup_ms: u64,
        /// Optional custom width
        #[arg(long)]
        width: Option<u32>,
        /// Optional custom height
        #[arg(long)]
        height: Option<u32>,
    },
    /// Scaffold, edit in-place, inspect, or run a declarative TOML test suite (`suite.toml`)
    Suite {
        #[command(subcommand)]
        action: SuiteCommand,
    },
    /// Generate a print-ready white HTML + vector SVG engineering datasheet for a plugin
    Datasheet {
        /// Path to `.clap`, `.vst3`, or `.component` plugin bundle or binary
        plugin: PathBuf,
        #[arg(long)]
        id: Option<String>,
        /// Output HTML file path
        #[arg(short = 'o', long, default_value = "datasheet.html")]
        output: PathBuf,
        #[arg(long, default_value_t = 48000.0)]
        sample_rate: f64,
        /// Optional parameter ID to sweep across 5 values on Bode & Dynamics plots
        #[arg(long)]
        sweep_param: Option<u32>,
        /// Optional reference plugin bundle for A/B overlay comparison
        #[arg(long)]
        reference: Option<PathBuf>,
        /// Parameter overrides in `ID=VALUE` format
        #[arg(short = 'p', long = "param")]
        params: Vec<String>,
    },
    /// Start the Model Context Protocol (MCP) JSON-RPC 2.0 server over stdio
    Mcp,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum CliSignalType {
    Silence,
    Sine,
    Saw,
    Square,
    Triangle,
    Impulse,
    Triggers,
    WhiteNoise,
    PinkNoise,
    Subnormals,
    Sweep,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum AnalysisMode {
    Spectrum,
    Bode,
    Thdn,
    Imd,
    Rt60,
    Delay,
    Dynamics,
    Stereo,
    All,
}

#[derive(Debug, Serialize)]
pub struct InspectOutput {
    pub bundle_descriptors: Vec<PluginDescriptorInfo>,
    pub active_plugin: PluginDescriptorInfo,
    pub extensions: Vec<ExtensionProbeResult>,
    pub audio_ports: Vec<AudioPortInfo>,
    pub note_ports: Vec<NotePortInfo>,
    pub parameters: Vec<ParamInfo>,
    pub latency_samples: Option<u32>,
    pub tail_samples: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct RenderOutput {
    pub plugin: String,
    pub sample_rate: u32,
    pub num_frames: usize,
    pub output_wav: Option<String>,
    pub rt_allocs: hph_rt_check::RtAllocStats,
    pub channels: Vec<ChannelMeterReport>,
}

#[derive(Debug, Serialize)]
pub struct AnalyzeOutput {
    pub plugin: String,
    pub sample_rate: f64,
    pub spectrum: Option<SpectrumReport>,
    pub bode: Option<BodeReport>,
    pub thdn: Option<ThdNReport>,
    pub imd: Option<ImdReport>,
    pub rt60: Option<Rt60Report>,
    pub delay: Option<DelayReport>,
    pub dynamics: Option<DynamicsReport>,
    pub stereo: Option<StereoReport>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SuiteCommand {
    /// Scaffold a new commented `suite.toml` manifest for a plugin
    Init {
        /// Path to `.clap`, `.vst3`, or `.component` plugin bundle or binary
        plugin: PathBuf,
        #[arg(long)]
        id: Option<String>,
        /// Output `.toml` manifest path
        #[arg(short = 'o', long, default_value = "suite.toml")]
        output: PathBuf,
        #[arg(long, default_value_t = 48000.0)]
        sample_rate: f64,
        /// Optional datasheet HTML output path to configure in the manifest
        #[arg(long)]
        datasheet: Option<String>,
        /// Run the plugin now and auto-populate baseline assertion thresholds
        #[arg(long)]
        capture: bool,
    },
    /// Add or update a `[[tests]]` step in-place (preserving all comments)
    Add {
        /// Path to `.toml` suite manifest
        #[arg(short = 'f', long = "file", default_value = "suite.toml")]
        suite_file: PathBuf,
        /// Name of the test step
        #[arg(short = 'n', long)]
        name: String,
        /// Parameter overrides in `NAME=VALUE` or `ID=VALUE` format
        #[arg(short = 'p', long = "param")]
        params: Vec<String>,
        /// Stimulus signal type
        #[arg(long, value_enum, default_value_t = CliSignalType::Sine)]
        signal: CliSignalType,
        #[arg(long, default_value_t = 1000.0)]
        freq: f64,
        #[arg(long, default_value_t = 0.5)]
        amp: f64,
        #[arg(long, default_value_t = 48000)]
        frames: usize,
        /// Explicit assertion overrides in `KEY=VALUE` format (e.g. `max_thd_percent=1.0`)
        #[arg(short = 'a', long = "assert")]
        assertions: Vec<String>,
        #[arg(long)]
        output_wav: Option<String>,
        #[arg(long)]
        screenshot_png: Option<String>,
        /// Run the plugin now and auto-populate baseline assertion thresholds
        #[arg(long)]
        capture: bool,
    },
    /// Remove a `[[tests]]` step by name in-place
    Remove {
        /// Path to `.toml` suite manifest
        #[arg(short = 'f', long = "file", default_value = "suite.toml")]
        suite_file: PathBuf,
        /// Name of the test step to remove
        #[arg(short = 'n', long)]
        name: String,
    },
    /// Modify top-level or step-level manifest keys in-place
    Set {
        /// Path to `.toml` suite manifest
        #[arg(short = 'f', long = "file", default_value = "suite.toml")]
        suite_file: PathBuf,
        /// Target a specific `[[tests]]` step by name (omit to edit top-level keys)
        #[arg(short = 's', long)]
        step: Option<String>,
        /// Assignments in `KEY=VALUE` format (e.g. `sample_rate=96000` or `param.Cutoff=8000`)
        sets: Vec<String>,
        /// Keys to remove (`--unset max_imd_percent` or `--unset param.Cutoff`)
        #[arg(long = "unset")]
        unsets: Vec<String>,
    },
    /// Inspect the steps, parameters, and assertions configured in a suite manifest
    Show {
        /// Path to `.toml` suite manifest
        #[arg(default_value = "suite.toml")]
        suite_file: PathBuf,
    },
    /// Execute a declarative TOML test suite and verify all assertions
    Run {
        /// Path to `.toml` suite manifest
        #[arg(default_value = "suite.toml")]
        suite_file: PathBuf,
        /// Re-capture and update assertion bounds in-place from live measurements
        #[arg(long)]
        update_baselines: bool,
        /// Optional step name filter when updating baselines
        #[arg(long)]
        step: Option<String>,
    },
}

pub fn parse_param_events(param_args: &[String]) -> Result<Vec<ProcessEvent>> {
    let mut events = Vec::new();
    for item in param_args {
        let (k, v) = item
            .split_once('=')
            .with_context(|| format!("Invalid --param '{item}', expected ID=VALUE"))?;
        let param_id: u32 = k
            .trim()
            .parse()
            .with_context(|| format!("Invalid param ID '{k}'"))?;
        let value: f64 = v
            .trim()
            .parse()
            .with_context(|| format!("Invalid param value '{v}'"))?;
        events.push(ProcessEvent::ParamValue {
            sample_offset: 0,
            param_id,
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value,
        });
    }
    Ok(events)
}
