# Headless Plugin Host (`hph`)

> [!IMPORTANT]
> **Your vibecoded plugins probably suck - and you probably do not know it yet.**

Maybe your filter or compressor sounds fine on a `-12 dBFS` test loop in your DAW, right up until:
- Moving a knob triggers a hidden `malloc` or `std::vector` reallocation on the real-time audio thread.
- A reverb tail decays into IEEE-754 subnormals (`1e-40`) and spikes your user's CPU load by **40x**.
- Someone feeds it a hot `+24 dBFS` transient before a limiter and your DSP silently auto-mutes, latches onto a constant DC rail, or blows up into `NaN`/`Inf`.
- Your saturator aliases harsh enharmonic spurs across the passband, or your lookahead limiter lies to the host about its PDC latency by 12 samples.

Even if you *do* know how to measure audio quality, you are stuck juggling three completely different format-specific validators-`clap-validator` for CLAP, `pluginval` for VST3, and `auval` for macOS Audio Units-and **none of them actually measure what your DSP does to audio**. They check whether your binary loads without segfaulting; they will not tell you your `-3 dB` cutoff frequency, unwrapped phase rotation, group delay, THD+N, two-tone intermodulation distortion, ISO 3382 Schroeder RT60 decay, or whether your GUI actually renders.

**`headless-plugin-host` (`hph`)** is a single, zero-DAW command-line tool, regression test runner, printable datasheet generator, and Model Context Protocol (MCP) server for **CLAP (`.clap`)**, **VST3 (`.vst3`)**, and **Audio Unit v2 (`.component`)** plugins.

---

## Comparison with Existing Validators

| Capability | `clap-validator` | `pluginval` | `auval` | **`hph` (`headless-plugin-host`)** |
| :--- | :---: | :---: | :---: | :--- |
| **Plugin Formats** | CLAP only | VST3 / AU / VST2 | AUv2 only (macOS) | **CLAP, VST3, and AUv2** |
| **ABI & Lifecycle Contract Checks** | Yes | Yes | Yes | **Yes (9-point suite + extension probing)** |
| **Real-Time Heap Allocation Interception** | - | Basic | - | **Yes (`malloc`/`free`/`realloc` audio-thread hook)** |
| **Denormal / Subnormal CPU Slowdown Ratio** | - | - | - | **Yes (Measures exact CPU slowdown + subnormal leaks)** |
| **`+24 dBFS` Overload, Auto-Mute & DC Latch Check** | - | - | - | **Yes (4-stage envelope + recovery verification)** |
| **Corrupted / Bit-Flipped State Fuzzing** | - | - | - | **Yes (Seeded PRNG chaos + state mutation torture)** |
| **Bode Magnitude, Unwrapped Phase & Group Delay** | - | - | - | **Yes (31-band ISO 1/3-octave transfer function)** |
| **THD+N, Even/Odd Harmonics & Aliasing Spurs** | - | - | - | **Yes ($H_2 \dots H_{10}$, SINAD, max alias spur dBc)** |
| **Two-Tone Intermodulation Distortion (IMD)** | - | - | - | **Yes (CCIF/SMPTE $f_2 - f_1$ & $2f_1 - f_2$)** |
| **ISO 3382 Reverberation Time (RT60) & Echo Taps** | - | - | - | **Yes (EDT, $T_{20}$, $T_{30}$, 9-band octave RT60, GCC-PHAT delay)** |
| **Dynamics Transfer Curve & Attack/Release** | - | - | - | **Yes (`-48 .. +24 dBFS` curve, ratio, knee, 10–90% ballistics)** |
| **4x Oversampled True-Peak & BS.1770-4 LUFS** | - | - | - | **Yes (Polyphase 4x FIR dBTP + K-weighted LUFS)** |
| **Offscreen Headless GUI Screenshot Capture** | - | - | - | **Yes (macOS Cocoa `NSView`, Windows `HWND`, Linux X11)** |
| **Printable HTML + Vector SVG Datasheets** | - | - | - | **Yes (`hph datasheet` with param sweeps & A/B overlays)** |
| **Declarative `.toml` Suites with Live Auto-Baselining** | - | - | - | **Yes (`hph suite init/add/set/run --capture`)** |
| **Built-In MCP Server for AI Coding Agents** | - | - | - | **Yes (`hph mcp` over stdio with crash isolation)** |

---

## Installation

### 1. Prebuilt Binaries (macOS, Linux, Windows)
Download the latest archive for your platform from [**GitHub Releases**](../../releases):
- **macOS (Universal Apple Silicon + Intel):** `hph-universal-apple-darwin.tar.gz`
- **Linux (`x86_64`):** `hph-x86_64-unknown-linux-gnu.tar.gz`
- **Windows (`x86_64`):** `hph-x86_64-pc-windows-msvc.zip`

