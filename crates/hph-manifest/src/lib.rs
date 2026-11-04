pub mod capture;
pub mod editor;

pub use capture::{
    capture_step_baseline, resolve_param_overrides, CapturedStepMetrics, ResolvedParamOverride,
};
pub use editor::{
    add_or_update_step_in_place, remove_step_in_place, scaffold_suite_manifest,
    set_values_in_place, update_baselines_in_place, StepEditSpec,
};

use anyhow::{anyhow, bail, Context, Result};
use hph_dsp::SignalSpec;
use std::collections::BTreeMap;
use std::path::Path;
use toml_edit::{value, DocumentMut, Item, Table, TableLike, Value};

#[derive(Debug, Clone)]
pub struct SuiteConfig {
    pub plugin: String,
    pub plugin_id: Option<String>,
    pub sample_rate: f64,
    pub run_validation: bool,
    pub datasheet_html: Option<String>,
    pub sweep_param: Option<u32>,
    pub tests: Vec<SuiteTestStep>,
}

#[derive(Debug, Clone)]
pub struct SuiteTestStep {
    pub name: String,
    pub params: BTreeMap<String, f64>,
    pub signal: Option<SignalSpec>,
    pub frames: usize,
    pub output_wav: Option<String>,
    pub screenshot_png: Option<String>,
    pub min_true_peak_dbtp: Option<f64>,
    pub max_true_peak_dbtp: Option<f64>,
    pub min_rms_dbfs: Option<f64>,
    pub max_rms_dbfs: Option<f64>,
    pub min_gain_db: Option<f64>,
    pub max_gain_db: Option<f64>,
    pub min_thd_percent: Option<f64>,
    pub max_thd_percent: Option<f64>,
    pub max_imd_percent: Option<f64>,
    pub max_dc_offset_dbfs: Option<f64>,
    pub min_rt60_sec: Option<f64>,
    pub max_rt60_sec: Option<f64>,
    pub expected_delay_ms: Option<f64>,
    pub min_stereo_correlation: Option<f64>,
    pub require_zero_rt_allocs: bool,
}

impl Default for SuiteTestStep {
    fn default() -> Self {
        Self {
            name: String::new(),
            params: BTreeMap::new(),
            signal: None,
            frames: 48000,
            output_wav: None,
            screenshot_png: None,
            min_true_peak_dbtp: None,
            max_true_peak_dbtp: None,
            min_rms_dbfs: None,
            max_rms_dbfs: None,
            min_gain_db: None,
            max_gain_db: None,
            min_thd_percent: None,
            max_thd_percent: None,
            max_imd_percent: None,
            max_dc_offset_dbfs: None,
            min_rt60_sec: None,
            max_rt60_sec: None,
            expected_delay_ms: None,
            min_stereo_correlation: None,
            require_zero_rt_allocs: true,
        }
    }
}

pub fn load_suite_file(path: &Path) -> Result<SuiteConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read suite file '{}'", path.display()))?;
    parse_suite_str(&text)
        .with_context(|| format!("Failed to parse suite TOML '{}'", path.display()))
}

pub fn parse_suite_str(toml_str: &str) -> Result<SuiteConfig> {
    let doc: DocumentMut = toml_str
        .parse()
        .context("Invalid TOML syntax in suite manifest")?;
    parse_suite_doc(&doc)
}

pub fn parse_suite_doc(doc: &DocumentMut) -> Result<SuiteConfig> {
    let root = doc.as_table();
    let plugin = get_str(root, "plugin")
        .ok_or_else(|| anyhow!("Missing required top-level `plugin` string in suite manifest"))?;
    let plugin_id = get_str(root, "plugin_id");
    let sample_rate = get_f64(root, "sample_rate").unwrap_or(48000.0);
    let run_validation = get_bool(root, "run_validation").unwrap_or(false);
    let datasheet_html = get_str(root, "datasheet_html");
    let sweep_param = get_i64(root, "sweep_param").map(|v| v as u32);

    let mut tests = Vec::new();
    if let Some(Item::ArrayOfTables(arr)) = root.get("tests") {
        for tbl in arr.iter() {
            tests.push(parse_step_table(tbl)?);
        }
    }

    Ok(SuiteConfig {
        plugin,
        plugin_id,
        sample_rate,
        run_validation,
        datasheet_html,
        sweep_param,
        tests,
    })
}

