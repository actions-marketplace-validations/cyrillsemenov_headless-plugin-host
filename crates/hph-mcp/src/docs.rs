use serde_json::{json, Value};

pub const MCP_INSTRUCTIONS: &str = r#"# Headless Plugin Host (`hph`) — Agent MCP Instructions

Use this MCP server whenever you are building, debugging, validating, profiling, or testing audio plugins in **CLAP (`.clap`)**, **VST3 (`.vst3`)**, or **Audio Unit v2 (`.component`)** formats on macOS, Windows, or Linux.

## Recommended Agent Workflow
1. **`hph_inspect` first**: Always inspect the compiled plugin bundle first to discover its exact parameter IDs (`id`), min/max/default ranges, audio/note port counts, and reported latency before passing `params` (`["ID=VALUE"]`) to other tools.
2. **`hph_validate` after every build**: Run the 8-check automated validation suite to verify ABI vtables, port topology, parameter bounds/text formatting, state save/load determinism, variable block-size invariance (`1, 17, 64, 512`), subnormal/denormal & NaN/Inf health, zero real-time audio-thread heap allocations, and bypass/zipper-noise behavior.
3. **`hph_analyze` to verify DSP math**:
   - **Filters / EQs**: Use `mode: "bode"` to verify DC gain, 1 kHz gain, `-3 dB` cutoff frequency (`cutoff_minus_3db_hz`), phase response, and unwrapped group delay (`group_delay_ms`).
   - **Saturators / Distortion / Waveshapers**: Use `mode: "thdn"` (measures THD%, Even vs. Odd harmonic balance `even_thd_db` / `odd_thd_db`, H2..H10 spurs, and `max_alias_spur_dbc`) and `mode: "imd"` (two-tone 19 kHz + 20 kHz CCIF & SMPTE intermodulation).
   - **Reverbs**: Use `mode: "rt60"` to measure ISO 3382 Schroeder backward-integrated `rt60_sec`, `edt_sec`, `t20_sec`, `t30_sec`, decay linearity `r_squared` (`> 0.95` indicates clean exponential decay), and 3 octave bands (`125 Hz`, `1000 Hz`, `4000 Hz`).
   - **Delays / Lookahead / Oversampling**: Use `mode: "delay"` to measure impulse onset, sub-sample GCC-PHAT cross-correlation delay, PDC discrepancy (`latency_discrepancy_samples` comparing actual acoustic delay vs. `latency_samples()`), and multi-tap echo spacing (`ms`) & feedback decay (`dB/tap`).
   - **Compressors / Limiters**: Use `mode: "dynamics"` to measure static compression ratio, threshold knee (`dBFS`), max gain reduction (`dB`), and tone-burst `10%..90%` attack & release times (`ms`).
   - **Stereo / Chorus / Pan / Widener**: Use `mode: "stereo"` to measure phase correlation (`-1.0` out-of-phase to `+1.0` mono), L/R balance (`dB`), Mid/Side RMS (`dBFS`), and stereo width.
4. **`hph_fuzz` for chaos stress-testing**: Run `hph_fuzz` (`corrupt_state: true`) to hammer the plugin with prime block sizes (`1..4096`), sample-rate hopping, sample-accurate parameter zipper storms, `+18 dBFS` overdrive, IEEE-754 subnormals, and corrupted state loads. Every run uses a deterministic `seed` so any failure is 100% reproducible.
5. **`hph_screenshot` for visual GUI verification**: Capture an offscreen PNG render (`Cocoa` `NSView` on macOS, `Win32` `HWND` on Windows, `X11` `Window` on Linux) and view the resulting image file to verify layout, controls, and meters.
6. **Need deeper reference?**: Call `hph_usage_guide` (or read resource `hph://docs/guide`) at any time for detailed metric interpretation rules and TOML test suite schemas."#;