```bash
# Example (macOS / Linux):
curl -fsSL https://github.com/cyrillsemenov/headless-plugin-host/releases/latest/download/hph-universal-apple-darwin.tar.gz | tar -xz
sudo mv hph /usr/local/bin/
```

### 2. Build from Source (Rust / Cargo)
Requires Rust 1.80+ (`rustup`):
```bash
git clone --recurse-submodules https://github.com/cyrillsemenov/headless-plugin-host.git
cd headless-plugin-host
cargo install --path crates/hph-cli
```

---

## How to Use

> [!TIP]
> Every subcommand accepts `.clap`, `.vst3`, or `.component` (macOS AUv2) bundles and supports `--json` for machine-readable output.

### 1. Inspect Plugin Topology & Parameters (`hph inspect`)
```bash
hph inspect path/to/MyPlugin.clap
```
Lists all descriptors in the bundle, audio/note ports, parameter IDs/names/ranges/defaults, PDC latency, tail length, and supported extensions.

### 2. Run the 9-Point Engineering Validation Suite (`hph validate`)
```bash
hph validate path/to/MyPlugin.vst3 --sample-rate 48000
```
Verifies:
- [x] **Descriptor & Port Contract Validity**
- [x] **Activation / Deactivation & Sample-Rate Lifecycle Torture**
- [x] **Buffer-Size Invariance** (`1` sample, prime `63`, `256`, `1024`)
- [x] **Parameter Automation & Out-of-Bounds Clamping**
- [x] **State Serialization Round-Trip & Determinism**
- [x] **Subnormal (`~ -800 dBFS`), `+24 dBFS` Overload, Auto-Mute, DC Latch & Recovery Health**
- [x] **Zero Real-Time Audio Thread Heap Allocations** (`malloc`/`free`/`realloc` interception)
- [x] **Format-Specific Contract Checks**
- [x] **Extended Host & Plugin Extension Probing**

### 3. Measure Acoustic & DSP Performance (`hph analyze`)
```bash
# Run all DSP analyzers at once:
hph analyze path/to/MyPlugin.clap all -p 101=2500.0 -p 102=0.5

# Or run a specific analyzer:
# Modes: all | spectrum | bode | thdn | imd | rt60 | delay | dynamics | stereo
hph analyze path/to/MyPlugin.clap bode
hph analyze path/to/MyPlugin.clap thdn --freq 1000
```

### 4. Generate a Printable Engineering Datasheet (`hph datasheet`)
```bash
# Single-plugin datasheet:
hph datasheet path/to/MyPlugin.clap -o datasheet.html

# Sweep a parameter (e.g. Cutoff ID 101) across 5 steps and overlay a reference plugin:
hph datasheet path/to/MyPlugin.clap -o datasheet.html --sweep-param 101 --reference path/to/Reference.vst3
```
Produces a self-contained, white-background HTML document (ready for **Print to PDF**) with inline vector SVG charts for Bode Magnitude/Phase/Group Delay, Dynamics Transfer Curve (`-48 .. +24 dBFS`), Harmonic Spurs ($H_2 \dots H_{10}$) + IMD, 9-Band Octave RT60, CPU Load, Validation Scorecard, and an embedded GUI screenshot.

### 5. Declarative TOML Regression Suites (`hph suite`)
You rarely need to edit `suite.toml` by hand. `hph suite` modifies manifests in-place while preserving all comments, resolves parameter names to IDs automatically, and can run your plugin live (`--capture`) to calculate baseline assertion thresholds:

```bash
# 1. Scaffold a new suite.toml and capture live baseline thresholds from the plugin:
hph suite init path/to/MyPlugin.clap -o suite.toml --capture --datasheet docs/datasheet.html

# 2. Add a new test step by parameter name (resolves "Cutoff" & "Drive" to IDs and captures assertions):
hph suite add -f suite.toml --name "Driven Midrange" -p "Cutoff=3500" -p "Drive=0.75" --signal sine --freq 1000 --amp 0.7 --capture

# 3. Tweak a parameter or assertion threshold in-place (preserving all comments):
hph suite set -f suite.toml --step "Driven Midrange" param.Cutoff=4200 max_thd_percent=18.0

# 4. Inspect or remove steps:
hph suite show suite.toml
hph suite remove -f suite.toml --name "Driven Midrange"

# 5. Run the regression suite (exits non-zero on any assertion or RT allocation failure):
hph suite run suite.toml

# 6. Intentionally changed your DSP? Re-record all assertion baselines in-place:
hph suite run suite.toml --update-baselines
```

