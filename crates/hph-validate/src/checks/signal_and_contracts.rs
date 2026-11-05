use crate::stream::{infer_channel_counts, process_audio_stream};
use crate::types::ValidationCheckResult;
use hph_core::*;
use hph_dsp::*;
use std::time::Instant;

/// Check #6: Audit plugin numerical resilience across subnormals (`~ -800 dBFS`) and a continuous 4-stage
/// dynamic envelope (`Quiet -60 dBFS` -> `Nominal -12 dBFS` -> `Overload +24 dBFS` -> `Recovery -12 dBFS`).
///
/// Catches:
/// 1. Numerical blowups (`NaN`, `Inf`, or runaway output `> +60 dBFS`).
/// 2. Overload auto-mutes (plugin outputs audio at `-12 dBFS` but collapses to silence on `+24 dBFS`).
/// 3. DC rail latch-up (waveshaper/filter locks onto a static DC rail on `+24 dBFS` AC input).
/// 4. Post-overload poisoned state / hang (fails to recover to nominal `-12 dBFS` output after overload).
pub fn check_headroom_and_overload(
    plugin: &mut dyn PluginInstance,
    sample_rate: f64,
) -> ValidationCheckResult {
    let t0 = Instant::now();
    let mut passed = true;
    let mut warnings = Vec::new();
    let (in_ch, out_ch) = infer_channel_counts(plugin);

    let mut nan_count = 0usize;
    let mut inf_count = 0usize;
    let mut sub_out_count = 0usize;

    // Part A: Subnormal / Denormal Storm (~ -800 dBFS)
    let n_sub = 2048;
    let subnormals = generate_signal(&SignalSpec::SubnormalNoise { seed: 99 }, sample_rate, n_sub);
    let sub_in_bufs = vec![subnormals; in_ch];

    if plugin.activate(sample_rate, 1, 2048).is_ok() && plugin.start_processing().is_ok() {
        plugin.reset();
        if let Ok((out, _, _)) = process_audio_stream(plugin, &sub_in_bufs, out_ch, n_sub, 256, &[])
        {
            for ch in &out {
                let rep = analyze_buffer_4x(ch);
                nan_count += rep.nan_count;
                inf_count += rep.inf_count;
                sub_out_count += rep.subnormal_count;
            }
        }
        plugin.stop_processing();
        plugin.deactivate();
    }

    if nan_count > 0 || inf_count > 0 {
        passed = false;
        warnings.push(format!(
            "Plugin emitted {nan_count} NaNs and {inf_count} Infs under subnormal stress"
        ));
    }
    if sub_out_count > 0 {
        warnings.push(format!(
            "Plugin leaked {sub_out_count} subnormal/denormal output samples (FTZ/DAZ recommended)"
        ));
    }

    // Part B: 4-Stage Continuous Dynamic Envelope
    // Stage 1 (Quiet):    2048 frames @ -60 dBFS (amp = 0.001)
    // Stage 2 (Nominal):  4096 frames @ -12 dBFS (amp = 0.25118864)
    // Stage 3 (Overload): 4096 frames @ +24 dBFS (amp = 15.8489319)
    // Stage 4 (Recovery): 8192 frames @ -12 dBFS (amp = 0.25118864)
    let s1_len = 2048usize;
    let s2_len = 4096usize;
    let s3_len = 4096usize;
    let s4_len = 8192usize;
    let total_env_frames = s1_len + s2_len + s3_len + s4_len;

    let amp_quiet = 10.0f64.powf(-60.0 / 20.0);
    let amp_nom = 10.0f64.powf(-12.0 / 20.0);
    let amp_hot = 10.0f64.powf(24.0 / 20.0);
    let w = 2.0 * std::f64::consts::PI * 1000.0 / sample_rate;

    let mut env_mono = Vec::with_capacity(total_env_frames);
    for i in 0..total_env_frames {
        let amp = if i < s1_len {
            amp_quiet
        } else if i < s1_len + s2_len {
            amp_nom
        } else if i < s1_len + s2_len + s3_len {
            amp_hot
        } else {
            amp_nom
        };
        env_mono.push((amp * (w * (i as f64)).sin()) as f32);
    }

    let env_in_bufs = vec![env_mono; in_ch];
    let mut env_nan = 0usize;
    let mut env_inf = 0usize;
    let mut max_env_peak_dbfs = -400.0f64;
    let mut headroom_summary = String::from("No audio output channels");

    if out_ch > 0
        && plugin.activate(sample_rate, 1, 2048).is_ok()
        && plugin.start_processing().is_ok()
    {
        plugin.reset();
        let lat = (plugin.latency_samples().unwrap_or(0) as usize).min(1536);
        if let Ok((out, _, _)) =
            process_audio_stream(plugin, &env_in_bufs, out_ch, total_env_frames, 256, &[])
        {
            for ch in &out {
                let rep = analyze_buffer_4x(ch);
                env_nan += rep.nan_count;
                env_inf += rep.inf_count;
                max_env_peak_dbfs = max_env_peak_dbfs.max(rep.sample_peak_dbfs);
            }

            if env_nan > 0 || env_inf > 0 {
                passed = false;
                warnings.push(format!(
                    "Plugin emitted {env_nan} NaNs and {env_inf} Infs during +24 dBFS overload envelope"
                ));
            }
            if max_env_peak_dbfs > 60.0 {
                passed = false;
                warnings.push(format!(
                    "Runaway output / filter blowup during +24 dBFS overload (peak = {max_env_peak_dbfs:.1} dBFS > +60 dBFS)"
                ));
            }

            // Evaluate steady-state second half of Stage 2 (Nominal), Stage 3 (+24 dBFS Overload), and tail of Stage 4 (Recovery)
            let ch0 = &out[0];
            let s2_start = (s1_len + s2_len / 2 + lat).min(total_env_frames);
            let s2_end = (s1_len + s2_len + lat).min(total_env_frames);

            let s3_start = (s1_len + s2_len + s3_len / 2 + lat).min(total_env_frames);
            let s3_end = (s1_len + s2_len + s3_len + lat).min(total_env_frames);

            let s4_start = total_env_frames.saturating_sub(2048);
            let s4_end = total_env_frames;

            let nom_rep = analyze_buffer_4x(&ch0[s2_start..s2_end]);
            let hot_rep = analyze_buffer_4x(&ch0[s3_start..s3_end]);
            let rec_rep = analyze_buffer_4x(&ch0[s4_start..s4_end]);

            let ac_rms = |r: &ChannelMeterReport| -> f64 {
                (r.rms_sample_linear * r.rms_sample_linear
                    - r.dc_offset_linear * r.dc_offset_linear)
                    .max(0.0)
                    .sqrt()
            };

            let nom_ac_rms = ac_rms(&nom_rep);
            let hot_ac_rms = ac_rms(&hot_rep);
            let rec_ac_rms = ac_rms(&rec_rep);

            let nom_ac_db = linear_to_db(nom_ac_rms);
            let hot_ac_db = linear_to_db(hot_ac_rms);
            let rec_ac_db = linear_to_db(rec_ac_rms);

            let in_nom_rms_db = linear_to_db(amp_nom / std::f64::consts::SQRT_2);
            let in_hot_rms_db = linear_to_db(amp_hot / std::f64::consts::SQRT_2);
            let nom_gain_db = nom_rep.rms_4x_dbfs - in_nom_rms_db;
            let hot_gain_db = hot_rep.rms_4x_dbfs - in_hot_rms_db;
            let rec_delta_db = (rec_rep.rms_4x_dbfs - nom_rep.rms_4x_dbfs).abs();

            // Enforce universal active-audio invariants if the plugin passes audio at nominal -12 dBFS
            if in_ch > 0 && nom_ac_db > -45.0 {
                // 1. Overload Auto-Mute / Drop-out check
                if hot_ac_db < -50.0 && hot_rep.dc_offset_linear.abs() <= 1e-3 {
                    passed = false;
                    warnings.push(format!(
                        "Overload auto-mute: plugin passed signal at -12 dBFS ({nom_ac_db:.1} dBFS AC) but muted ({hot_ac_db:.1} dBFS AC) on +24 dBFS input"
                    ));
                }

                // 2. DC Rail Latch-Up check (output locked onto constant DC rail with collapsed AC oscillation)
                if hot_rep.dc_offset_linear.abs() > 1e-3
                    && hot_ac_rms < hot_rep.dc_offset_linear.abs() * 0.5
                {
                    passed = false;
                    warnings.push(format!(
                        "Overload DC rail latch-up: on +24 dBFS AC input, output locked to DC ({:.1} dBFS) while AC RMS collapsed ({hot_ac_db:.1} dBFS)",
                        hot_rep.dc_offset_dbfs
                    ));
                }

                // 3. Post-Overload Recovery check
                if rec_ac_db < -50.0 {
                    passed = false;
                    warnings.push(format!(
                        "Post-overload hang: plugin failed to recover when input returned to -12 dBFS (recovery AC = {rec_ac_db:.1} dBFS vs nominal {nom_ac_db:.1} dBFS)"
                    ));
                } else if rec_delta_db > 3.0 {
                    warnings.push(format!(
                        "Extended post-overload release/tail decay: output at -12 dBFS differed from pre-overload nominal by {rec_delta_db:.2} dB after 128 ms"
                    ));
                } else if rec_delta_db > 1.0 {
                    warnings.push(format!(
                        "Slow post-overload recovery: output at -12 dBFS differed by {rec_delta_db:.2} dB after 128 ms"
                    ));
                }
            }

            headroom_summary = format!(
                "Nom(-12dB) Gain={nom_gain_db:+.1}dB | Hot(+24dB) Gain={hot_gain_db:+.1}dB (TP={:.1}dBTP) | Recovery delta={rec_delta_db:.2}dB",
                hot_rep.true_peak_4x_dbtp
            );
        }
        plugin.stop_processing();
        plugin.deactivate();
    }

    let total_nans = nan_count + env_nan;
    let total_infs = inf_count + env_inf;

    ValidationCheckResult {
        category: "Numerical Health".into(),
        name: "Subnormal, +24 dBFS Overload & Recovery Health".into(),
        passed,
        duration_ms: t0.elapsed().as_secs_f64() * 1e3,
        details: format!(
            "NaNs: {total_nans}, Infs: {total_infs}, Subnormals: {sub_out_count} | {headroom_summary}"
        ),
        warnings,
    }
}

