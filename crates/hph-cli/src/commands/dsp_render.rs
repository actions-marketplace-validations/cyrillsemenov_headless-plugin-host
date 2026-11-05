use crate::cli::{
    load_plugin_bundle, parse_param_events, AnalysisMode, AnalyzeOutput, CliSignalType,
    RenderOutput, SuiteCommand,
};
use anyhow::{Context, Result};
use hph_core::ProcessEvent;
use hph_dsp::*;
use hph_gui::capture_plugin_screenshot;
use hph_manifest::{
    add_or_update_step_in_place, capture_step_baseline, load_suite_file, remove_step_in_place,
    resolve_param_overrides, scaffold_suite_manifest, set_values_in_place,
    update_baselines_in_place, StepEditSpec,
};
use hph_validate::{infer_channel_counts, process_audio_stream, run_full_validation};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[allow(clippy::too_many_arguments)]
pub fn run_render(
    plugin: &Path,
    id: Option<&str>,
    signal: CliSignalType,
    freq: f64,
    amp: f64,
    duration: f64,
    sample_rate: u32,
    block_size: usize,
    input_wav: Option<PathBuf>,
    output_wav: Option<PathBuf>,
    params: &[String],
    note: Option<i16>,
    json_output: bool,
) -> Result<()> {
    let bundle = load_plugin_bundle(plugin)?;
    let mut instance = bundle.create_instance(id)?;
    let (in_ch, out_ch) = infer_channel_counts(&*instance);

    let (sr, num_frames, in_channels) = if let Some(ref wav_path) = input_wav {
        let data = read_wav_file(wav_path)?;
        let mut adapted = Vec::with_capacity(in_ch);
        for c in 0..in_ch {
            adapted.push(
                data.channels
                    .get(c % data.channels.len().max(1))
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; data.num_frames]),
            );
        }
        (data.sample_rate, data.num_frames, adapted)
    } else {
        let frames = ((duration * (sample_rate as f64)).round() as usize).max(1);
        let spec = match signal {
            CliSignalType::Silence => SignalSpec::Silence,
            CliSignalType::Sine => SignalSpec::Sine {
                freq_hz: freq,
                amplitude: amp,
                phase_rad: 0.0,
            },
            CliSignalType::Saw => SignalSpec::Saw {
                freq_hz: freq,
                amplitude: amp,
            },
            CliSignalType::Square => SignalSpec::Square {
                freq_hz: freq,
                amplitude: amp,
                duty: 0.5,
            },
            CliSignalType::Triangle => SignalSpec::Triangle {
                freq_hz: freq,
                amplitude: amp,
            },
            CliSignalType::Impulse => SignalSpec::Impulse {
                offset_samples: 0,
                amplitude: amp,
            },
            CliSignalType::Triggers => SignalSpec::TriggerTrain {
                freq_hz: freq,
                pulse_samples: 4,
                amplitude: amp,
            },
            CliSignalType::WhiteNoise => SignalSpec::WhiteNoise {
                amplitude: amp,
                seed: 1,
            },
            CliSignalType::PinkNoise => SignalSpec::PinkNoise {
                amplitude: amp,
                seed: 1,
            },
            CliSignalType::Subnormals => SignalSpec::SubnormalNoise { seed: 1 },
            CliSignalType::Sweep => SignalSpec::LogSweep {
                start_hz: 20.0,
                end_hz: (sample_rate as f64) * 0.45,
                amplitude: amp,
            },
        };
        let mono = generate_signal(&spec, sample_rate as f64, frames);
        (sample_rate, frames, vec![mono; in_ch])
    };

    let mut events = parse_param_events(params)?;
    if let Some(key) = note {
        events.push(ProcessEvent::NoteOn {
            sample_offset: 0,
            port_index: 0,
            channel: 0,
            key,
            note_id: 1,
            velocity: 0.85,
        });
    }

    instance.activate(sr as f64, 1, block_size.max(64) as u32)?;
    instance.start_processing()?;
    instance.reset();

    let (out_channels, _, rt_allocs) = process_audio_stream(
        &mut *instance,
        &in_channels,
        out_ch,
        num_frames,
        block_size,
        &events,
    )?;

    instance.stop_processing();
    instance.deactivate();

    if let Some(ref out_path) = output_wav {
        write_wav_file_f32(out_path, sr, &out_channels)?;
    }

    let meter_reports: Vec<ChannelMeterReport> =
        out_channels.iter().map(|c| analyze_buffer_4x(c)).collect();

    let out = RenderOutput {
        plugin: instance.descriptor().name.clone(),
        sample_rate: sr,
        num_frames,
        output_wav: output_wav.as_ref().map(|p| p.display().to_string()),
        rt_allocs,
        channels: meter_reports,
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!(
            "=== Render & 4x Oversampled Meter Summary: {} ===",
            out.plugin
        );
        println!("  Frames: {} @ {} Hz", out.num_frames, out.sample_rate);
        if let Some(ref p) = out.output_wav {
            println!("  Saved WAV: {p}");
        }
        println!(
            "  RT Allocations: {} allocs, {} frees ({} bytes)",
            out.rt_allocs.alloc_calls, out.rt_allocs.free_calls, out.rt_allocs.bytes_allocated
        );
        for (idx, ch) in out.channels.iter().enumerate() {
            println!(
                "  Channel {idx}: SamplePeak={:.2} dBFS | 4x TruePeak={:.2} dBTP (ISP +{:.2} dB) | 4x RMS={:.2} dBFS | LUFS={:.2} | Crest={:.2} dB | DC={:.2} dBFS | NaNs={} Subnormals={}",
                ch.sample_peak_dbfs,
                ch.true_peak_4x_dbtp,
                ch.intersample_overshoot_db,
                ch.rms_4x_dbfs,
                ch.k_weighted_lufs,
                ch.crest_factor_db,
                ch.dc_offset_dbfs,
                ch.nan_count,
                ch.subnormal_count
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_analyze(
    plugin: &Path,
    id: Option<&str>,
    mode: AnalysisMode,
    sample_rate: f64,
    freq: f64,
    tail_sec: f64,
    params: &[String],
    json_output: bool,
) -> Result<()> {
    let bundle = load_plugin_bundle(plugin)?;
    let mut instance = bundle.create_instance(id)?;
    let (in_ch, out_ch) = infer_channel_counts(&*instance);
    let param_events = parse_param_events(params)?;

    let do_spec = matches!(mode, AnalysisMode::Spectrum | AnalysisMode::All);
    let do_thdn = matches!(mode, AnalysisMode::Thdn | AnalysisMode::All);
    let do_bode = matches!(mode, AnalysisMode::Bode | AnalysisMode::All);
    let do_imd = matches!(mode, AnalysisMode::Imd | AnalysisMode::All);
    let do_rt60 = matches!(mode, AnalysisMode::Rt60 | AnalysisMode::All);
    let do_delay = matches!(mode, AnalysisMode::Delay | AnalysisMode::All);
    let do_dynamics = matches!(mode, AnalysisMode::Dynamics | AnalysisMode::All);
    let do_stereo = matches!(mode, AnalysisMode::Stereo | AnalysisMode::All);

    let mut spec_report = None;
    let mut thdn_report = None;
    let mut bode_report = None;
    let mut imd_report = None;
    let mut rt60_report = None;
    let mut delay_report = None;
    let mut dynamics_report = None;
    let mut stereo_report = None;

    if do_spec || do_thdn || do_stereo {
        let n = sample_rate.round() as usize; // 1.0 sec => exact 1 Hz bins
        let sine = generate_signal(
            &SignalSpec::Sine {
                freq_hz: freq,
                amplitude: 0.5,
                phase_rad: 0.0,
            },
            sample_rate,
            n,
        );
        let in_bufs = vec![sine; in_ch];
        instance.activate(sample_rate, 1, 512)?;
        instance.start_processing()?;
        instance.reset();
        let (out_bufs, _, _) =
            process_audio_stream(&mut *instance, &in_bufs, out_ch, n, 256, &param_events)?;
        instance.stop_processing();
        instance.deactivate();

        // Skip first 4096 samples of filter/envelope transient settle
        let steady_slice = if out_bufs[0].len() > 8192 {
            &out_bufs[0][4096..]
        } else {
            &out_bufs[0][..]
        };

        if do_spec {
            spec_report = Some(compute_spectrum(
                steady_slice,
                sample_rate,
                WindowType::BlackmanHarris,
            ));
        }
        if do_thdn {
            thdn_report = Some(analyze_thdn(steady_slice, sample_rate, freq));
        }
        if do_stereo {
            let right_slice = if out_bufs.len() >= 2 {
                if out_bufs[1].len() > 8192 {
                    &out_bufs[1][4096..]
                } else {
                    &out_bufs[1][..]
                }
            } else {
                steady_slice
            };
            stereo_report = Some(analyze_stereo_image(steady_slice, right_slice));
        }
    }

    if do_imd {
        let n = sample_rate.round() as usize;
        let f1 = 19000.0f64.min(sample_rate * 0.38);
        let f2 = 20000.0f64.min(sample_rate * 0.42);
        let two_tone = generate_signal(
            &SignalSpec::TwoTone {
                freq1_hz: f1,
                amp1: 0.35,
                freq2_hz: f2,
                amp2: 0.35,
            },
            sample_rate,
            n,
        );
        let in_bufs = vec![two_tone; in_ch];
        instance.activate(sample_rate, 1, 512)?;
        instance.start_processing()?;
        instance.reset();
        let (out_bufs, _, _) =
            process_audio_stream(&mut *instance, &in_bufs, out_ch, n, 256, &param_events)?;
        instance.stop_processing();
        instance.deactivate();

        let steady_slice = if out_bufs[0].len() > 8192 {
            &out_bufs[0][4096..]
        } else {
            &out_bufs[0][..]
        };
        imd_report = Some(analyze_imd(steady_slice, sample_rate, f1, f2));
    }

    if do_bode || do_rt60 || do_delay {
        let n = if do_rt60 {
            ((tail_sec.max(0.5) * sample_rate).round() as usize).max(16384)
        } else {
            16384
        };
        let impulse = generate_signal(
            &SignalSpec::Impulse {
                offset_samples: 0,
                amplitude: 0.8,
            },
            sample_rate,
            n,
        );
        let in_bufs = vec![impulse.clone(); in_ch];
        instance.activate(sample_rate, 1, 512)?;
        instance.start_processing()?;
        instance.reset();
        let reported_lat = instance.latency_samples();
        let (out_bufs, _, _) =
            process_audio_stream(&mut *instance, &in_bufs, out_ch, n, 256, &param_events)?;
        instance.stop_processing();
        instance.deactivate();

        if do_bode {
            let bode_len = n.min(16384);
            bode_report = Some(compute_bode_response(
                &impulse[..bode_len],
                &out_bufs[0][..bode_len],
                sample_rate,
            ));
        }
        if do_rt60 {
            rt60_report = Some(analyze_rt60(&out_bufs[0], sample_rate));
        }
        if do_delay {
            delay_report = Some(analyze_delay_and_latency(
                &impulse,
                &out_bufs[0],
                sample_rate,
                reported_lat,
            ));
        }
    }

    if do_dynamics {
        dynamics_report = Some(analyze_dynamics_envelope(sample_rate, |mono_stim| {
            let frames = mono_stim.len();
            let in_bufs = vec![mono_stim.to_vec(); in_ch];
            let _ = instance.activate(sample_rate, 1, 512);
            let _ = instance.start_processing();
            instance.reset();
            let res =
                process_audio_stream(&mut *instance, &in_bufs, out_ch, frames, 256, &param_events);
            instance.stop_processing();
            instance.deactivate();
            res.map(|(mut chs, _, _)| chs.swap_remove(0))
                .unwrap_or_else(|_| vec![0.0; frames])
        }));
    }

    let out = AnalyzeOutput {
        plugin: instance.descriptor().name.clone(),
        sample_rate,
        spectrum: spec_report,
        bode: bode_report,
        thdn: thdn_report,
        imd: imd_report,
        rt60: rt60_report,
        delay: delay_report,
        dynamics: dynamics_report,
        stereo: stereo_report,
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("=== DSP Analysis: {} ===", out.plugin);
        if let Some(ref s) = out.spectrum {
            println!(
                "\n[Spectrum] Peak={:.2} Hz ({:.2} dBFS) | Centroid={:.1} Hz | Flatness={:.4}",
                s.peak_freq_hz, s.peak_magnitude_db, s.spectral_centroid_hz, s.spectral_flatness
            );
        }
        if let Some(ref t) = out.thdn {
            println!(
                "\n[THD+N @ {:.1} Hz] Fundamental={:.2} dBFS | THD={:.4}% ({:.2} dB) | Even={:.2} dB | Odd={:.2} dB | THD+N={:.4}% ({:.2} dB) | SINAD={:.2} dB | Max Alias Spur={:.2} dBc",
                t.fundamental_freq_hz,
                t.fundamental_level_dbfs,
                t.thd_percent,
                t.thd_db,
                t.even_thd_db,
                t.odd_thd_db,
                t.thdn_percent,
                t.thdn_db,
                t.sinad_db,
                t.max_alias_spur_dbc
            );
            for h in &t.harmonics {
                println!(
                    "  H{:<2} ({:>7.1} Hz): {:>7.2} dBc",
                    h.order, h.freq_hz, h.level_dbc
                );
            }
        }
        if let Some(ref imd) = out.imd {
            println!(
                "\n[Two-Tone IMD ({:.0} Hz + {:.0} Hz)] IMD={:.4}% ({:.2} dB) | Diff (f2-f1)={:.2} dBc | IMD3-={:.2} dBc | IMD3+={:.2} dBc",
                imd.f1_hz,
                imd.f2_hz,
                imd.imd_percent,
                imd.imd_db,
                imd.diff_tone_dbc,
                imd.third_order_lower_dbc,
                imd.third_order_upper_dbc
            );
        }
        if let Some(ref b) = out.bode {
            println!(
                "\n[Bode Frequency & Group Delay Response] DC={:.2} dB | 1kHz={:.2} dB | -3dB Cutoff={:?} Hz",
                b.dc_gain_db, b.gain_1khz_db, b.cutoff_minus_3db_hz
            );
            for pt in &b.points {
                println!(
                    "  {:>8.1} Hz : {:>7.2} dB  ({:>7.1} deg, {:>7.3} ms group delay)",
                    pt.freq_hz, pt.magnitude_db, pt.phase_deg, pt.group_delay_ms
                );
            }
        }
        if let Some(ref r) = out.rt60 {
            println!(
                "\n[Reverb RT60 (ISO 3382 Schroeder EDC)] RT60={:.3} s (EDT={:.3} s, T20={:.3} s, T30={:.3} s, R^2={:.4}, DR={:.1} dB)",
                r.rt60_sec, r.edt_sec, r.t20_sec, r.t30_sec, r.r_squared, r.dynamic_range_db
            );
            for band in &r.octave_bands {
                println!(
                    "  Octave {:>6.0} Hz : RT60={:.3} s (R^2={:.3})",
                    band.center_freq_hz, band.rt60_sec, band.r_squared
                );
            }
        }
        if let Some(ref d) = out.delay {
            println!(
                "\n[Delay & Latency] Onset={} samples ({:.3} ms) | Peak={:.2} samples ({:.3} ms) | XCorr={:.2} samples ({:.3} ms) | Reported={:?} samples (Delta={:?})",
                d.onset_delay_samples,
                d.onset_delay_ms,
                d.peak_delay_samples,
                d.peak_delay_ms,
                d.xcorr_delay_samples,
                d.xcorr_delay_ms,
                d.reported_latency_samples,
                d.latency_discrepancy_samples
            );
            if let (Some(sp), Some(fb)) =
                (d.estimated_tap_spacing_ms, d.estimated_feedback_db_per_tap)
            {
                println!(
                    "  Echo Taps ({}): Avg Spacing={:.2} ms | Avg Decay={:.2} dB/tap",
                    d.echo_taps.len(),
                    sp,
                    fb
                );
            }
            for tap in d.echo_taps.iter().take(6) {
                println!(
                    "    Tap #{:<2} @ {:>8.2} ms ({:>6} smp): {:>7.2} dBFS ({:>6.2} dBc)",
                    tap.tap_index, tap.time_ms, tap.sample_index, tap.level_dbfs, tap.relative_db
                );
            }
        }
        if let Some(ref dyn_rep) = out.dynamics {
            println!(
                "\n[Dynamics / Compressor] Est. Ratio={:.2}:1 | Threshold={:?} dBFS | Max GR={:.2} dB | Attack={:.2} ms | Release={:.2} ms",
                dyn_rep.estimated_ratio,
                dyn_rep.estimated_threshold_dbfs,
                dyn_rep.max_gain_reduction_db,
                dyn_rep.attack_time_ms,
                dyn_rep.release_time_ms
            );
            for pt in &dyn_rep.static_curve {
                println!(
                    "  In {:>+6.1} dBFS -> Out {:>+6.2} dBFS (Gain {:>+6.2} dB)",
                    pt.input_dbfs, pt.output_dbfs, pt.gain_db
                );
            }
        }
        if let Some(ref st) = out.stereo {
            println!(
                "\n[Stereo Image] Phase Correlation={:+.4} | L/R Balance={:+.2} dB | Mid={:.2} dBFS | Side={:.2} dBFS | Width={:.3}",
                st.phase_correlation,
                st.lr_balance_db,
                st.mid_rms_dbfs,
                st.side_rms_dbfs,
                st.stereo_width
            );
        }
    }
    Ok(())
}

pub fn run_suite_command(action: &SuiteCommand) -> Result<()> {
    match action {
        SuiteCommand::Init {
            plugin,
            id,
            output,
            sample_rate,
            datasheet,
            capture,
        } => {
            let bundle = load_plugin_bundle(plugin)?;
            let mut instance = bundle.create_instance(id.as_deref())?;
            let toml_text = scaffold_suite_manifest(
                &plugin.to_string_lossy(),
                id.as_deref(),
                &mut *instance,
                *sample_rate,
                datasheet.as_deref(),
                *capture,
            )?;
            std::fs::write(output, toml_text)
                .with_context(|| format!("Failed to write '{}'", output.display()))?;
            println!(
                "Scaffolded suite manifest '{}' for '{}' (capture={})",
                output.display(),
                instance.descriptor().name,
                capture
            );
            Ok(())
        }
        SuiteCommand::Add {
            suite_file,
            name,
            params,
            signal,
            freq,
            amp,
            frames,
            assertions,
            output_wav,
            screenshot_png,
            capture,
        } => {
            let cfg = load_suite_file(suite_file)?;
            let bundle = load_plugin_bundle(Path::new(&cfg.plugin))?;
            let mut instance = bundle.create_instance(cfg.plugin_id.as_deref())?;
            let resolved_params = resolve_param_overrides(&*instance, params)?;

            let sig_spec = match *signal {
                CliSignalType::Silence => SignalSpec::Silence,
                CliSignalType::Sine => SignalSpec::Sine {
                    freq_hz: *freq,
                    amplitude: *amp,
                    phase_rad: 0.0,
                },
                CliSignalType::Saw => SignalSpec::Saw {
                    freq_hz: *freq,
                    amplitude: *amp,
                },
                CliSignalType::Square => SignalSpec::Square {
                    freq_hz: *freq,
                    amplitude: *amp,
                    duty: 0.5,
                },
                CliSignalType::Triangle => SignalSpec::Triangle {
                    freq_hz: *freq,
                    amplitude: *amp,
                },
                CliSignalType::Impulse => SignalSpec::Impulse {
                    offset_samples: 0,
                    amplitude: *amp,
                },
                CliSignalType::Triggers => SignalSpec::TriggerTrain {
                    freq_hz: *freq,
                    pulse_samples: 4,
                    amplitude: *amp,
                },
                CliSignalType::WhiteNoise => SignalSpec::WhiteNoise {
                    amplitude: *amp,
                    seed: 1,
                },
                CliSignalType::PinkNoise => SignalSpec::PinkNoise {
                    amplitude: *amp,
                    seed: 1,
                },
                CliSignalType::Subnormals => SignalSpec::SubnormalNoise { seed: 1 },
                CliSignalType::Sweep => SignalSpec::LogSweep {
                    start_hz: 20.0,
                    end_hz: cfg.sample_rate * 0.45,
                    amplitude: *amp,
                },
            };

            let mut assert_map = BTreeMap::new();
            for item in assertions {
                let (k, v) = item
                    .split_once('=')
                    .with_context(|| format!("Invalid --assert '{item}', expected KEY=VALUE"))?;
                let val: f64 = v
                    .trim()
                    .parse()
                    .with_context(|| format!("Invalid assertion float '{v}'"))?;
                assert_map.insert(k.trim().to_string(), val);
            }

            let cap = if *capture {
                let param_pairs: Vec<(u32, f64)> =
                    resolved_params.iter().map(|p| (p.id, p.value)).collect();
                Some(capture_step_baseline(
                    &mut *instance,
                    cfg.sample_rate,
                    *frames,
                    &sig_spec,
                    &param_pairs,
                )?)
            } else {
                None
            };

            let updated = add_or_update_step_in_place(
                suite_file,
                &StepEditSpec {
                    name: name.clone(),
                    frames: *frames,
                    signal: sig_spec,
                    params: resolved_params,
                    assertions: assert_map,
                    output_wav: output_wav.clone(),
                    screenshot_png: screenshot_png.clone(),
                    require_zero_rt_allocs: true,
                },
                cap.as_ref(),
            )?;
            println!(
                "{} step '{}' in '{}' (capture={})",
                if updated { "Updated" } else { "Added" },
                name,
                suite_file.display(),
                capture
            );
            Ok(())
        }
        SuiteCommand::Remove { suite_file, name } => {
            let removed = remove_step_in_place(suite_file, name)?;
            if removed {
                println!("Removed step '{}' from '{}'", name, suite_file.display());
            } else {
                anyhow::bail!("Step '{}' not found in '{}'", name, suite_file.display());
            }
            Ok(())
        }
        SuiteCommand::Set {
            suite_file,
            step,
            sets,
            unsets,
        } => {
            let cfg = load_suite_file(suite_file)?;
            let bundle_opt = load_plugin_bundle(Path::new(&cfg.plugin)).ok();
            let inst_opt = bundle_opt
                .as_ref()
                .and_then(|b| b.create_instance(cfg.plugin_id.as_deref()).ok());
            set_values_in_place(
                suite_file,
                step.as_deref(),
                sets,
                unsets,
                inst_opt.as_deref(),
            )?;
            println!(
                "Updated '{}' ({})",
                suite_file.display(),
                step.as_deref().unwrap_or("top-level")
            );
            Ok(())
        }
        SuiteCommand::Show { suite_file } => {
            let cfg = load_suite_file(suite_file)?;
            println!(
                "Suite Manifest: {} | Plugin: {} | SR: {:.0} Hz | Validate: {} | Steps: {}",
                suite_file.display(),
                cfg.plugin,
                cfg.sample_rate,
                cfg.run_validation,
                cfg.tests.len()
            );
            for (i, st) in cfg.tests.iter().enumerate() {
                println!(
                    "  [{}] '{}' (frames={}, params={}, zero_rt_allocs={})",
                    i + 1,
                    st.name,
                    st.frames,
                    st.params.len(),
                    st.require_zero_rt_allocs
                );
            }
            Ok(())
        }
        SuiteCommand::Run {
            suite_file,
            update_baselines,
            step,
        } => {
            if *update_baselines {
                let cfg = load_suite_file(suite_file)?;
                let bundle = load_plugin_bundle(Path::new(&cfg.plugin))?;
                let mut instance = bundle.create_instance(cfg.plugin_id.as_deref())?;
                let count = update_baselines_in_place(suite_file, step.as_deref(), &mut *instance)?;
                println!(
                    "Updated baselines for {} step(s) in '{}'",
                    count,
                    suite_file.display()
                );
            }
            run_suite(suite_file)
        }
    }
}

pub fn run_suite(suite_file: &Path) -> Result<()> {
    let cfg = load_suite_file(suite_file)?;
    let bundle = load_plugin_bundle(Path::new(&cfg.plugin))?;
    let mut instance = bundle.create_instance(cfg.plugin_id.as_deref())?;

    let mut failures = Vec::new();
    if cfg.run_validation {
        let rep = run_full_validation(&mut *instance, cfg.sample_rate);
        if rep.failed_checks > 0 {
            failures.push(format!(
                "Validation suite had {} failed checks",
                rep.failed_checks
            ));
        }
    }

    let (in_ch, out_ch) = infer_channel_counts(&*instance);
    for step in &cfg.tests {
        let mut events = Vec::new();
        for (k, &v) in &step.params {
            if let Ok(pid) = k.parse::<u32>() {
                events.push(ProcessEvent::ParamValue {
                    sample_offset: 0,
                    param_id: pid,
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: v,
                });
            }
        }

        let sig = step.signal.clone().unwrap_or(SignalSpec::Sine {
            freq_hz: 1000.0,
            amplitude: 0.5,
            phase_rad: 0.0,
        });
        let mono = generate_signal(&sig, cfg.sample_rate, step.frames);
        let in_bufs = vec![mono.clone(); in_ch];

        instance.activate(cfg.sample_rate, 1, 512)?;
        instance.start_processing()?;
        instance.reset();
        let reported_lat = instance.latency_samples();
        let (out_bufs, _, rt_stats) =
            process_audio_stream(&mut *instance, &in_bufs, out_ch, step.frames, 256, &events)?;
        instance.stop_processing();
        instance.deactivate();

        if let Some(ref wav_out) = step.output_wav {
            write_wav_file_f32(Path::new(wav_out), cfg.sample_rate as u32, &out_bufs)?;
        }
        if let Some(ref png_out) = step.screenshot_png {
            let _ = capture_plugin_screenshot(&mut *instance, Path::new(png_out), 150, None)?;
        }

        let in_meter = analyze_buffer_4x(&mono);
        let meter = analyze_buffer_4x(&out_bufs[0]);
        let gain_db = meter.rms_4x_dbfs - in_meter.rms_4x_dbfs;
        if step.require_zero_rt_allocs && rt_stats.has_violations() {
            failures.push(format!(
                "[{}] RT allocation violation: {:?}",
                step.name, rt_stats
            ));
        }
        if let Some(min_tp) = step.min_true_peak_dbtp {
            if meter.true_peak_4x_dbtp < min_tp {
                failures.push(format!(
                    "[{}] True-Peak {:.2} dBTP below min {:.2} dBTP",
                    step.name, meter.true_peak_4x_dbtp, min_tp
                ));
            }
        }
        if let Some(max_tp) = step.max_true_peak_dbtp {
            if meter.true_peak_4x_dbtp > max_tp {
                failures.push(format!(
                    "[{}] True-Peak {:.2} dBTP exceeded max {:.2} dBTP",
                    step.name, meter.true_peak_4x_dbtp, max_tp
                ));
            }
        }
        if let Some(min_rms) = step.min_rms_dbfs {
            if meter.rms_4x_dbfs < min_rms {
                failures.push(format!(
                    "[{}] 4x RMS {:.2} dBFS below min {:.2} dBFS",
                    step.name, meter.rms_4x_dbfs, min_rms
                ));
            }
        }
        if let Some(max_rms) = step.max_rms_dbfs {
            if meter.rms_4x_dbfs > max_rms {
                failures.push(format!(
                    "[{}] 4x RMS {:.2} dBFS exceeded max {:.2} dBFS",
                    step.name, meter.rms_4x_dbfs, max_rms
                ));
            }
        }
        if let Some(min_gain) = step.min_gain_db {
            if gain_db < min_gain {
                failures.push(format!(
                    "[{}] Gain {:.2} dB below min {:.2} dB (in={:.2} dBFS, out={:.2} dBFS)",
                    step.name, gain_db, min_gain, in_meter.rms_4x_dbfs, meter.rms_4x_dbfs
                ));
            }
        }
        if let Some(max_gain) = step.max_gain_db {
            if gain_db > max_gain {
                failures.push(format!(
                    "[{}] Gain {:.2} dB exceeded max {:.2} dB (in={:.2} dBFS, out={:.2} dBFS)",
                    step.name, gain_db, max_gain, in_meter.rms_4x_dbfs, meter.rms_4x_dbfs
                ));
            }
        }
        if step.min_thd_percent.is_some() || step.max_thd_percent.is_some() {
            let thdn = analyze_thdn(&out_bufs[0], cfg.sample_rate, 1000.0);
            if let Some(min_thd) = step.min_thd_percent {
                if thdn.thd_percent < min_thd {
                    failures.push(format!(
                        "[{}] THD {:.4}% below min {:.4}%",
                        step.name, thdn.thd_percent, min_thd
                    ));
                }
            }
            if let Some(max_thd) = step.max_thd_percent {
                if thdn.thd_percent > max_thd {
                    failures.push(format!(
                        "[{}] THD {:.4}% exceeded max {:.4}%",
                        step.name, thdn.thd_percent, max_thd
                    ));
                }
            }
        }
        if let Some(max_dc) = step.max_dc_offset_dbfs {
            if meter.dc_offset_dbfs > max_dc {
                failures.push(format!(
                    "[{}] DC offset {:.2} dBFS exceeded max {:.2} dBFS",
                    step.name, meter.dc_offset_dbfs, max_dc
                ));
            }
        }
        if let Some(max_imd) = step.max_imd_percent {
            let imd = analyze_imd(&out_bufs[0], cfg.sample_rate, 19000.0, 20000.0);
            if imd.imd_percent > max_imd {
                failures.push(format!(
                    "[{}] IMD {:.4}% exceeded max {:.4}%",
                    step.name, imd.imd_percent, max_imd
                ));
            }
        }
        if step.min_rt60_sec.is_some() || step.max_rt60_sec.is_some() {
            let rt60 = analyze_rt60(&out_bufs[0], cfg.sample_rate);
            if let Some(min_r) = step.min_rt60_sec {
                if rt60.rt60_sec < min_r {
                    failures.push(format!(
                        "[{}] RT60 {:.3}s below min {:.3}s",
                        step.name, rt60.rt60_sec, min_r
                    ));
                }
            }
            if let Some(max_r) = step.max_rt60_sec {
                if rt60.rt60_sec > max_r {
                    failures.push(format!(
                        "[{}] RT60 {:.3}s exceeded max {:.3}s",
                        step.name, rt60.rt60_sec, max_r
                    ));
                }
            }
        }
        if let Some(exp_delay_ms) = step.expected_delay_ms {
            let d = analyze_delay_and_latency(&mono, &out_bufs[0], cfg.sample_rate, reported_lat);
            if (d.peak_delay_ms - exp_delay_ms).abs() > 0.5 {
                failures.push(format!(
                    "[{}] Peak delay {:.3} ms differed from expected {:.3} ms",
                    step.name, d.peak_delay_ms, exp_delay_ms
                ));
            }
        }
        if let Some(min_corr) = step.min_stereo_correlation {
            let right = if out_bufs.len() >= 2 {
                &out_bufs[1]
            } else {
                &out_bufs[0]
            };
            let st = analyze_stereo_image(&out_bufs[0], right);
            if st.phase_correlation < min_corr {
                failures.push(format!(
                    "[{}] Stereo phase correlation {:.4} below min {:.4}",
                    step.name, st.phase_correlation, min_corr
                ));
            }
        }
        println!(
            "Step '{}': 4x TP={:.2} dBTP, 4x RMS={:.2} dBFS, LUFS={:.2}, RT allocs={}",
            step.name,
            meter.true_peak_4x_dbtp,
            meter.rms_4x_dbfs,
            meter.k_weighted_lufs,
            rt_stats.alloc_calls
        );
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("SUITE FAILURE: {f}");
        }
        std::process::exit(1);
    }
    if let Some(ref html_out) = cfg.datasheet_html {
        run_datasheet(
            Path::new(&cfg.plugin),
            cfg.plugin_id.as_deref(),
            Path::new(html_out),
            cfg.sample_rate,
            cfg.sweep_param,
            None,
            &[],
            false,
        )?;
    }
    println!("All suite steps passed!");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_datasheet(
    plugin_path: &Path,
    plugin_id: Option<&str>,
    output_html: &Path,
    sample_rate: f64,
    sweep_param: Option<u32>,
    reference_plugin: Option<&Path>,
    param_overrides: &[String],
    json: bool,
) -> Result<()> {
    let bundle = load_plugin_bundle(plugin_path)?;
    let mut instance = bundle.create_instance(plugin_id)?;

    let mut ref_bundle_holder = None;
    let mut ref_instance_holder: Option<Box<dyn hph_core::PluginInstance>> = None;
    if let Some(ref_path) = reference_plugin {
        let rb = load_plugin_bundle(ref_path)?;
        let ri = rb.create_instance(None)?;
        ref_bundle_holder = Some(rb);
        ref_instance_holder = Some(ri);
    }
    let _ = &ref_bundle_holder;

    let base_events = parse_param_events(param_overrides)?;
    let ref_mut: Option<&mut dyn hph_core::PluginInstance> = match ref_instance_holder {
        Some(ref mut boxed) => Some(&mut **boxed),
        None => None,
    };
    let report = hph_datasheet::generate_plugin_datasheet(
        &mut *instance,
        ref_mut,
        output_html,
        sample_rate,
        sweep_param,
        &base_events,
    )?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "Generated printable HTML/SVG datasheet for '{}' ({}) -> {}",
            report.descriptor.name, report.descriptor.id, report.output_html
        );
        println!(
            "  Fingerprint: EQ={}, Harmonics={}, Dynamics={}, SpatialTail={} | 1kHz Gain={:+.2} dB, THD+N={:.4}%, RT60={:.3}s",
            report.fingerprint.frequency_shaping_active,
            report.fingerprint.nonlinear_harmonics_active,
            report.fingerprint.dynamic_compression_active,
            report.fingerprint.spatial_tail_active,
            report.primary_bode.gain_1khz_db,
            report.primary_thdn.thdn_percent,
            report.primary_rt60.rt60_sec,
        );
        if report.multibus.has_sidechain_input || report.multibus.has_aux_outputs {
            println!(
                "  Multi-Bus Routing: InBuses={}, OutBuses={} ({} active) | Sidechain Ducking={:?} dB",
                report.multibus.input_buses.len(),
                report.multibus.output_buses.len(),
                report.multibus.active_output_stems,
                report.multibus.sidechain_ducking_db,
            );
        }
        if let Some(ref inst) = report.instrument_sweep {
            println!(
                "  Instrument Matrix: {} registers | Mean Pitch Error={:.2} cents, Max Vel DR={:.2} dB, Brightness Tilt={:.2}x",
                inst.registers.len(),
                inst.mean_abs_pitch_error_cents,
                inst.max_velocity_dynamic_range_db,
                inst.mean_brightness_tilt_ratio,
            );
        }
    }

    Ok(())
}
