use crate::capture::{
    capture_step_baseline, find_param_info, CapturedStepMetrics, ResolvedParamOverride,
};
use crate::{parse_step_table, write_signal_table};
use anyhow::{anyhow, bail, Context, Result};
use hph_core::PluginInstance;
use hph_dsp::SignalSpec;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;
use toml_edit::{value, ArrayOfTables, DocumentMut, Item, Table};

#[derive(Debug, Clone)]
pub struct StepEditSpec {
    pub name: String,
    pub frames: usize,
    pub signal: SignalSpec,
    pub params: Vec<ResolvedParamOverride>,
    pub assertions: BTreeMap<String, f64>,
    pub output_wav: Option<String>,
    pub screenshot_png: Option<String>,
    pub require_zero_rt_allocs: bool,
}

const KNOWN_STEP_FLOAT_ASSERTIONS: &[&str] = &[
    "min_true_peak_dbtp",
    "max_true_peak_dbtp",
    "min_rms_dbfs",
    "max_rms_dbfs",
    "min_gain_db",
    "max_gain_db",
    "min_thd_percent",
    "max_thd_percent",
    "max_imd_percent",
    "max_dc_offset_dbfs",
    "min_rt60_sec",
    "max_rt60_sec",
    "expected_delay_ms",
    "min_stereo_correlation",
];

/// Scaffold a richly commented `suite.toml` manifest for `instance`, optionally capturing
/// live baseline assertions for the default test steps.
pub fn scaffold_suite_manifest(
    plugin_path_str: &str,
    plugin_id: Option<&str>,
    instance: &mut dyn PluginInstance,
    sample_rate: f64,
    datasheet_html: Option<&str>,
    capture: bool,
) -> Result<String> {
    let desc = instance.descriptor().clone();
    let params = instance.params();

    let mut out = String::with_capacity(4096);
    let _ = writeln!(
        out,
        "# =============================================================================="
    );
    let _ = writeln!(
        out,
        "# HPH Declarative Regression & Engineering Suite: {} ({})",
        desc.name, desc.id
    );
    if !params.is_empty() {
        let _ = writeln!(out, "# Discovered Plugin Parameters:");
        for p in &params {
            let def_txt = instance
                .param_value_to_text(p.id, p.default_value)
                .unwrap_or_else(|| format!("{:.2}", p.default_value));
            let _ = writeln!(
                out,
                "#   {:>4} : {:<24} [{:.2} .. {:.2}] (default = {:.2} / {})",
                p.id, p.name, p.min_value, p.max_value, p.default_value, def_txt
            );
        }
    }
    let _ = writeln!(
        out,
        "# ==============================================================================\n"
    );

    let mut doc = DocumentMut::new();
    doc["plugin"] = value(plugin_path_str);
    if let Some(pid) = plugin_id {
        doc["plugin_id"] = value(pid);
    }
    doc["sample_rate"] = value(sample_rate);
    doc["run_validation"] = value(true);
    if let Some(html) = datasheet_html {
        doc["datasheet_html"] = value(html);
    }

    let default_param_pairs: Vec<(u32, f64)> =
        params.iter().map(|p| (p.id, p.default_value)).collect();
    let default_resolved: Vec<ResolvedParamOverride> = params
        .iter()
        .map(|p| ResolvedParamOverride {
            id: p.id,
            name: p.name.clone(),
            value: p.default_value,
            formatted: instance
                .param_value_to_text(p.id, p.default_value)
                .unwrap_or_else(|| format!("{:.2}", p.default_value)),
        })
        .collect();

    // Step 1: Nominal 1 kHz Sine Passband (-6 dBFS)
    let passband_sig = SignalSpec::Sine {
        freq_hz: 1000.0,
        amplitude: 0.5,
        phase_rad: 0.0,
    };
    let passband_cap = if capture {
        Some(capture_step_baseline(
            instance,
            sample_rate,
            48000,
            &passband_sig,
            &default_param_pairs,
        )?)
    } else {
        None
    };

    // Step 2: Hot +24 dBFS Overdrive Ceiling
    let hot_sig = SignalSpec::Sine {
        freq_hz: 1000.0,
        amplitude: 15.848932,
        phase_rad: 0.0,
    };
    let hot_cap = if capture {
        Some(capture_step_baseline(
            instance,
            sample_rate,
            48000,
            &hot_sig,
            &default_param_pairs,
        )?)
    } else {
        None
    };

    let mut tests_arr = ArrayOfTables::new();
    tests_arr.push(build_step_table(
        &StepEditSpec {
            name: "Clean 1kHz Sine Passband".into(),
            frames: 48000,
            signal: passband_sig,
            params: default_resolved.clone(),
            assertions: BTreeMap::new(),
            output_wav: None,
            screenshot_png: None,
            require_zero_rt_allocs: true,
        },
        passband_cap.as_ref(),
    ));
    tests_arr.push(build_step_table(
        &StepEditSpec {
            name: "Hot +24dBFS Overdrive Ceiling".into(),
            frames: 48000,
            signal: hot_sig,
            params: default_resolved,
            assertions: BTreeMap::new(),
            output_wav: None,
            screenshot_png: None,
            require_zero_rt_allocs: true,
        },
        hot_cap.as_ref(),
    ));
    doc["tests"] = Item::ArrayOfTables(tests_arr);

    out.push_str(&doc.to_string());
    Ok(out)
}

