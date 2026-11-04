use anyhow::{anyhow, bail, Context, Result};
use hph_core::{ParamInfo, PluginInstance, ProcessEvent};
use hph_dsp::{analyze_buffer_4x, analyze_thdn, generate_signal, SignalSpec};
use hph_validate::{infer_channel_counts, process_audio_stream};

#[derive(Debug, Clone)]
pub struct ResolvedParamOverride {
    pub id: u32,
    pub name: String,
    pub value: f64,
    pub formatted: String,
}

#[derive(Debug, Clone)]
pub struct CapturedStepMetrics {
    pub measured_true_peak_dbtp: f64,
    pub measured_rms_dbfs: f64,
    pub measured_gain_db: f64,
    pub measured_thd_percent: Option<f64>,
    pub measured_dc_offset_dbfs: f64,
    pub suggested_max_true_peak_dbtp: f64,
    pub suggested_min_rms_dbfs: Option<f64>,
    pub suggested_max_gain_db: Option<f64>,
    pub suggested_min_thd_percent: Option<f64>,
    pub suggested_max_thd_percent: Option<f64>,
    pub suggested_max_dc_offset_dbfs: f64,
}

/// Resolve `ID=VALUE` or `NAME=VALUE` parameter strings against a live plugin's `ParamInfo` table,
/// verifying that each value lies within `[min_value .. max_value]`.
pub fn resolve_param_overrides(
    instance: &dyn PluginInstance,
    raw_params: &[String],
) -> Result<Vec<ResolvedParamOverride>> {
    let infos = instance.params();
    let mut out = Vec::with_capacity(raw_params.len());

    for raw in raw_params {
        let (key_str, val_str) = raw
            .split_once('=')
            .with_context(|| format!("Invalid parameter override '{raw}', expected KEY=VALUE"))?;
        let key_trim = key_str.trim();
        let value: f64 = val_str
            .trim()
            .parse()
            .with_context(|| format!("Invalid numeric parameter value '{val_str}' in '{raw}'"))?;

        let matched = find_param_info(&infos, key_trim)?;
        if value < matched.min_value - 1e-6 || value > matched.max_value + 1e-6 {
            bail!(
                "Parameter '{}' (id={}) value {} is out of range [{:.4} .. {:.4}]",
                matched.name,
                matched.id,
                value,
                matched.min_value,
                matched.max_value
            );
        }
        let formatted = instance
            .param_value_to_text(matched.id, value)
            .unwrap_or_else(|| format!("{value:.3}"));

        out.push(ResolvedParamOverride {
            id: matched.id,
            name: matched.name.clone(),
            value,
            formatted,
        });
    }

    Ok(out)
}