pub fn usage_guide_markdown(topic: &str) -> String {
    match topic.trim().to_ascii_lowercase().as_str() {
        "workflow" => r#"# `hph` Agent Workflow Playbook

1. **Discover (`hph_inspect`)**:
   - Call `hph_inspect` with `{"plugin": "/path/to/MyPlugin.clap"}`.
   - Record the parameter `id` numbers (e.g. `100`, `101`) and `[min_value .. max_value]` ranges.
2. **Validate (`hph_validate`)**:
   - Call `hph_validate` with `{"plugin": "/path/to/MyPlugin.clap"}`.
   - Ensure all 8 checks pass (`failed_checks == 0`) and check `warnings` for subnormal leaks or parameter step discontinuities (`max_slew_rate > 0.75`).
3. **Measure DSP (`hph_analyze` / `hph_render`)**:
   - Pass parameter overrides as `"params": ["101=2000.0", "102=0.75"]`.
   - Compare measured cutoff, THD, IMD, RT60, or delay against your DSP design targets.
4. **Torture Test (`hph_fuzz`)**:
   - Call `hph_fuzz` with `{"plugin": "/path/to/MyPlugin.clap", "iterations": 300, "corrupt_state": true}`.
   - Because `hph mcp` isolates plugin execution in a subprocess, even a C/C++ `SIGSEGV` will be caught cleanly and reported back to you without crashing the MCP server.
5. **Visual Check (`hph_screenshot`)**:
   - Call `hph_screenshot` with `{"plugin": "/path/to/MyPlugin.clap", "output": "/tmp/plugin_gui.png", "warmup_ms": 250}`."#
            .to_string(),

        "measurements" => r#"# Interpreting `hph` DSP Measurements

- **4x Oversampled True-Peak (`true_peak_4x_dbtp`) & `intersample_overshoot_db`**:
  - Reconstructs continuous inter-sample peaks at `+0.00, +0.25, +0.50, +0.75` fractional sample phases using a 64-tap Kaiser-windowed sinc polyphase FIR filter.
  - Can exceed `sample_peak_dbfs` by up to `+3.01 dB` on high-frequency tones near $f_s / 4$.
- **K-Weighted Loudness (`k_weighted_lufs`)**:
  - ITU-R BS.1770-4 2-stage K-weighting cascade (High-shelf pre-filter + RLB high-pass). A `0 dBFS` (`1.0` peak) 1 kHz sine wave measures `-3.01 LUFS` per channel.
- **THD+N (`thdn`) & Even/Odd Harmonics**:
  - `even_thd_db` ($H_2, H_4, H_6, H_8, H_{10}$): Indicates asymmetric waveshaping (tube/triode warmth or DC bias).
  - `odd_thd_db` ($H_3, H_5, H_7, H_9$): Indicates symmetric saturation (`tanh`, soft-clip, tape/transistor).
  - `max_alias_spur_dbc`: Highest non-harmonic bin spur relative to fundamental. If `> -60 dBc`, the saturator likely needs oversampling or ADAA.
- **Two-Tone IMD (`imd`)**:
  - Feeds `19 kHz + 20 kHz` (clamped below Nyquist) and measures the `1 kHz` difference tone (`diff_tone_dbc`, IMD2) and `18 kHz` / `21 kHz` third-order products (`third_order_lower_dbc`, IMD3). Catches high-frequency foldback that single-tone THD misses.
- **Reverb `rt60` (ISO 3382 Schroeder EDC)**:
  - Uses backward integration of $h^2[n]$ with automatic tail-noise truncation and direct dry-spike compensation (so a 50% dry/wet mix measures the same wet decay slope as 100% wet).
  - Check `r_squared`: values `>= 0.95` indicate a smooth exponential decay; low `r_squared` or growing peaks indicate comb resonance buildup or gated/modulated tails.
- **Acoustic `delay` & PDC Audit**:
  - `onset_delay_samples`: First arrival sample (`-40 dBc`).
  - `peak_delay_samples` / `xcorr_delay_samples`: Sub-sample interpolated peak and blended GCC-PHAT cross-correlation lag.
  - `latency_discrepancy_samples`: `onset_delay_samples - reported_latency_samples`. Non-zero values on a dry pass-through indicate unreported lookahead or oversampling FIR latency.
  - `echo_taps`: Lists up to 16 discrete echo reflections with `estimated_tap_spacing_ms` and `estimated_feedback_db_per_tap`."#
            .to_string(),

        "params" => r#"# Passing Parameter Overrides (`params`)

1. Call `hph_inspect` to list all plugin parameters:
   - For **CLAP**, `id` is the 32-bit `clap_id` (often `MODULE_OFFSET + pid` or `0, 1, 2...`).
   - For **VST3**, `id` is the `Steinberg::Vst::ParamID`, and `min_value..max_value` are in plain units (or `0.0..1.0` normalized).
   - For **AUv2 (`.component`)**, `id` is the `AudioUnitParameterID` in `kAudioUnitScope_Global`.
2. Pass overrides as an array of `"ID=VALUE"` strings:
   ```json
   {
     "plugin": "/path/to/Plugin.clap",
     "mode": "bode",
     "params": ["101=1200.0", "102=0.0"]
   }
   ```
3. All parameter events are injected sample-accurately at `sample_offset = 0` before analysis begins."#
            .to_string(),

        "fuzz" => r#"# Deterministic Seeded Fuzz Torture (`hph_fuzz`)

`hph_fuzz` exercises the plugin under extreme, deterministic chaos driven by a 64-bit `SplitMix64` seed:
- **Variable & Prime Block Sizes**: `1, 3, 7, 13, 31, 63, 64, 127, 251, 509, 1024`, plus uniform random block lengths.
- **Sample-Rate Hopping**: Periodically deactivates and re-activates across `44.1 kHz, 48 kHz, 88.2 kHz, 96 kHz`.
- **Sample-Accurate Zipper Storms**: Random `ParamValue` jumps (biased toward `min_value`, `max_value`, `default_value`, and random interior points) at arbitrary sample offsets within each block.
- **Adversarial Signals**: Cycles between white noise, `+24 dBFS` (`±16.0`) rail overdrive, IEEE-754 subnormals (`±1e-39`), Nyquist alternating square waves, hard DC steps, and silence.
- **Corrupted State Fuzzing (`corrupt_state: true`)**: Periodically feeds truncated, bit-flipped, and empty byte slices into `load_state()`, then restores valid state to verify the state deserializer never panics or corrupts memory.
- **Invariants Enforced**: Zero `NaN`/`Inf` samples, zero runaway filter blowups (`|y| > 1e4` / `+80 dBFS`), zero real-time heap allocations, and zero thread-check violations."#
            .to_string(),

        "suite-toml" => r#"# Declarative `.toml` Regression Suite Schema

You can save a declarative regression test suite to a `.toml` file and run it via `hph suite <file.toml>`:

```toml
plugin = "target/debug/libmy_plugin.dylib"
sample_rate = 48000.0
run_validation = true

[[tests]]
name = "Clean 1kHz Sine Pass-Through"
frames = 48000
params = { "101" = 18000.0, "102" = 0.0 }
signal = { type = "sine", freq_hz = 1000.0, amplitude = 0.5, phase_rad = 0.0 }
max_true_peak_dbtp = 0.5
min_rms_dbfs = -12.0
min_gain_db = -0.5
max_gain_db = 0.5
max_thd_percent = 0.5
max_imd_percent = 1.0
min_stereo_correlation = 0.99
require_zero_rt_allocs = true

[[tests]]
name = "Hot +24dBFS Overdrive Saturation"
frames = 48000
params = { "101" = 18000.0, "102" = 0.8 }
signal = { type = "sine", freq_hz = 1000.0, amplitude = 15.848932, phase_rad = 0.0 }
max_true_peak_dbtp = 2.0
min_rms_dbfs = -4.0
max_gain_db = -18.0
min_thd_percent = 20.0
max_dc_offset_dbfs = -30.0
require_zero_rt_allocs = true

[[tests]]
name = "Reverb Tail Decay Check"
frames = 144000
params = { "0" = 1.0, "6" = 0.0 }
signal = { type = "impulse", offset_samples = 0, amplitude = 0.8 }
min_rt60_sec = 1.0
max_rt60_sec = 2.5
require_zero_rt_allocs = true
```"#
            .to_string(),

        _ => format!(
            "{MCP_INSTRUCTIONS}\n\n---\n\n{}\n\n---\n\n{}\n\n---\n\n{}",
            usage_guide_markdown("workflow"),
            usage_guide_markdown("measurements"),
            usage_guide_markdown("suite-toml")
        ),
    }
}

pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "hph_usage_guide",
            "description": "Returns the comprehensive agent usage guide and DSP measurement interpretation handbook for headless-plugin-host (`hph`). Call this first if you are unsure which analysis mode to use or how to interpret RT60, IMD, True-Peak, LUFS, or Delay/Latency results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "topic": {
                        "type": "string",
                        "enum": ["all", "workflow", "measurements", "params", "fuzz", "suite-toml"],
                        "description": "Specific documentation section to retrieve (default: 'all')."
                    }
                }
            }
        },
        {
            "name": "hph_inspect",
            "description": "Inspect a `.clap`, `.vst3`, or `.component` (AUv2) audio plugin bundle. Returns plugin descriptors, supported extensions, audio/note port layouts, all parameters (IDs, names, min/max/default values, formatted text), and reported latency/tail samples.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin": {
                        "type": "string",
                        "description": "Path to the `.clap`, `.vst3`, or `.component` plugin bundle or binary."
                    },
                    "id": {
                        "type": "string",
                        "description": "Optional plugin ID if the bundle contains multiple plugins."
                    }
                },
                "required": ["plugin"]
            }
        },
        {
            "name": "hph_validate",
            "description": "Run the 8-check automated plugin validation suite: (1) ABI & Extension VTables, (2) Audio/Note Port Topology, (3) Parameter Bounds & Text Formatting, (4) State Save/Load Roundtrip & Reset Determinism, (5) Variable Block-Size Invariance (1, 17, 64, 512), (6) Subnormal/Denormal & NaN/Inf Sanitizer, (7) Zero Real-Time Audio-Thread Heap Allocation & Thread-Check Audit, and (8) Bypass Null-Test & Parameter Zipper-Noise Audit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin": {
                        "type": "string",
                        "description": "Path to the `.clap`, `.vst3`, or `.component` plugin."
                    },
                    "id": {
                        "type": "string",
                        "description": "Optional plugin ID if the bundle contains multiple plugins."
                    },
                    "sample_rate": {
                        "type": "number",
                        "description": "Sample rate in Hz (default: 48000.0)."
                    }
                },
                "required": ["plugin"]
            }
        },
        {
            "name": "hph_analyze",
            "description": "Run precision DSP measurements on a plugin: `spectrum` (FFT peak, centroid, flatness), `bode` (frequency response, -3dB cutoff, phase, unwrapped group delay in ms), `thdn` (THD%, Even vs. Odd THD dB, H2..H10 harmonics, max alias spur dBc), `imd` (two-tone CCIF/SMPTE intermodulation distortion), `rt60` (ISO 3382 Schroeder backward-integrated EDT, T20, T30, RT60, R^2, and 125Hz/1kHz/4kHz octave bands), `delay` (impulse onset, sub-sample GCC-PHAT delay, PDC latency discrepancy, and multi-tap echo finder), `dynamics` (compressor ratio, threshold, attack/release ms), `stereo` (phase correlation, L/R balance, Mid/Side RMS, stereo width), or `all`.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin": {
                        "type": "string",
                        "description": "Path to the `.clap`, `.vst3`, or `.component` plugin."
                    },
                    "id": {
                        "type": "string",
                        "description": "Optional plugin ID."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["all", "spectrum", "bode", "thdn", "imd", "rt60", "delay", "dynamics", "stereo"],
                        "description": "Analysis mode to run (default: 'all')."
                    },
                    "sample_rate": {
                        "type": "number",
                        "description": "Sample rate in Hz (default: 48000.0)."
                    },
                    "freq": {
                        "type": "number",
                        "description": "Fundamental test frequency in Hz for `spectrum` and `thdn` (default: 1000.0)."
                    },
                    "tail_sec": {
                        "type": "number",
                        "description": "Impulse response capture duration in seconds for `rt60` and `delay` (default: 3.0)."
                    },
                    "params": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional parameter overrides in `'ID=VALUE'` format, e.g. `[\"101=2000.0\", \"102=0.5\"]`."
                    }
                },
                "required": ["plugin"]
            }
        },
        {
            "name": "hph_fuzz",
            "description": "Run a deterministic, seeded chaos / fuzz torture test against a plugin. Exercises prime/odd block sizes (1..4096), sample-rate hopping, sample-accurate parameter zipper storms, polyphonic note bursts, extreme +18 dBFS overdrive, IEEE-754 subnormals, Nyquist squares, DC steps, and optional corrupted/truncated `load_state()` calls while asserting zero crashes, zero NaNs/Infs, bounded output (< +80 dBFS), and zero RT heap allocations.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin": {
                        "type": "string",
                        "description": "Path to the `.clap`, `.vst3`, or `.component` plugin."
                    },
                    "id": {
                        "type": "string",
                        "description": "Optional plugin ID."
                    },
                    "iterations": {
                        "type": "integer",
                        "description": "Number of fuzz iterations/blocks to run (default: 300)."
                    },
                    "seed": {
                        "type": "integer",
                        "description": "64-bit PRNG seed for 100% deterministic reproduction."
                    },
                    "max_block_size": {
                        "type": "integer",
                        "description": "Maximum audio block size in frames (default: 1024)."
                    },
                    "corrupt_state": {
                        "type": "boolean",
                        "description": "Also fuzz `load_state()` with truncated and bit-flipped state buffers (default: true)."
                    }
                },
                "required": ["plugin"]
            }
        },
        {
            "name": "hph_bench",
            "description": "Benchmark DSP processing latency (µs percentiles, CPU load %, real-time speedup factor) across sample rates (44.1, 48, 96 kHz) and buffer sizes (64..1024), measure the Denormal CPU Slowdown Ratio, and optionally benchmark offscreen GUI creation/frame-pump/capture latency.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin": {
                        "type": "string",
                        "description": "Path to the `.clap`, `.vst3`, or `.component` plugin."
                    },
                    "id": {
                        "type": "string",
                        "description": "Optional plugin ID."
                    },
                    "blocks": {
                        "type": "integer",
                        "description": "Number of blocks to process per configuration (default: 200)."
                    },
                    "gui": {
                        "type": "boolean",
                        "description": "Also benchmark offscreen GUI creation, 60Hz frame pumping, resize, and RGBA capture (default: false)."
                    }
                },
                "required": ["plugin"]
            }
        },
        {
            "name": "hph_render",
            "description": "Generate a test signal (or read an input WAV file), process it through the plugin, measure 4x Oversampled True-Peak (dBTP), 4x RMS (dBFS), ITU-R BS.1770-4 K-weighted Loudness (LUFS), Crest factor, DC offset, slew rate, and RT allocations, and optionally write the output to a WAV file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin": {
                        "type": "string",
                        "description": "Path to the `.clap`, `.vst3`, or `.component` plugin."
                    },
                    "id": {
                        "type": "string",
                        "description": "Optional plugin ID."
                    },
                    "signal": {
                        "type": "string",
                        "enum": ["silence", "sine", "saw", "square", "triangle", "impulse", "triggers", "white-noise", "pink-noise", "subnormals", "sweep"],
                        "description": "Test signal generator type (default: 'sine')."
                    },
                    "freq": {
                        "type": "number",
                        "description": "Signal frequency in Hz (default: 1000.0)."
                    },
                    "amp": {
                        "type": "number",
                        "description": "Signal amplitude 0.0..1.0 (default: 0.5)."
                    },
                    "duration": {
                        "type": "number",
                        "description": "Duration in seconds (default: 1.0)."
                    },
                    "sample_rate": {
                        "type": "integer",
                        "description": "Sample rate in Hz (default: 48000)."
                    },
                    "block_size": {
                        "type": "integer",
                        "description": "Processing block size in frames (default: 256)."
                    },
                    "input_wav": {
                        "type": "string",
                        "description": "Optional input WAV file path (overrides signal generator)."
                    },
                    "output_wav": {
                        "type": "string",
                        "description": "Optional output WAV file path to save rendered audio."
                    },
                    "params": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional parameter overrides in `'ID=VALUE'` format."
                    },
                    "note": {
                        "type": "integer",
                        "description": "Optional MIDI/CLAP note key (0..127) to trigger at sample 0."
                    }
                },
                "required": ["plugin"]
            }
        },
        {
            "name": "hph_screenshot",
            "description": "Open the plugin's GUI offscreen (Cocoa NSView on macOS, Win32 HWND on Windows, X11 Window on Linux), pump the main event loop and timers, and save an RGBA PNG screenshot.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin": {
                        "type": "string",
                        "description": "Path to the `.clap`, `.vst3`, or `.component` plugin."
                    },
                    "id": {
                        "type": "string",
                        "description": "Optional plugin ID."
                    },
                    "output": {
                        "type": "string",
                        "description": "Output PNG file path."
                    },
                    "warmup_ms": {
                        "type": "integer",
                        "description": "Warmup duration in ms to pump timers/runloop before capture (default: 200)."
                    },
                    "width": {
                        "type": "integer",
                        "description": "Optional custom window width."
                    },
                    "height": {
                        "type": "integer",
                        "description": "Optional custom window height."
                    }
                },
                "required": ["plugin", "output"]
            }
        },
        {
            "name": "hph_datasheet",
            "description": "Generate a print-ready white HTML + inline vector SVG engineering datasheet for an audio plugin, including Bode Magnitude/Unwrapped Phase/Group Delay, Dynamics (-48..+24 dBFS), Harmonics (H2..H10) & IMD, 9-Band Octave RT60, CPU Load, 9-check Validation, and optional Parameter-Family Sweep (`sweep_param`) or A/B Reference Plugin (`reference`) overlays.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plugin": {
                        "type": "string",
                        "description": "Path to the `.clap`, `.vst3`, or `.component` plugin."
                    },
                    "id": {
                        "type": "string",
                        "description": "Optional plugin ID."
                    },
                    "output": {
                        "type": "string",
                        "description": "Output `.html` file path for the printable datasheet."
                    },
                    "sample_rate": {
                        "type": "number",
                        "description": "Sample rate in Hz (default: 48000)."
                    },
                    "sweep_param": {
                        "type": "integer",
                        "description": "Optional parameter ID to step across 5 values on Bode and Dynamics plots."
                    },
                    "reference": {
                        "type": "string",
                        "description": "Optional path to a reference plugin bundle for A/B dashed overlay comparison."
                    },
                    "params": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional parameter overrides in `'ID=VALUE'` format."
                    }
                },
                "required": ["plugin", "output"]
            }
        },
        {
            "name": "hph_suite",
            "description": "Scaffold, edit in-place (preserving comments via `toml_edit`), inspect, or run a declarative `.toml` test suite (`suite.toml`). Supports `action`: `'init'`, `'add'`, `'remove'`, `'set'`, `'show'`, or `'run'` (with optional `capture: true` or `update_baselines: true`).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["init", "add", "remove", "set", "show", "run"],
                        "description": "Suite subcommand to execute (default: 'run')."
                    },
                    "suite_file": {
                        "type": "string",
                        "description": "Path to the `.toml` suite manifest (default: 'suite.toml')."
                    },
                    "plugin": {
                        "type": "string",
                        "description": "Plugin path (required for `action: 'init'`)."
                    },
                    "name": {
                        "type": "string",
                        "description": "Step name (for `add`, `remove`, `set`, or `run`)."
                    },
                    "params": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Parameter overrides in `'NAME=VALUE'` or `'ID=VALUE'` format (for `add`)."
                    },
                    "assertions": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Explicit assertion overrides in `'KEY=VALUE'` format (for `add`)."
                    },
                    "sets": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Key-value assignments in `'KEY=VALUE'` format (for `set`)."
                    },
                    "unsets": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Keys to remove in-place (for `set`)."
                    },
                    "capture": {
                        "type": "boolean",
                        "description": "Run the plugin and auto-populate baseline assertion thresholds (for `init` and `add`)."
                    },
                    "update_baselines": {
                        "type": "boolean",
                        "description": "Re-capture and update assertion thresholds in-place before running (for `run`)."
                    }
                }
            }
        }
    ])
}