fn build_step_table(spec: &StepEditSpec, cap: Option<&CapturedStepMetrics>) -> Table {
    let mut t = Table::new();
    t["name"] = value(&spec.name);
    t["frames"] = value(spec.frames as i64);

    if let Some(c) = cap {
        t["max_true_peak_dbtp"] = value(c.suggested_max_true_peak_dbtp);
        if let Some(min_rms) = c.suggested_min_rms_dbfs {
            t["min_rms_dbfs"] = value(min_rms);
        }
        if let Some(max_gain) = c.suggested_max_gain_db {
            t["max_gain_db"] = value(max_gain);
        }
        if let Some(min_thd) = c.suggested_min_thd_percent {
            t["min_thd_percent"] = value(min_thd);
        }
        if let Some(max_thd) = c.suggested_max_thd_percent {
            t["max_thd_percent"] = value(max_thd);
        }
        t["max_dc_offset_dbfs"] = value(c.suggested_max_dc_offset_dbfs);
    }

    // Explicit user assertions override captured defaults
    for (k, &v) in &spec.assertions {
        t[k.as_str()] = value(v);
    }

    if let Some(ref wav) = spec.output_wav {
        t["output_wav"] = value(wav);
    }
    if let Some(ref png) = spec.screenshot_png {
        t["screenshot_png"] = value(png);
    }
    t["require_zero_rt_allocs"] = value(spec.require_zero_rt_allocs);

    if !spec.params.is_empty() {
        let mut ptbl = Table::new();
        for p in &spec.params {
            let key = p.id.to_string();
            ptbl[&key] = value(p.value);
            if let Some(mut kv) = ptbl.key_mut(&key) {
                kv.leaf_decor_mut().set_prefix("");
            }
            if let Some(val_item) = ptbl.get_mut(&key) {
                if let Some(v) = val_item.as_value_mut() {
                    v.decor_mut()
                        .set_suffix(format!(" # {} ({})", p.name, p.formatted));
                }
            }
        }
        t["params"] = Item::Table(ptbl);
    }

    t["signal"] = Item::Table(write_signal_table(&spec.signal));
    t
}