/// Check #8: Bypass Null-Test & Parameter Zipper-Noise Audit
pub fn check_bypass_and_zipper_noise(
    plugin: &mut dyn PluginInstance,
    sample_rate: f64,
) -> ValidationCheckResult {
    let t8 = Instant::now();
    let mut passed8 = true;
    let mut warnings8 = Vec::new();
    let (in_ch, out_ch) = infer_channel_counts(plugin);
    let params = plugin.params();
    let bypass_param = params.iter().find(|p| p.flags.is_bypass).cloned();
    let mut bypass_detail = "No dedicated bypass parameter".to_string();

    if let Some(ref bp) = bypass_param {
        if plugin.activate(sample_rate, 1, 1024).is_ok() && plugin.start_processing().is_ok() {
            plugin.reset();
            let lat = (plugin.latency_samples().unwrap_or(0) as usize).min(16384);
            let n_frames = (lat + 1024).max(1024);
            let stim = generate_signal(
                &SignalSpec::Sine {
                    freq_hz: 440.0,
                    amplitude: 0.5,
                    phase_rad: 0.0,
                },
                sample_rate,
                n_frames,
            );
            let in_bufs = vec![stim.clone(); in_ch];
            let on_ev = [ProcessEvent::ParamValue {
                sample_offset: 0,
                param_id: bp.id,
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value: bp.max_value,
            }];
            if let Ok((out, _, _)) =
                process_audio_stream(plugin, &in_bufs, out_ch, n_frames, 256, &on_ev)
            {
                let settle = (lat + 128).min(n_frames.saturating_sub(256));
                let mut max_err_lat = 0.0f64;
                let mut max_err_zero = 0.0f64;
                for i in settle..n_frames {
                    let ref_idx = i.saturating_sub(lat);
                    let err_lat = (out[0][i] as f64 - stim[ref_idx] as f64).abs();
                    let err_zero = (out[0][i] as f64 - stim[i] as f64).abs();
                    max_err_lat = max_err_lat.max(err_lat);
                    max_err_zero = max_err_zero.max(err_zero);
                }
                let max_null_err = max_err_lat.min(max_err_zero);
                let null_db = linear_to_db(max_null_err);
                bypass_detail = format!("Bypass null residual: {null_db:.1} dBFS");
            }
            // Restore default bypass value
            let off_ev = [ProcessEvent::ParamValue {
                sample_offset: 0,
                param_id: bp.id,
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value: bp.default_value,
            }];
            let _ = process_audio_stream(plugin, &in_bufs, out_ch, 64, 64, &off_ev);
            plugin.stop_processing();
            plugin.deactivate();
        }
    }

    // Parameter Zipper-Noise / Step Slew Audit
    let mut max_zipper_slew = 0.0f64;
    if plugin.activate(sample_rate, 1, 512).is_ok() && plugin.start_processing().is_ok() {
        plugin.reset();
        let n_frames = 512;
        let dc_stim = generate_signal(&SignalSpec::DcOffset { level: 0.25 }, sample_rate, n_frames);
        let in_bufs = vec![dc_stim; in_ch];
        for p in params
            .iter()
            .filter(|p| super::is_safe_fuzz_param(p) && !p.flags.is_stepped)
            .take(4)
        {
            let step_events = [
                ProcessEvent::ParamValue {
                    sample_offset: 0,
                    param_id: p.id,
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: p.min_value,
                },
                ProcessEvent::ParamValue {
                    sample_offset: 256,
                    param_id: p.id,
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: p.max_value,
                },
                ProcessEvent::ParamValue {
                    sample_offset: 384,
                    param_id: p.id,
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: p.default_value,
                },
            ];
            if let Ok((out, _, _)) =
                process_audio_stream(plugin, &in_bufs, out_ch, n_frames, 512, &step_events)
            {
                let meter = analyze_buffer_4x(&out[0]);
                if meter.nan_count > 0 || meter.inf_count > 0 {
                    passed8 = false;
                    warnings8.push(format!(
                        "Param '{}' step jump produced NaN/Inf output",
                        p.name
                    ));
                }
                max_zipper_slew = max_zipper_slew.max(meter.max_slew_rate);
                if meter.max_slew_rate > 0.75 {
                    warnings8.push(format!(
                        "Param '{}' exhibited high step discontinuity (slew={:.3}/sample; consider parameter smoothing)",
                        p.name, meter.max_slew_rate
                    ));
                }
            }
        }
        plugin.stop_processing();
        plugin.deactivate();
    }

    ValidationCheckResult {
        category: "Bypass & Smoothing".into(),
        name: "Bypass Null-Test & Parameter Zipper-Noise Audit".into(),
        passed: passed8,
        duration_ms: t8.elapsed().as_secs_f64() * 1e3,
        details: format!("{bypass_detail} | Max step slew: {max_zipper_slew:.4}/sample"),
        warnings: warnings8,
    }
}