### 6. Chaos Fuzzing, Benchmarking, Rendering & GUI Screenshots
```bash
# Seeded chaos fuzzing (prime block sizes, zipper parameter storms, +24 dBFS rails, corrupted state):
hph fuzz path/to/MyPlugin.clap --iterations 500 --corrupt-state

# Multi-rate (44.1 / 48 / 96 kHz) & multi-block (64..1024) CPU & Denormal benchmark:
hph bench path/to/MyPlugin.clap --blocks 200 --gui

# Render a test signal or input WAV through the plugin and meter 4x True-Peak & K-weighted LUFS:
hph render path/to/MyPlugin.clap --signal sine --freq 1000 --amp 0.5 -o out.wav

# Capture an offscreen GUI screenshot to PNG:
hph screenshot path/to/MyPlugin.clap -o gui.png --warmup-ms 250
```

---

## Model Context Protocol (MCP) Server

`hph` includes a built-in, self-documenting **MCP server** (`hph mcp`) over `stdio`. It works with any MCP-compatible client, including **Claude Code**, **Claude Desktop**, **Cursor**, **Windsurf**, **Cline**, **Zed**, and **Gemini / Antigravity**.

> [!NOTE]
> Every MCP tool call executes in an **isolated child process**. If a C/C++ plugin segfaults (`SIGSEGV` / `SIGABRT`) during development, the MCP server stays alive and returns the crash diagnostics directly to your agent.

### MCP Client Configuration
Add `hph` to your client's MCP configuration file (e.g. `.mcp.json`, `claude_desktop_config.json`, or `.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "headless-plugin-host": {
      "command": "hph",
      "args": ["mcp"]
    }
  }
}
```

### Exposed MCP Capabilities
- **10 Tools:** `hph_usage_guide`, `hph_inspect`, `hph_validate`, `hph_analyze`, `hph_fuzz`, `hph_bench`, `hph_render`, `hph_screenshot`, `hph_datasheet`, `hph_suite`
- **Resources:** `hph://docs/guide` (DSP measurement & interpretation handbook) and `hph://docs/toml-suite-schema`
- **Prompts:** `audit-plugin` (5-stage automated engineering audit) and `debug-dsp` (targeted diagnosis for aliasing, RT allocations, denormals, or latency bugs)

---

## Continuous Integration (GitHub Actions)

`headless-plugin-host` can be used directly as a composite GitHub Action on `macos-latest`, `windows-latest`, and `ubuntu-latest` (automatically configuring `xvfb-run` on Linux for headless GUI screenshots and posting full Markdown reports to the GitHub Job Summary):

```yaml
- name: Validate, Fuzz, Analyze & Screenshot Plugin
  uses: cyrillsemenov/headless-plugin-host@v0.1.0
  with:
    plugin: build/MyPlugin_artefacts/Release/CLAP/MyPlugin.clap
    validate: "true"
    fuzz-iterations: "300"
    corrupt-state: "true"
    analyze-mode: "all"
    screenshot: "true"
    screenshot-output: hph-artifacts/plugin-gui.png
    suite-file: tests/suite.toml
```

---

## Workspace Architecture & Contributing

The repository is organized as a modular Rust workspace:
- `crates/hph-core` - Format-agnostic traits (`PluginBundle`, `PluginInstance`, `PluginGui`, `PluginContractChecks`)
- `crates/hph-clap-sys` & `crates/hph-clap` - Pure-Rust CLAP C ABI bindings & host implementation
- `crates/hph-vst3-sys` & `crates/hph-vst3` - Pure-Rust Steinberg VST3 COM ABI bindings & host implementation
- `crates/hph-au-sys` & `crates/hph-au` - macOS AudioToolbox / Audio Unit v2 host implementation
- `crates/hph-rt-check` - Global allocator wrapper detecting real-time audio thread heap allocations
- `crates/hph-dsp` - Signal generators, arbitrary-length Bluestein/Radix-2 FFT, 31-band Bode, THD+N, IMD, 9-band ISO 3382 RT60, GCC-PHAT delay, dynamics, stereo, and 4x True-Peak / BS.1770-4 LUFS metering
- `crates/hph-gui` - Offscreen native window hosting & RGBA PNG capture (Cocoa / Win32 / X11)
- `crates/hph-validate` - 9-point validation suite, multi-block benchmarks, and seeded chaos fuzzer
- `crates/hph-datasheet` - Printable HTML + inline vector SVG engineering datasheet generator
- `crates/hph-manifest` - Zero-`serde` `toml_edit` suite manifest scaffolder, in-place editor & live auto-baseliner
- `crates/hph-mcp` - JSON-RPC 2.0 Model Context Protocol stdio server
- `crates/hph-cli` - Unified `hph` command-line interface
- `crates/hph-mock-plugin` - Self-contained reference CLAP plugin used by integration tests

### Running Tests
```bash
cargo test --workspace
```

Contributions, bug reports, and new DSP measurement modules are welcome. Please ensure `cargo test --workspace` passes before opening a pull request.

---

## License

Licensed under the [MIT License](./LICENSE).

---

## PS

If your vibecoded plugins still suck, try to think out of the box. Stay creative.
