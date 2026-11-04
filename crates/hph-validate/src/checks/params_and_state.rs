use crate::stream::{infer_channel_counts, process_audio_stream};
use crate::types::ValidationCheckResult;
use hph_core::*;
use hph_dsp::*;
use std::collections::HashSet;
use std::time::Instant;

/// Check #3: Parameter Bounds, Uniqueness & Text Formatting
pub fn check_parameters_contract(plugin: &dyn PluginInstance) -> ValidationCheckResult {
    let t0 = Instant::now();
    let params = plugin.params();
    let mut passed = true;
    let mut warnings = Vec::new();
    let mut ids = HashSet::new();
    let mut bypass_count = 0;

    for p in &params {
        if !ids.insert(p.id) {
            passed = false;
            warnings.push(format!("Duplicate parameter ID {} ('{}')", p.id, p.name));
        }
        if !p.min_value.is_finite()
            || !p.max_value.is_finite()
            || !p.default_value.is_finite()
            || p.min_value > p.max_value
            || p.default_value < p.min_value
            || p.default_value > p.max_value
        {
            passed = false;
            warnings.push(format!(
                "Param '{}' (id={}) has invalid range [{}, {}] default={}",
                p.name, p.id, p.min_value, p.max_value, p.default_value
            ));
        }
        if p.flags.is_bypass {
            bypass_count += 1;
        }
        if let Some(val) = plugin.get_param_value(p.id) {
            if val < p.min_value - 1e-6 || val > p.max_value + 1e-6 {
                warnings.push(format!(
                    "Param '{}' current value {} is outside [{}, {}]",
                    p.name, val, p.min_value, p.max_value
                ));
            }
        } else {
            passed = false;
            warnings.push(format!(
                "`get_value` failed for param '{}' (id={})",
                p.name, p.id
            ));
        }

        // Test value_to_text at min, default, max
        for probe_v in [p.min_value, p.default_value, p.max_value] {
            if let Some(txt) = plugin.param_value_to_text(p.id, probe_v) {
                let _ = plugin.param_text_to_value(p.id, &txt);
            } else {
                warnings.push(format!(
                    "Param '{}' `value_to_text({probe_v})` returned false",
                    p.name
                ));
            }
        }
    }
    if bypass_count > 1 {
        passed = false;
        warnings.push(format!(
            "Multiple parameters ({bypass_count}) marked with CLAP_PARAM_IS_BYPASS (max 1 allowed)"
        ));
    }

    ValidationCheckResult {
        category: "Parameters".into(),
        name: "Parameter Bounds, Uniqueness & Text Formatting".into(),
        passed,
        duration_ms: t0.elapsed().as_secs_f64() * 1e3,
        details: format!("Audited {} parameters", params.len()),
        warnings,
    }
}

/// Check #4: State Save/Load Roundtrip & Reset Determinism
pub fn check_state_and_reset_determinism(
    plugin: &mut dyn PluginInstance,
    sample_rate: f64,
) -> ValidationCheckResult {
    let t0 = Instant::now();
    let mut passed = true;
    let mut warnings = Vec::new();
    let mut details = String::new();

    let (in_ch, out_ch) = infer_channel_counts(plugin);
    let params = plugin.params();
    let n_frames = 2048;
    let stim = generate_signal(
        &SignalSpec::Sine {
            freq_hz: 440.0,
            amplitude: 0.5,
            phase_rad: 0.0,
        },
        sample_rate,
        n_frames,
    );
    let in_bufs = vec![stim; in_ch];
    let note_events = vec![ProcessEvent::NoteOn {
        sample_offset: 0,
        port_index: 0,
        channel: 0,
        key: 60,
        note_id: 1,
        velocity: 0.8,
    }];

    if let Err(e) = plugin.activate(sample_rate, 1, 4096) {
        passed = false;
        warnings.push(format!("activate() failed: {e}"));
    } else {
        let _ = plugin.start_processing();
        plugin.reset();

        let saved_state = if plugin.supports_state() {
            match plugin.save_state() {
                Ok(bytes) => Some(bytes),
                Err(e) => {
                    passed = false;
                    warnings.push(format!("save_state() failed: {e}"));
                    None
                }
            }
        } else {
            None
        };

        let render1 = process_audio_stream(plugin, &in_bufs, out_ch, n_frames, 256, &note_events);

        // Mutate all automatable parameters via flush/process, then restore state & reset
        if let Some(ref state_bytes) = saved_state {
            let mut scramble_events = Vec::new();
            for p in &params {
                if !p.flags.is_readonly {
                    scramble_events.push(ProcessEvent::ParamValue {
                        sample_offset: 0,
                        param_id: p.id,
                        note_id: -1,
                        port_index: -1,
                        channel: -1,
                        key: -1,
                        value: p.max_value,
                    });
                }
            }
            let _ = process_audio_stream(plugin, &in_bufs, out_ch, 256, 256, &scramble_events);
            plugin.stop_processing();
            if let Err(e) = plugin.load_state(state_bytes) {
                passed = false;
                warnings.push(format!("load_state() failed: {e}"));
            }
            let _ = plugin.start_processing();
        }

        plugin.reset();
        let render2 = process_audio_stream(plugin, &in_bufs, out_ch, n_frames, 256, &note_events);

        // Restore initial state so subsequent checks start with default parameters
        if let Some(ref state_bytes) = saved_state {
            plugin.stop_processing();
            let _ = plugin.load_state(state_bytes);
        }

        if let (Ok((out1, _, _)), Ok((out2, _, _))) = (render1, render2) {
            let max_diff = out1[0]
                .iter()
                .zip(out2[0].iter())
                .map(|(&a, &b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            if max_diff > 1e-5 {
                warnings.push(format!(
                    "Non-deterministic output after state restore + reset (max delta = {max_diff:.6})"
                ));
            }
            details = format!(
                "State bytes: {}, post-reset max sample delta: {:.2e}",
                saved_state.as_ref().map(|b| b.len()).unwrap_or(0),
                max_diff
            );
        } else {
            passed = false;
            warnings.push("Audio render failed during determinism check".into());
        }

        plugin.stop_processing();
        plugin.deactivate();
    }

    ValidationCheckResult {
        category: "State & Determinism".into(),
        name: "State Save/Load Roundtrip & Reset Determinism".into(),
        passed,
        duration_ms: t0.elapsed().as_secs_f64() * 1e3,
        details,
        warnings,
    }
}