pub fn find_param_info<'a>(infos: &'a [ParamInfo], query: &str) -> Result<&'a ParamInfo> {
    if let Ok(id) = query.parse::<u32>() {
        if let Some(p) = infos.iter().find(|p| p.id == id) {
            return Ok(p);
        }
        bail!("Parameter ID {id} not found on plugin");
    }

    let q_lower = query.to_ascii_lowercase();
    if let Some(exact) = infos
        .iter()
        .find(|p| p.name.to_ascii_lowercase() == q_lower)
    {
        return Ok(exact);
    }

    let substr_matches: Vec<&ParamInfo> = infos
        .iter()
        .filter(|p| p.name.to_ascii_lowercase().contains(&q_lower))
        .collect();
    match substr_matches.len() {
        1 => Ok(substr_matches[0]),
        0 => {
            let available: Vec<String> = infos
                .iter()
                .map(|p| format!("{} ('{}')", p.id, p.name))
                .collect();
            bail!(
                "No parameter matching '{query}'. Available parameters: {}",
                available.join(", ")
            )
        }
        _ => {
            let candidates: Vec<String> = substr_matches
                .iter()
                .map(|p| format!("{} ('{}')", p.id, p.name))
                .collect();
            bail!(
                "Ambiguous parameter name '{query}' matched multiple parameters: {}",
                candidates.join(", ")
            )
        }
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Execute the plugin with `signal` and `params` for `frames` samples and derive safe assertion bounds.
pub fn capture_step_baseline(
    instance: &mut dyn PluginInstance,
    sample_rate: f64,
    frames: usize,
    signal: &SignalSpec,
    params: &[(u32, f64)],
) -> Result<CapturedStepMetrics> {
    let (in_ch, out_ch) = infer_channel_counts(instance);
    let mono_in = generate_signal(signal, sample_rate, frames);
    let in_bufs = vec![mono_in.clone(); in_ch.max(1)];

    let events: Vec<ProcessEvent> = params
        .iter()
        .map(|&(param_id, value)| ProcessEvent::ParamValue {
            sample_offset: 0,
            param_id,
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value,
        })
        .collect();

    instance
        .activate(sample_rate, 1, 512)
        .map_err(|e| anyhow!("Failed to activate plugin during capture: {e}"))?;
    instance
        .start_processing()
        .map_err(|e| anyhow!("Failed to start_processing during capture: {e}"))?;
    instance.reset();
    let (out_bufs, _, _) =
        process_audio_stream(instance, &in_bufs, out_ch.max(1), frames, 256, &events)
            .map_err(|e| anyhow!("Failed to process audio stream during capture: {e}"))?;
    instance.stop_processing();
    instance.deactivate();

    let in_meter = analyze_buffer_4x(&mono_in);
    let out_meter = analyze_buffer_4x(&out_bufs[0]);
    let gain_db = out_meter.rms_4x_dbfs - in_meter.rms_4x_dbfs;

    let measured_thd_percent = if let SignalSpec::Sine { freq_hz, .. } = *signal {
        let steady = if out_bufs[0].len() > 8192 {
            &out_bufs[0][4096..]
        } else {
            &out_bufs[0][..]
        };
        Some(analyze_thdn(steady, sample_rate, freq_hz).thd_percent)
    } else {
        None
    };

    let suggested_max_true_peak_dbtp = round2((out_meter.true_peak_4x_dbtp + 1.0).max(0.5));
    let suggested_min_rms_dbfs = if out_meter.rms_4x_dbfs > -90.0 {
        Some(round2(out_meter.rms_4x_db_or_floor() - 2.5))
    } else {
        None
    };
    let suggested_max_gain_db = if in_meter.rms_4x_dbfs > -60.0 && gain_db < -3.0 {
        Some(round2(gain_db + 2.0))
    } else {
        None
    };

    let (suggested_min_thd_percent, suggested_max_thd_percent) = match measured_thd_percent {
        Some(thd) if thd < 0.25 => (None, Some(round2((thd * 2.5).max(0.05)))),
        Some(thd) if thd >= 1.5 => (Some(round2(thd * 0.6)), None),
        Some(thd) => (None, Some(round2(thd * 2.0 + 0.5))),
        None => (None, None),
    };

    let suggested_max_dc_offset_dbfs =
        round2((out_meter.dc_offset_dbfs + 12.0).clamp(-60.0, -24.0));

    Ok(CapturedStepMetrics {
        measured_true_peak_dbtp: round2(out_meter.true_peak_4x_dbtp),
        measured_rms_dbfs: round2(out_meter.rms_4x_dbfs),
        measured_gain_db: round2(gain_db),
        measured_thd_percent: measured_thd_percent.map(round2),
        measured_dc_offset_dbfs: round2(out_meter.dc_offset_dbfs),
        suggested_max_true_peak_dbtp,
        suggested_min_rms_dbfs,
        suggested_max_gain_db,
        suggested_min_thd_percent,
        suggested_max_thd_percent,
        suggested_max_dc_offset_dbfs,
    })
}

trait MeterExt {
    fn rms_4x_db_or_floor(&self) -> f64;
}

impl MeterExt for hph_dsp::ChannelMeterReport {
    fn rms_4x_db_or_floor(&self) -> f64 {
        if self.rms_4x_dbfs.is_finite() {
            self.rms_4x_dbfs
        } else {
            -120.0
        }
    }
}
