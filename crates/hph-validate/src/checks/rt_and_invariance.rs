use crate::stream::{infer_channel_counts, process_audio_stream};
use crate::types::ValidationCheckResult;
use hph_core::*;
use hph_dsp::*;
use hph_rt_check::*;
use std::time::Instant;

/// Check #5: Variable Block-Size Invariance (1, 17, 64, 512)
pub fn check_block_size_invariance(
    plugin: &mut dyn PluginInstance,
    sample_rate: f64,
) -> ValidationCheckResult {
    let t0 = Instant::now();
    let mut passed = true;
    let mut warnings = Vec::new();
    let (in_ch, out_ch) = infer_channel_counts(plugin);
    let n_frames = 1024;
    let stim = generate_signal(
        &SignalSpec::Sine {
            freq_hz: 330.0,
            amplitude: 0.4,
            phase_rad: 0.0,
        },
        sample_rate,
        n_frames,
    );
    let in_bufs = vec![stim; in_ch];

    let mut outputs_by_blk = Vec::new();
    for &blk in &[1usize, 17, 64, 512] {
        if plugin.activate(sample_rate, 1, 1024).is_ok() && plugin.start_processing().is_ok() {
            plugin.reset();
            if let Ok((out, _, _)) =
                process_audio_stream(plugin, &in_bufs, out_ch, n_frames, blk, &[])
            {
                outputs_by_blk.push((blk, out[0].clone()));
            } else {
                passed = false;
                warnings.push(format!("process() failed with block size {blk}"));
            }
            plugin.stop_processing();
            plugin.deactivate();
        }
    }

    let mut max_blk_diff = 0.0f32;
    if outputs_by_blk.len() >= 2 {
        let ref_out = &outputs_by_blk[0].1;
        for (blk, out) in &outputs_by_blk[1..] {
            let diff = ref_out
                .iter()
                .zip(out.iter())
                .map(|(&a, &b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            max_blk_diff = max_blk_diff.max(diff);
            if diff > 1e-4 {
                warnings.push(format!(
                    "Block size {blk} differs from block size 1 by {diff:.5}"
                ));
            }
        }
    }

    ValidationCheckResult {
        category: "DSP Invariance".into(),
        name: "Variable Block-Size Invariance (1, 17, 64, 512)".into(),
        passed,
        duration_ms: t0.elapsed().as_secs_f64() * 1e3,
        details: format!("Max delta across block sizes: {max_blk_diff:.2e}"),
        warnings,
    }
}

/// Check #7: Real-Time Allocation Audit & Thread-Check Contract
pub fn check_realtime_allocation_and_threads(
    plugin: &mut dyn PluginInstance,
    sample_rate: f64,
) -> (ValidationCheckResult, HostDiagnostics) {
    let t0 = Instant::now();
    let mut passed = true;
    let mut warnings = Vec::new();
    let (in_ch, out_ch) = infer_channel_counts(plugin);
    let n_frames = 4096;
    let stim = generate_signal(
        &SignalSpec::WhiteNoise {
            amplitude: 0.25,
            seed: 1234,
        },
        sample_rate,
        n_frames,
    );
    let in_bufs = vec![stim; in_ch];

    // Generate parameter automation sweep events on non-bypass parameters
    let mut events = Vec::new();
    for (idx, p) in plugin
        .params()
        .iter()
        .filter(|p| !p.flags.is_readonly && !p.flags.is_bypass)
        .take(4)
        .enumerate()
    {
        for step in 0..8 {
            let frac = (step as f64) / 7.0;
            let val = p.min_value + frac * (p.max_value - p.min_value);
            events.push(ProcessEvent::ParamValue {
                sample_offset: (step * 512 + idx * 16).min(n_frames - 1) as u32,
                param_id: p.id,
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value: val,
            });
        }
    }

    let mut rt_stats = RtAllocStats::default();
    if plugin.activate(sample_rate, 1, 1024).is_ok() && plugin.start_processing().is_ok() {
        // Warm up 1 block outside RT check
        let _ = process_audio_stream(plugin, &in_bufs, out_ch, 256, 256, &[]);
        if let Ok((_, _, stats)) =
            process_audio_stream(plugin, &in_bufs, out_ch, n_frames, 256, &events)
        {
            rt_stats = stats;
        }
        plugin.stop_processing();
        plugin.deactivate();
    }

    if rt_stats.has_violations() {
        passed = false;
        warnings.push(format!(
            "Real-time violation: {} malloc(s) ({} bytes) and {} free(s) inside process()",
            rt_stats.alloc_calls, rt_stats.bytes_allocated, rt_stats.free_calls
        ));
    }

    let diag = plugin.take_diagnostics();
    if !diag.thread_violations.is_empty() {
        passed = false;
        for v in &diag.thread_violations {
            warnings.push(v.clone());
        }
    }

    let check = ValidationCheckResult {
        category: "Real-Time Safety".into(),
        name: "Zero-Allocation Audio Thread & Thread-Check Audit".into(),
        passed,
        duration_ms: t0.elapsed().as_secs_f64() * 1e3,
        details: format!(
            "RT allocs: {}, RT frees: {}, bytes: {}, thread violations: {}",
            rt_stats.alloc_calls,
            rt_stats.free_calls,
            rt_stats.bytes_allocated,
            diag.thread_violations.len()
        ),
        warnings,
    };

    (check, diag)
}