pub(crate) fn parse_step_table(tbl: &Table) -> Result<SuiteTestStep> {
    let name = get_str(tbl, "name")
        .ok_or_else(|| anyhow!("Each `[[tests]]` entry must have a `name` string"))?;
    let frames = get_i64(tbl, "frames").unwrap_or(48000).max(1) as usize;

    let mut params = BTreeMap::new();
    if let Some(item) = tbl.get("params") {
        if let Some(ptbl) = item.as_table_like() {
            for (k, v) in ptbl.iter() {
                let val = item_to_f64(v).ok_or_else(|| {
                    anyhow!("Step '{name}': parameter '{k}' must be a numeric value")
                })?;
                params.insert(k.to_string(), val);
            }
        }
    }

    let signal = if let Some(item) = tbl.get("signal") {
        if let Some(stbl) = item.as_table_like() {
            Some(parse_signal_table(&name, stbl)?)
        } else {
            None
        }
    } else {
        None
    };

    Ok(SuiteTestStep {
        name,
        params,
        signal,
        frames,
        output_wav: get_str(tbl, "output_wav"),
        screenshot_png: get_str(tbl, "screenshot_png"),
        min_true_peak_dbtp: get_f64(tbl, "min_true_peak_dbtp"),
        max_true_peak_dbtp: get_f64(tbl, "max_true_peak_dbtp"),
        min_rms_dbfs: get_f64(tbl, "min_rms_dbfs"),
        max_rms_dbfs: get_f64(tbl, "max_rms_dbfs"),
        min_gain_db: get_f64(tbl, "min_gain_db"),
        max_gain_db: get_f64(tbl, "max_gain_db"),
        min_thd_percent: get_f64(tbl, "min_thd_percent"),
        max_thd_percent: get_f64(tbl, "max_thd_percent"),
        max_imd_percent: get_f64(tbl, "max_imd_percent"),
        max_dc_offset_dbfs: get_f64(tbl, "max_dc_offset_dbfs"),
        min_rt60_sec: get_f64(tbl, "min_rt60_sec"),
        max_rt60_sec: get_f64(tbl, "max_rt60_sec"),
        expected_delay_ms: get_f64(tbl, "expected_delay_ms"),
        min_stereo_correlation: get_f64(tbl, "min_stereo_correlation"),
        require_zero_rt_allocs: get_bool(tbl, "require_zero_rt_allocs").unwrap_or(true),
    })
}

pub(crate) fn parse_signal_table(step_name: &str, stbl: &dyn TableLike) -> Result<SignalSpec> {
    let sig_type = stbl
        .get("type")
        .and_then(|i| i.as_str())
        .ok_or_else(|| anyhow!("Step '{step_name}': `[tests.signal]` requires a `type` string"))?;

    let f64_or =
        |key: &str, def: f64| -> f64 { stbl.get(key).and_then(item_to_f64).unwrap_or(def) };
    let usize_or = |key: &str, def: usize| -> usize {
        stbl.get(key)
            .and_then(|i| i.as_integer())
            .map(|v| v.max(0) as usize)
            .unwrap_or(def)
    };
    let u64_or = |key: &str, def: u64| -> u64 {
        stbl.get(key)
            .and_then(|i| i.as_integer())
            .map(|v| v.max(0) as u64)
            .unwrap_or(def)
    };

    match sig_type {
        "silence" => Ok(SignalSpec::Silence),
        "sine" => Ok(SignalSpec::Sine {
            freq_hz: f64_or("freq_hz", 1000.0),
            amplitude: f64_or("amplitude", 0.5),
            phase_rad: f64_or("phase_rad", 0.0),
        }),
        "two_tone" | "twotone" => Ok(SignalSpec::TwoTone {
            freq1_hz: f64_or("freq1_hz", 19000.0),
            amp1: f64_or("amp1", 0.25),
            freq2_hz: f64_or("freq2_hz", 20000.0),
            amp2: f64_or("amp2", 0.25),
        }),
        "saw" => Ok(SignalSpec::Saw {
            freq_hz: f64_or("freq_hz", 440.0),
            amplitude: f64_or("amplitude", 0.5),
        }),
        "square" => Ok(SignalSpec::Square {
            freq_hz: f64_or("freq_hz", 440.0),
            amplitude: f64_or("amplitude", 0.5),
            duty: f64_or("duty", 0.5),
        }),
        "triangle" => Ok(SignalSpec::Triangle {
            freq_hz: f64_or("freq_hz", 440.0),
            amplitude: f64_or("amplitude", 0.5),
        }),
        "impulse" => Ok(SignalSpec::Impulse {
            offset_samples: usize_or("offset_samples", 0),
            amplitude: f64_or("amplitude", 1.0),
        }),
        "trigger_train" | "triggers" => Ok(SignalSpec::TriggerTrain {
            freq_hz: f64_or("freq_hz", 2.0),
            pulse_samples: usize_or("pulse_samples", 4),
            amplitude: f64_or("amplitude", 1.0),
        }),
        "white_noise" | "whitenoise" => Ok(SignalSpec::WhiteNoise {
            amplitude: f64_or("amplitude", 0.5),
            seed: u64_or("seed", 1),
        }),
        "pink_noise" | "pinknoise" => Ok(SignalSpec::PinkNoise {
            amplitude: f64_or("amplitude", 0.5),
            seed: u64_or("seed", 1),
        }),
        "subnormal_noise" | "subnormals" => Ok(SignalSpec::SubnormalNoise {
            seed: u64_or("seed", 1),
        }),
        "dc_offset" | "dc" => Ok(SignalSpec::DcOffset {
            level: f64_or("level", 0.5),
        }),
        "log_sweep" | "sweep" => Ok(SignalSpec::LogSweep {
            start_hz: f64_or("start_hz", 20.0),
            end_hz: f64_or("end_hz", 20000.0),
            amplitude: f64_or("amplitude", 0.5),
        }),
        other => bail!("Step '{step_name}': unknown signal type '{other}'"),
    }
}