/// Add a new `[[tests]]` step or replace an existing step with the same `name` in-place,
/// preserving all existing comments and formatting in `toml_path`.
pub fn add_or_update_step_in_place(
    toml_path: &Path,
    spec: &StepEditSpec,
    cap: Option<&CapturedStepMetrics>,
) -> Result<bool> {
    for k in spec.assertions.keys() {
        if !KNOWN_STEP_FLOAT_ASSERTIONS.contains(&k.as_str()) {
            bail!(
                "Unknown assertion key '{k}'. Valid assertion keys: {}",
                KNOWN_STEP_FLOAT_ASSERTIONS.join(", ")
            );
        }
    }

    let raw = std::fs::read_to_string(toml_path)
        .with_context(|| format!("Failed to read '{}'", toml_path.display()))?;
    let mut doc: DocumentMut = raw
        .parse()
        .with_context(|| format!("Failed to parse TOML in '{}'", toml_path.display()))?;

    let new_tbl = build_step_table(spec, cap);
    let mut updated_existing = false;

    if !doc.contains_key("tests") {
        doc["tests"] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    let arr = doc["tests"].as_array_of_tables_mut().ok_or_else(|| {
        anyhow!(
            "`tests` in '{}' is not an array of tables",
            toml_path.display()
        )
    })?;

    for existing in arr.iter_mut() {
        if existing.get("name").and_then(|i| i.as_str()) == Some(spec.name.as_str()) {
            *existing = new_tbl.clone();
            updated_existing = true;
            break;
        }
    }
    if !updated_existing {
        arr.push(new_tbl);
    }

    std::fs::write(toml_path, doc.to_string())
        .with_context(|| format!("Failed to write '{}'", toml_path.display()))?;
    Ok(updated_existing)
}

/// Remove a `[[tests]]` step by `name` in-place while preserving all comments and remaining steps.
pub fn remove_step_in_place(toml_path: &Path, step_name: &str) -> Result<bool> {
    let raw = std::fs::read_to_string(toml_path)
        .with_context(|| format!("Failed to read '{}'", toml_path.display()))?;
    let mut doc: DocumentMut = raw
        .parse()
        .with_context(|| format!("Failed to parse TOML in '{}'", toml_path.display()))?;

    let Some(Item::ArrayOfTables(arr)) = doc.get_mut("tests") else {
        return Ok(false);
    };

    let mut found_idx = None;
    for (idx, tbl) in arr.iter().enumerate() {
        if tbl.get("name").and_then(|i| i.as_str()) == Some(step_name) {
            found_idx = Some(idx);
            break;
        }
    }

    if let Some(idx) = found_idx {
        arr.remove(idx);
        std::fs::write(toml_path, doc.to_string())
            .with_context(|| format!("Failed to write '{}'", toml_path.display()))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Modify top-level or step-level keys (`KEY=VALUE`) and/or remove keys (`--unset KEY`) in-place.
pub fn set_values_in_place(
    toml_path: &Path,
    step_name: Option<&str>,
    sets: &[String],
    unsets: &[String],
    instance: Option<&dyn PluginInstance>,
) -> Result<()> {
    let raw = std::fs::read_to_string(toml_path)
        .with_context(|| format!("Failed to read '{}'", toml_path.display()))?;
    let mut doc: DocumentMut = raw
        .parse()
        .with_context(|| format!("Failed to parse TOML in '{}'", toml_path.display()))?;

    if let Some(target_step) = step_name {
        let arr = doc
            .get_mut("tests")
            .and_then(|i| i.as_array_of_tables_mut())
            .ok_or_else(|| anyhow!("No `[[tests]]` array found in '{}'", toml_path.display()))?;
        let step_tbl = arr
            .iter_mut()
            .find(|t| t.get("name").and_then(|i| i.as_str()) == Some(target_step))
            .ok_or_else(|| {
                anyhow!(
                    "Test step '{target_step}' not found in '{}'",
                    toml_path.display()
                )
            })?;

        for unset_key in unsets {
            if let Some(param_q) = unset_key
                .strip_prefix("param.")
                .or_else(|| unset_key.strip_prefix("params."))
            {
                let key_id = if let Some(inst) = instance {
                    find_param_info(&inst.params(), param_q)?.id.to_string()
                } else {
                    param_q.to_string()
                };
                if let Some(ptbl) = step_tbl
                    .get_mut("params")
                    .and_then(|i| i.as_table_like_mut())
                {
                    ptbl.remove(&key_id);
                }
            } else {
                step_tbl.remove(unset_key);
            }
        }

        for kv in sets {
            let (k_raw, v_raw) = kv
                .split_once('=')
                .with_context(|| format!("Invalid assignment '{kv}', expected KEY=VALUE"))?;
            let k = k_raw.trim();
            let v = v_raw.trim();

            if let Some(param_q) = k
                .strip_prefix("param.")
                .or_else(|| k.strip_prefix("params."))
            {
                let fval: f64 = v
                    .parse()
                    .with_context(|| format!("Invalid parameter float '{v}'"))?;
                let (key_id, comment_opt) = if let Some(inst) = instance {
                    let params = inst.params();
                    let info = find_param_info(&params, param_q)?;
                    if fval < info.min_value - 1e-6 || fval > info.max_value + 1e-6 {
                        bail!(
                            "Parameter '{}' (id={}) value {} is out of range [{:.4} .. {:.4}]",
                            info.name,
                            info.id,
                            fval,
                            info.min_value,
                            info.max_value
                        );
                    }
                    let txt = inst
                        .param_value_to_text(info.id, fval)
                        .unwrap_or_else(|| format!("{fval:.2}"));
                    (
                        info.id.to_string(),
                        Some(format!(" # {} ({})", info.name, txt)),
                    )
                } else {
                    (param_q.to_string(), None)
                };
                if !step_tbl.contains_key("params") {
                    step_tbl["params"] = Item::Table(Table::new());
                }
                let ptbl = step_tbl["params"]
                    .as_table_like_mut()
                    .ok_or_else(|| anyhow!("`params` is not a table"))?;
                ptbl.insert(&key_id, value(fval));
                if let Some(suffix) = comment_opt {
                    if let Some(item) = ptbl.get_mut(&key_id) {
                        if let Some(val_mut) = item.as_value_mut() {
                            val_mut.decor_mut().set_suffix(suffix);
                        }
                    }
                }
            } else if let Some(sig_k) = k.strip_prefix("signal.") {
                if !step_tbl.contains_key("signal") {
                    step_tbl["signal"] = Item::Table(Table::new());
                }
                let stbl = step_tbl["signal"]
                    .as_table_like_mut()
                    .ok_or_else(|| anyhow!("`signal` is not a table"))?;
                if sig_k == "type" {
                    stbl.insert("type", value(v));
                } else if sig_k == "offset_samples" || sig_k == "pulse_samples" || sig_k == "seed" {
                    let iv: i64 = v
                        .parse()
                        .with_context(|| format!("Invalid integer '{v}'"))?;
                    stbl.insert(sig_k, value(iv));
                } else {
                    let fv: f64 = v.parse().with_context(|| format!("Invalid float '{v}'"))?;
                    stbl.insert(sig_k, value(fv));
                }
            } else if k == "name" || k == "output_wav" || k == "screenshot_png" {
                step_tbl[k] = value(v);
            } else if k == "frames" {
                let iv: i64 = v
                    .parse()
                    .with_context(|| format!("Invalid frames integer '{v}'"))?;
                step_tbl["frames"] = value(iv);
            } else if k == "require_zero_rt_allocs" {
                let bv: bool = v
                    .parse()
                    .with_context(|| format!("Invalid boolean '{v}'"))?;
                step_tbl["require_zero_rt_allocs"] = value(bv);
            } else if KNOWN_STEP_FLOAT_ASSERTIONS.contains(&k) {
                let fv: f64 = v
                    .parse()
                    .with_context(|| format!("Invalid assertion float '{v}'"))?;
                step_tbl[k] = value(fv);
            } else {
                bail!(
                    "Unknown step key '{k}'. Use assertion keys ({}), `param.<NAME_OR_ID>`, `signal.<FIELD>`, `frames`, `output_wav`, `screenshot_png`, or `require_zero_rt_allocs`.",
                    KNOWN_STEP_FLOAT_ASSERTIONS.join(", ")
                );
            }
        }
    } else {
        for unset_key in unsets {
            doc.remove(unset_key);
        }
        for kv in sets {
            let (k_raw, v_raw) = kv
                .split_once('=')
                .with_context(|| format!("Invalid assignment '{kv}', expected KEY=VALUE"))?;
            let k = k_raw.trim();
            let v = v_raw.trim();
            match k {
                "plugin" | "plugin_id" | "datasheet_html" => {
                    doc[k] = value(v);
                }
                "sample_rate" => {
                    let fv: f64 = v
                        .parse()
                        .with_context(|| format!("Invalid sample_rate '{v}'"))?;
                    doc["sample_rate"] = value(fv);
                }
                "run_validation" => {
                    let bv: bool = v
                        .parse()
                        .with_context(|| format!("Invalid boolean '{v}'"))?;
                    doc["run_validation"] = value(bv);
                }
                "sweep_param" => {
                    let iv: i64 = if let Some(inst) = instance {
                        find_param_info(&inst.params(), v)?.id as i64
                    } else {
                        v.parse()
                            .with_context(|| format!("Invalid sweep_param '{v}'"))?
                    };
                    doc["sweep_param"] = value(iv);
                }
                other => {
                    bail!(
                        "Unknown top-level suite key '{other}'. Valid top-level keys: plugin, plugin_id, sample_rate, run_validation, datasheet_html, sweep_param (or pass `--step <STEP_NAME>` to edit a test step)."
                    );
                }
            }
        }
    }

    std::fs::write(toml_path, doc.to_string())
        .with_context(|| format!("Failed to write '{}'", toml_path.display()))?;
    Ok(())
}

/// Re-run the plugin across all (or a single named) `[[tests]]` steps in `toml_path` and update
/// their assertion bounds in-place while preserving all comments and layout.
pub fn update_baselines_in_place(
    toml_path: &Path,
    step_filter: Option<&str>,
    instance: &mut dyn PluginInstance,
) -> Result<usize> {
    let raw = std::fs::read_to_string(toml_path)
        .with_context(|| format!("Failed to read '{}'", toml_path.display()))?;
    let mut doc: DocumentMut = raw
        .parse()
        .with_context(|| format!("Failed to parse TOML in '{}'", toml_path.display()))?;

    let sample_rate = doc
        .get("sample_rate")
        .and_then(crate::item_to_f64)
        .unwrap_or(48000.0);

    let Some(Item::ArrayOfTables(arr)) = doc.get_mut("tests") else {
        return Ok(0);
    };

    let mut updated_count = 0usize;
    for tbl in arr.iter_mut() {
        let step = parse_step_table(tbl)?;
        if let Some(filter) = step_filter {
            if step.name != filter {
                continue;
            }
        }
        let sig = step.signal.unwrap_or(SignalSpec::Sine {
            freq_hz: 1000.0,
            amplitude: 0.5,
            phase_rad: 0.0,
        });
        let mut param_pairs = Vec::with_capacity(step.params.len());
        for (k, v) in &step.params {
            let pid: u32 = k
                .parse()
                .with_context(|| format!("Step '{}': param key '{k}' must be u32 ID", step.name))?;
            param_pairs.push((pid, *v));
        }

        let cap = capture_step_baseline(instance, sample_rate, step.frames, &sig, &param_pairs)?;
        tbl["max_true_peak_dbtp"] = value(cap.suggested_max_true_peak_dbtp);
        if let Some(min_rms) = cap.suggested_min_rms_dbfs {
            tbl["min_rms_dbfs"] = value(min_rms);
        }
        if let Some(max_gain) = cap.suggested_max_gain_db {
            tbl["max_gain_db"] = value(max_gain);
        }
        if let Some(min_thd) = cap.suggested_min_thd_percent {
            tbl["min_thd_percent"] = value(min_thd);
            tbl.remove("max_thd_percent");
        } else if let Some(max_thd) = cap.suggested_max_thd_percent {
            tbl["max_thd_percent"] = value(max_thd);
            tbl.remove("min_thd_percent");
        }
        tbl["max_dc_offset_dbfs"] = value(cap.suggested_max_dc_offset_dbfs);
        updated_count += 1;
    }

    std::fs::write(toml_path, doc.to_string())
        .with_context(|| format!("Failed to write '{}'", toml_path.display()))?;
    Ok(updated_count)
}