/// Check #9: Format-Specific Extended Host & Plugin Contracts
pub fn check_extended_contracts(plugin: &mut dyn PluginInstance) -> ValidationCheckResult {
    let t9 = Instant::now();
    let mut passed9 = true;
    let mut warnings9 = Vec::new();
    let details9 = match plugin.run_extended_contract_checks() {
        Ok(verified_list) => {
            if verified_list.is_empty() {
                "No optional extended contracts active".into()
            } else {
                verified_list.join(", ")
            }
        }
        Err(e) => {
            passed9 = false;
            warnings9.push(format!("Extended contract check failed: {e}"));
            format!("Failed: {e}")
        }
    };
    let diag9 = plugin.take_diagnostics();
    if !diag9.thread_violations.is_empty() {
        passed9 = false;
        warnings9.extend(diag9.thread_violations);
    }
    ValidationCheckResult {
        category: "Extended Contracts".into(),
        name: "Format-Specific Extended Host & Plugin Contracts".into(),
        passed: passed9,
        duration_ms: t9.elapsed().as_secs_f64() * 1e3,
        details: details9,
        warnings: warnings9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum SyntheticBehavior {
        Transparent,
        BrickwallLimiter,
        Saturator,
        BuggyAutoMute,
        BuggyDcLatch,
        BuggyPoisonedHang,
    }

    struct SyntheticPlugin {
        desc: PluginDescriptorInfo,
        behavior: SyntheticBehavior,
        poisoned: bool,
    }

    impl SyntheticPlugin {
        fn new(behavior: SyntheticBehavior) -> Self {
            Self {
                desc: PluginDescriptorInfo {
                    format: PluginFormat::Clap,
                    id: "com.hph.synthetic".into(),
                    name: "Synthetic Test Plugin".into(),
                    vendor: "HPH".into(),
                    version: "1.0".into(),
                    url: String::new(),
                    manual_url: String::new(),
                    support_url: String::new(),
                    description: String::new(),
                    features: vec!["audio-effect".into()],
                },
                behavior,
                poisoned: false,
            }
        }
    }

    impl PluginGui for SyntheticPlugin {}
    impl PluginContractChecks for SyntheticPlugin {}

    impl PluginInstance for SyntheticPlugin {
        fn descriptor(&self) -> &PluginDescriptorInfo {
            &self.desc
        }

        fn audio_ports(&self) -> Vec<AudioPortInfo> {
            vec![
                AudioPortInfo {
                    id: 0,
                    index: 0,
                    name: "In".into(),
                    is_input: true,
                    channel_count: 1,
                    is_main: true,
                    port_type: "mono".into(),
                    in_place_pair_id: None,
                    prefers_64bit: false,
                },
                AudioPortInfo {
                    id: 1,
                    index: 0,
                    name: "Out".into(),
                    is_input: false,
                    channel_count: 1,
                    is_main: true,
                    port_type: "mono".into(),
                    in_place_pair_id: None,
                    prefers_64bit: false,
                },
            ]
        }

        fn activate(
            &mut self,
            _sample_rate: f64,
            _min_frames: u32,
            _max_frames: u32,
        ) -> HostResult<()> {
            self.poisoned = false;
            Ok(())
        }

        fn deactivate(&mut self) {}

        fn reset(&mut self) {
            self.poisoned = false;
        }

        fn process(
            &mut self,
            buffers: &mut AudioProcessBuffers<'_>,
            _in_events: &[ProcessEvent],
            _out_events: &mut Vec<ProcessEvent>,
        ) -> HostResult<ProcessStatus> {
            for i in 0..buffers.num_frames {
                let x = buffers.inputs[0][i];
                let y = match self.behavior {
                    SyntheticBehavior::Transparent => x,
                    SyntheticBehavior::BrickwallLimiter => x.clamp(-1.0, 1.0),
                    SyntheticBehavior::Saturator => (x as f64).tanh() as f32,
                    SyntheticBehavior::BuggyAutoMute => {
                        if x.abs() > 2.0 {
                            0.0
                        } else {
                            x
                        }
                    }
                    SyntheticBehavior::BuggyDcLatch => {
                        if x.abs() > 2.0 {
                            1.0
                        } else {
                            x
                        }
                    }
                    SyntheticBehavior::BuggyPoisonedHang => {
                        if x.abs() > 2.0 {
                            self.poisoned = true;
                        }
                        if self.poisoned {
                            0.0
                        } else {
                            x
                        }
                    }
                };
                buffers.outputs[0][i] = if y.abs() < 1e-30 { 0.0 } else { y };
            }
            Ok(ProcessStatus::Continue)
        }
    }

    #[test]
    fn test_headroom_and_overload_passes_valid_plugin_archetypes() {
        for mode in [
            SyntheticBehavior::Transparent,
            SyntheticBehavior::BrickwallLimiter,
            SyntheticBehavior::Saturator,
        ] {
            let mut plug = SyntheticPlugin::new(mode);
            let res = check_headroom_and_overload(&mut plug, 48000.0);
            assert!(
                res.passed,
                "Expected valid plugin behavior to pass +24 dBFS overload check, got warnings: {:?} details: {}",
                res.warnings, res.details
            );
        }
    }

    #[test]
    fn test_headroom_and_overload_catches_auto_mute_dc_latch_and_hang() {
        let mut mute_plug = SyntheticPlugin::new(SyntheticBehavior::BuggyAutoMute);
        let mute_res = check_headroom_and_overload(&mut mute_plug, 48000.0);
        assert!(!mute_res.passed);
        assert!(
            mute_res
                .warnings
                .iter()
                .any(|w| w.contains("Overload auto-mute")),
            "Expected auto-mute warning, got: {:?}",
            mute_res.warnings
        );

        let mut dc_plug = SyntheticPlugin::new(SyntheticBehavior::BuggyDcLatch);
        let dc_res = check_headroom_and_overload(&mut dc_plug, 48000.0);
        assert!(!dc_res.passed);
        assert!(
            dc_res
                .warnings
                .iter()
                .any(|w| w.contains("DC rail latch-up")),
            "Expected DC rail latch warning, got: {:?}",
            dc_res.warnings
        );

        let mut hang_plug = SyntheticPlugin::new(SyntheticBehavior::BuggyPoisonedHang);
        let hang_res = check_headroom_and_overload(&mut hang_plug, 48000.0);
        assert!(!hang_res.passed);
        assert!(
            hang_res
                .warnings
                .iter()
                .any(|w| w.contains("Post-overload hang")),
            "Expected post-overload hang warning, got: {:?}",
            hang_res.warnings
        );
    }
}