pub(crate) fn write_signal_table(sig: &SignalSpec) -> Table {
    let mut t = Table::new();
    match *sig {
        SignalSpec::Silence => {
            t["type"] = value("silence");
        }
        SignalSpec::Sine {
            freq_hz,
            amplitude,
            phase_rad,
        } => {
            t["type"] = value("sine");
            t["freq_hz"] = value(freq_hz);
            t["amplitude"] = value(amplitude);
            t["phase_rad"] = value(phase_rad);
        }
        SignalSpec::TwoTone {
            freq1_hz,
            amp1,
            freq2_hz,
            amp2,
        } => {
            t["type"] = value("two_tone");
            t["freq1_hz"] = value(freq1_hz);
            t["amp1"] = value(amp1);
            t["freq2_hz"] = value(freq2_hz);
            t["amp2"] = value(amp2);
        }
        SignalSpec::Saw { freq_hz, amplitude } => {
            t["type"] = value("saw");
            t["freq_hz"] = value(freq_hz);
            t["amplitude"] = value(amplitude);
        }
        SignalSpec::Square {
            freq_hz,
            amplitude,
            duty,
        } => {
            t["type"] = value("square");
            t["freq_hz"] = value(freq_hz);
            t["amplitude"] = value(amplitude);
            t["duty"] = value(duty);
        }
        SignalSpec::Triangle { freq_hz, amplitude } => {
            t["type"] = value("triangle");
            t["freq_hz"] = value(freq_hz);
            t["amplitude"] = value(amplitude);
        }
        SignalSpec::Impulse {
            offset_samples,
            amplitude,
        } => {
            t["type"] = value("impulse");
            t["offset_samples"] = value(offset_samples as i64);
            t["amplitude"] = value(amplitude);
        }
        SignalSpec::TriggerTrain {
            freq_hz,
            pulse_samples,
            amplitude,
        } => {
            t["type"] = value("trigger_train");
            t["freq_hz"] = value(freq_hz);
            t["pulse_samples"] = value(pulse_samples as i64);
            t["amplitude"] = value(amplitude);
        }
        SignalSpec::WhiteNoise { amplitude, seed } => {
            t["type"] = value("white_noise");
            t["amplitude"] = value(amplitude);
            t["seed"] = value(seed as i64);
        }
        SignalSpec::PinkNoise { amplitude, seed } => {
            t["type"] = value("pink_noise");
            t["amplitude"] = value(amplitude);
            t["seed"] = value(seed as i64);
        }
        SignalSpec::SubnormalNoise { seed } => {
            t["type"] = value("subnormal_noise");
            t["seed"] = value(seed as i64);
        }
        SignalSpec::DcOffset { level } => {
            t["type"] = value("dc_offset");
            t["level"] = value(level);
        }
        SignalSpec::LogSweep {
            start_hz,
            end_hz,
            amplitude,
        } => {
            t["type"] = value("log_sweep");
            t["start_hz"] = value(start_hz);
            t["end_hz"] = value(end_hz);
            t["amplitude"] = value(amplitude);
        }
    }
    t
}

pub(crate) fn item_to_f64(item: &Item) -> Option<f64> {
    match item.as_value()? {
        Value::Float(f) => Some(*f.value()),
        Value::Integer(i) => Some(*i.value() as f64),
        _ => None,
    }
}

fn get_str(tbl: &Table, key: &str) -> Option<String> {
    tbl.get(key)?.as_str().map(|s| s.to_string())
}

fn get_f64(tbl: &Table, key: &str) -> Option<f64> {
    item_to_f64(tbl.get(key)?)
}

fn get_i64(tbl: &Table, key: &str) -> Option<i64> {
    tbl.get(key)?.as_integer()
}

fn get_bool(tbl: &Table, key: &str) -> Option<bool> {
    tbl.get(key)?.as_bool()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hph_core::*;

    struct MockManifestPlugin {
        desc: PluginDescriptorInfo,
        cutoff: f64,
        drive: f64,
        sr: f64,
    }

    impl MockManifestPlugin {
        fn new() -> Self {
            Self {
                desc: PluginDescriptorInfo {
                    format: PluginFormat::Clap,
                    id: "com.hph.mock.manifest".into(),
                    name: "Mock Manifest Filter".into(),
                    vendor: "HPH".into(),
                    version: "1.0.0".into(),
                    url: String::new(),
                    manual_url: String::new(),
                    support_url: String::new(),
                    description: String::new(),
                    features: vec!["audio-effect".into()],
                },
                cutoff: 8000.0,
                drive: 0.2,
                sr: 48000.0,
            }
        }
    }

    impl PluginGui for MockManifestPlugin {}
    impl PluginContractChecks for MockManifestPlugin {}

    impl PluginInstance for MockManifestPlugin {
        fn descriptor(&self) -> &PluginDescriptorInfo {
            &self.desc
        }

        fn activate(&mut self, sr: f64, _min: u32, _max: u32) -> HostResult<()> {
            self.sr = sr;
            Ok(())
        }
        fn deactivate(&mut self) {}
        fn reset(&mut self) {}

        fn process(
            &mut self,
            buffers: &mut AudioProcessBuffers<'_>,
            in_events: &[ProcessEvent],
            _out_events: &mut Vec<ProcessEvent>,
        ) -> HostResult<ProcessStatus> {
            for ev in in_events {
                if let ProcessEvent::ParamValue {
                    param_id, value, ..
                } = *ev
                {
                    match param_id {
                        101 => self.cutoff = value,
                        102 => self.drive = value,
                        _ => {}
                    }
                }
            }
            let gain = 1.0 + (self.drive as f32) * 3.0;
            for ch in 0..buffers.outputs.len() {
                let inp = buffers
                    .inputs
                    .get(ch)
                    .or_else(|| buffers.inputs.first())
                    .copied();
                for i in 0..buffers.num_frames {
                    let x = inp.map(|b| b[i]).unwrap_or(0.0);
                    buffers.outputs[ch][i] = (x * gain).tanh();
                }
            }
            Ok(ProcessStatus::Continue)
        }

        fn params(&self) -> Vec<ParamInfo> {
            let flags = ParamFlags {
                is_stepped: false,
                is_periodic: false,
                is_hidden: false,
                is_readonly: false,
                is_bypass: false,
                is_automatable: true,
                is_automatable_per_note_id: false,
                is_automatable_per_key: false,
                is_automatable_per_channel: false,
                is_automatable_per_port: false,
                is_modulatable: false,
                is_modulatable_per_note_id: false,
                is_modulatable_per_key: false,
                is_modulatable_per_channel: false,
                is_modulatable_per_port: false,
                requires_process: false,
                raw_bits: 0,
            };
            vec![
                ParamInfo {
                    id: 101,
                    index: 0,
                    name: "Filter Cutoff".into(),
                    module: "Filter".into(),
                    min_value: 20.0,
                    max_value: 20000.0,
                    default_value: 8000.0,
                    flags: flags.clone(),
                },
                ParamInfo {
                    id: 102,
                    index: 1,
                    name: "Saturation Drive".into(),
                    module: "Drive".into(),
                    min_value: 0.0,
                    max_value: 1.0,
                    default_value: 0.2,
                    flags,
                },
            ]
        }

        fn get_param_value(&self, param_id: u32) -> Option<f64> {
            match param_id {
                101 => Some(self.cutoff),
                102 => Some(self.drive),
                _ => None,
            }
        }

        fn param_value_to_text(&self, param_id: u32, value: f64) -> Option<String> {
            match param_id {
                101 => Some(format!("{value:.0} Hz")),
                102 => Some(format!("{:.0}%", value * 100.0)),
                _ => None,
            }
        }
    }

    #[test]
    fn test_scaffold_and_in_place_edits_preserve_comments() {
        let mut plug = MockManifestPlugin::new();
        let tmp_path = std::env::temp_dir().join("hph_manifest_test_suite.toml");

        // 1. Scaffold with --capture
        let toml_text = scaffold_suite_manifest(
            "mock.clap",
            None,
            &mut plug,
            48000.0,
            Some("datasheet.html"),
            true,
        )
        .expect("scaffold should succeed");
        std::fs::write(&tmp_path, &toml_text).unwrap();
        assert!(toml_text.contains("# Discovered Plugin Parameters:"));
        assert!(toml_text.contains("Filter Cutoff"));

        let parsed = load_suite_file(&tmp_path).expect("should parse scaffolded TOML");
        assert_eq!(parsed.plugin, "mock.clap");
        assert_eq!(parsed.tests.len(), 2);

        // 2. Resolve parameters by name ("Cutoff" -> 101, "Drive" -> 102) and add step in-place
        let resolved = resolve_param_overrides(
            &plug,
            &["Cutoff=4500".to_string(), "Drive=0.75".to_string()],
        )
        .expect("name resolution should succeed");
        assert_eq!(resolved[0].id, 101);
        assert_eq!(resolved[1].id, 102);

        let sig = SignalSpec::Sine {
            freq_hz: 1000.0,
            amplitude: 0.6,
            phase_rad: 0.0,
        };
        let cap = capture_step_baseline(
            &mut plug,
            48000.0,
            24000,
            &sig,
            &[(101, 4500.0), (102, 0.75)],
        )
        .unwrap();

        add_or_update_step_in_place(
            &tmp_path,
            &StepEditSpec {
                name: "Warm Saturation".into(),
                frames: 24000,
                signal: sig,
                params: resolved,
                assertions: BTreeMap::new(),
                output_wav: None,
                screenshot_png: None,
                require_zero_rt_allocs: true,
            },
            Some(&cap),
        )
        .unwrap();

        // 3. Edit step in-place via `set_values_in_place`
        set_values_in_place(
            &tmp_path,
            Some("Warm Saturation"),
            &[
                "param.Cutoff=3200".to_string(),
                "max_true_peak_dbtp=1.25".to_string(),
            ],
            &[],
            Some(&plug),
        )
        .unwrap();

        // Verify top-of-file comments and inline parameter comments are still intact
        let edited_raw = std::fs::read_to_string(&tmp_path).unwrap();
        assert!(edited_raw.contains("# Discovered Plugin Parameters:"));
        assert!(edited_raw.contains("# Filter Cutoff (3200 Hz)"));

        let parsed2 = load_suite_file(&tmp_path).unwrap();
        assert_eq!(parsed2.tests.len(), 3);
        let warm = parsed2
            .tests
            .iter()
            .find(|s| s.name == "Warm Saturation")
            .unwrap();
        assert_eq!(warm.params.get("101"), Some(&3200.0));
        assert_eq!(warm.max_true_peak_dbtp, Some(1.25));

        // 4. Remove step in-place
        assert!(remove_step_in_place(&tmp_path, "Warm Saturation").unwrap());
        let parsed3 = load_suite_file(&tmp_path).unwrap();
        assert_eq!(parsed3.tests.len(), 2);

        // 5. Update baselines in-place
        let count = update_baselines_in_place(&tmp_path, None, &mut plug).unwrap();
        assert_eq!(count, 2);

        let _ = std::fs::remove_file(&tmp_path);
    }
}
