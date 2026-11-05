pub mod render_html;
pub mod svg_plot;

use hph_core::*;
use hph_dsp::*;
use hph_gui::capture_plugin_screenshot;
use hph_validate::*;
use render_html::build_printable_html_datasheet;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct SweepTraceSnapshot {
    pub label: String,
    pub bode: BodeReport,
    pub thdn: ThdNReport,
    pub dynamics: DynamicsReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatasheetFingerprint {
    pub frequency_shaping_active: bool,
    pub nonlinear_harmonics_active: bool,
    pub dynamic_compression_active: bool,
    pub spatial_tail_active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BusTelemetryEntry {
    pub index: u32,
    pub id: u32,
    pub name: String,
    pub is_input: bool,
    pub is_main: bool,
    pub channel_count: u32,
    pub port_type: String,
    pub measured_peak_dbfs: Option<f64>,
    pub measured_rms_dbfs: Option<f64>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiBusRoutingReport {
    pub input_buses: Vec<BusTelemetryEntry>,
    pub output_buses: Vec<BusTelemetryEntry>,
    pub has_sidechain_input: bool,
    pub has_aux_outputs: bool,
    pub active_output_stems: usize,
    pub sidechain_ducking_db: Option<f64>,
    pub sidechain_key_param_name: Option<String>,
    pub sidechain_dynamics: Option<DynamicsReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstrumentVelocityPoint {
    pub velocity: f64,
    pub peak_dbfs: f64,
    pub rms_dbfs: f64,
    pub fundamental_hz: f64,
    pub spectral_centroid_hz: f64,
    pub thd_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstrumentRegisterRow {
    pub note_name: String,
    pub midi_key: i16,
    pub nominal_freq_hz: f64,
    pub measured_fundamental_hz: f64,
    pub pitch_error_cents: f64,
    pub pitch_tracked: bool,
    pub velocity_points: Vec<InstrumentVelocityPoint>,
    pub velocity_dynamic_range_db: f64,
    pub brightness_tilt_ratio: f64,
    pub spectral_envelope: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstrumentSweepReport {
    pub is_percussive: bool,
    pub registers: Vec<InstrumentRegisterRow>,
    pub mean_abs_pitch_error_cents: f64,
    pub max_velocity_dynamic_range_db: f64,
    pub mean_brightness_tilt_ratio: f64,
}

#[derive(Debug, Serialize)]
pub struct DatasheetReport {
    pub descriptor: PluginDescriptorInfo,
    pub sample_rate: f64,
    pub output_html: String,
    pub fingerprint: DatasheetFingerprint,
    pub latency_samples: Option<u32>,
    pub tail_samples: Option<u32>,
    pub validation: ValidationReport,
    pub benchmark: BenchmarkReport,
    pub primary_bode: BodeReport,
    pub primary_thdn: ThdNReport,
    pub primary_imd: ImdReport,
    pub primary_rt60: Rt60Report,
    pub primary_delay: DelayReport,
    pub primary_dynamics: DynamicsReport,
    pub primary_stereo: StereoReport,
    pub multibus: MultiBusRoutingReport,
    pub instrument_sweep: Option<InstrumentSweepReport>,
    pub sweep_traces: Vec<SweepTraceSnapshot>,
    pub reference_name: Option<String>,
}

fn base64_encode(bytes: &[u8]) -> String {
    const LUT: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(LUT[((triple >> 18) & 0x3F) as usize] as char);
        out.push(LUT[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(LUT[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(LUT[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn is_instrument_plugin(instance: &dyn PluginInstance, in_ch: usize) -> bool {
    let has_note_in = instance.note_ports().iter().any(|p| p.is_input);
    if !has_note_in {
        return false;
    }
    if in_ch == 0 {
        return true;
    }
    instance.descriptor().features.iter().any(|f| {
        let fl = f.to_ascii_lowercase();
        fl.contains("instrument") || fl.contains("synth") || fl.contains("aumu")
    })
}

fn default_instrument_midi_key(instance: &dyn PluginInstance) -> i16 {
    let desc = instance.descriptor();
    let is_drum = desc.name.to_ascii_lowercase().contains("drum")
        || desc
            .features
            .iter()
            .any(|f| f.to_ascii_lowercase().contains("drum"));
    if is_drum {
        36 // General MIDI C1 Bass Drum 1
    } else {
        69 // A4 (440 Hz)
    }
}

pub fn measure_trace_snapshot(
    instance: &mut dyn PluginInstance,
    sample_rate: f64,
    events: &[ProcessEvent],
    label: String,
) -> HostResult<SweepTraceSnapshot> {
    let layout = infer_multibus_layout(instance);
    let in_ch = layout.main_in_channels;
    let total_out_ch = layout.total_out_channels.max(1);
    let is_inst = is_instrument_plugin(instance, in_ch);
    let inst_key = default_instrument_midi_key(instance);
    let mut inst_events = events.to_vec();
    if is_inst
        && !inst_events
            .iter()
            .any(|e| matches!(e, ProcessEvent::NoteOn { .. }))
    {
        inst_events.push(ProcessEvent::NoteOn {
            sample_offset: 0,
            port_index: 0,
            channel: 0,
            key: inst_key,
            note_id: 1,
            velocity: 0.85,
        });
    }

    // 1. Small-Signal Impulse -> Linear Bode Response (with parameter-smoothing warm-up and PDC latency compensation)
    let n_warmup = 8192usize;
    let n_ir = 16384usize;
    let silence = vec![vec![0.0f32; n_warmup]; in_ch];
    let impulse = generate_signal(
        &SignalSpec::Impulse {
            offset_samples: 0,
            amplitude: if in_ch == 0 { 1.0 } else { 1e-3 },
        },
        sample_rate,
        n_ir,
    );
    let mut flush_out = Vec::new();
    instance.activate(sample_rate, 1, 512)?;
    instance.start_processing()?;
    instance.reset();
    let lat = (instance.latency_samples().unwrap_or(0) as usize).min(65536);
    let total_ir = n_ir + lat;
    let mut ir_padded = impulse.clone();
    ir_padded.resize(total_ir, 0.0);
    let ir_in = vec![ir_padded; in_ch];
    let _ = instance.flush_params(events, &mut flush_out);
    let (warm_raw, _, _) =
        process_audio_stream(instance, &silence, total_out_ch, n_warmup, 256, events)?;
    let warm_out = mix_down_output_buses(&warm_raw, &layout, is_inst);
    let dc_baseline = warm_out
        .first()
        .and_then(|ch| ch.last().copied())
        .unwrap_or(0.0);
    let ir_trigger_events: &[ProcessEvent] = if in_ch == 0 { &inst_events } else { &[] };
    let (ir_raw, _, _) = process_audio_stream(
        instance,
        &ir_in,
        total_out_ch,
        total_ir,
        256,
        ir_trigger_events,
    )?;
    instance.stop_processing();
    instance.deactivate();
    let ir_out = mix_down_output_buses(&ir_raw, &layout, is_inst);
    let ir_slice = &ir_out[0][lat..lat + n_ir];
    let ir_clean: Vec<f32> = ir_slice.iter().map(|&s| s - dc_baseline).collect();
    let mut bode = compute_bode_response(&impulse, &ir_clean, sample_rate);

    // Fallback to -12 dBFS Log-Sine Sweep when a 1-sample Dirac impulse is rejected
    // by a de-clicker (RX De-click), noise gate (kHs Gate), or zero-phase ring modulator (MRingModulator)
    if in_ch > 0 && !is_inst && bode.max_gain_db < -60.0 {
        let sweep = generate_signal(
            &SignalSpec::LogSweep {
                start_hz: 20.0,
                end_hz: 20000.0,
                amplitude: 0.25,
            },
            sample_rate,
            n_ir,
        );
        let mut sw_padded = sweep.clone();
        sw_padded.resize(total_ir, 0.0);
        // Feed both main and sidechain inputs so gates/duckers open during the sweep
        let sw_in = vec![sw_padded; layout.total_in_channels.max(in_ch)];
        instance.activate(sample_rate, 1, 512)?;
        instance.start_processing()?;
        instance.reset();
        let _ = instance.flush_params(events, &mut flush_out);
        let _ = process_audio_stream(instance, &silence, total_out_ch, n_warmup, 256, events)?;
        let (sw_raw, _, _) =
            process_audio_stream(instance, &sw_in, total_out_ch, total_ir, 256, &[])?;
        instance.stop_processing();
        instance.deactivate();
        let sw_out = mix_down_output_buses(&sw_raw, &layout, is_inst);
        let sw_slice = &sw_out[0][lat..lat + n_ir];
        let sw_clean: Vec<f32> = sw_slice.iter().map(|&s| s - dc_baseline).collect();
        let sw_bode = compute_bode_response(&sweep, &sw_clean, sample_rate);
        if sw_bode.max_gain_db > bode.max_gain_db {
            bode = sw_bode;
        }
    }

    // 2. 1 kHz Sine (or 440 Hz A4 MIDI NoteOn for Instruments) -> THD+N & Spectrum
    let n_sine = (sample_rate.round() as usize).max(16384) + lat;
    let probe_freq = if is_inst { 440.0 } else { 1000.0 };
    let sine = generate_signal(
        &SignalSpec::Sine {
            freq_hz: probe_freq,
            amplitude: 0.5,
            phase_rad: 0.0,
        },
        sample_rate,
        n_sine,
    );
    let sine_in = vec![sine; in_ch];
    instance.activate(sample_rate, 1, 512)?;
    instance.start_processing()?;
    instance.reset();
    let _ = instance.flush_params(events, &mut flush_out);
    let (sine_raw, _, _) =
        process_audio_stream(instance, &sine_in, total_out_ch, n_sine, 256, &inst_events)?;
    instance.stop_processing();
    instance.deactivate();
    let sine_out = mix_down_output_buses(&sine_raw, &layout, is_inst);
    let steady_start = if is_inst && inst_key == 36 {
        lat.min(sine_out[0].len().saturating_sub(4096))
    } else {
        (lat + 4096).min(sine_out[0].len().saturating_sub(4096))
    };
    let steady = &sine_out[0][steady_start..];
    let thdn = analyze_thdn(steady, sample_rate, probe_freq);

    if bode.gain_1khz_db < -60.0 && thdn.fundamental_level_dbfs > -60.0 {
        bode.gain_1khz_db = if is_inst {
            thdn.fundamental_level_dbfs
        } else {
            thdn.fundamental_level_dbfs - linear_to_db(0.5)
        };
    }

    // 3. Dynamics Curve (-48 .. +24 dBFS)
    let dynamics = analyze_dynamics_envelope(sample_rate, |mono_stim| {
        let frames = mono_stim.len();
        let total_frames = frames + lat;
        let mut padded_stim = mono_stim.to_vec();
        padded_stim.resize(total_frames, 0.0);
        let in_bufs = vec![padded_stim; in_ch];
        let mut fl = Vec::new();
        let _ = instance.activate(sample_rate, 1, 512);
        let _ = instance.start_processing();
        instance.reset();
        let _ = instance.flush_params(events, &mut fl);
        let _ = process_audio_stream(instance, &silence, total_out_ch, n_warmup, 256, events);
        let res = process_audio_stream(instance, &in_bufs, total_out_ch, total_frames, 256, &[]);
        instance.stop_processing();
        instance.deactivate();
        res.map(|(chs, _, _)| {
            let mixed = mix_down_output_buses(&chs, &layout, is_inst);
            mixed[0][lat..lat + frames].to_vec()
        })
        .unwrap_or_else(|_| vec![0.0; frames])
    });

    Ok(SweepTraceSnapshot {
        label,
        bode,
        thdn,
        dynamics,
    })
}

fn measure_channel_peak_and_rms_dbfs(samples: &[f32]) -> (f64, f64) {
    if samples.is_empty() {
        return (-140.0, -140.0);
    }
    let mut peak = 0.0f64;
    let mut sum_sq = 0.0f64;
    for &s in samples {
        let v = (s as f64).abs();
        if v > peak {
            peak = v;
        }
        sum_sq += v * v;
    }
    let rms = (sum_sq / (samples.len() as f64)).sqrt();
    (
        linear_to_db(peak).clamp(-140.0, 60.0),
        linear_to_db(rms).clamp(-140.0, 60.0),
    )
}

fn measure_multibus_and_sidechain(
    instance: &mut dyn PluginInstance,
    sample_rate: f64,
    audio_ports: &[AudioPortInfo],
    param_infos: &[ParamInfo],
    base_events: &[ProcessEvent],
    default_reset_events: &[ProcessEvent],
) -> HostResult<MultiBusRoutingReport> {
    let layout = infer_multibus_layout(instance);
    let in_ch = layout.main_in_channels;
    let total_in_ch = layout.total_in_channels;
    let total_out_ch = layout.total_out_channels.max(1);
    let is_inst = is_instrument_plugin(instance, in_ch);
    let inst_key = default_instrument_midi_key(instance);

    let mut input_buses = Vec::new();
    let mut output_buses = Vec::new();
    for p in audio_ports {
        let entry = BusTelemetryEntry {
            index: p.index,
            id: p.id,
            name: p.name.clone(),
            is_input: p.is_input,
            is_main: p.is_main,
            channel_count: p.channel_count,
            port_type: p.port_type.clone(),
            measured_peak_dbfs: None,
            measured_rms_dbfs: None,
            active: p.is_main,
        };
        if p.is_input {
            input_buses.push(entry);
        } else {
            output_buses.push(entry);
        }
    }

    // 1. Probe all output buses simultaneously during excitation to measure per-bus stem activity
    let n_probe = 8192usize;
    let mut probe_events = base_events.to_vec();
    if is_inst
        && !probe_events
            .iter()
            .any(|e| matches!(e, ProcessEvent::NoteOn { .. }))
    {
        // Trigger both default key and GM kit pieces (Kick=36, Snare=38, HiHat=42, Tom=45, C4=60, A4=69)
        // when the instrument has multiple output buses so multi-out drum kits light up their stems
        let keys: &[i16] = if layout.out_bus_counts.len() > 1 {
            &[36, 38, 42, 45, 48, 60, 69]
        } else {
            std::slice::from_ref(&inst_key)
        };
        for (idx, &k) in keys.iter().enumerate() {
            probe_events.push(ProcessEvent::NoteOn {
                sample_offset: 0,
                port_index: 0,
                channel: 0,
                key: k,
                note_id: (idx + 1) as i32,
                velocity: 0.90,
            });
        }
    }
    let probe_sig = generate_signal(
        &SignalSpec::Sine {
            freq_hz: 1000.0,
            amplitude: 0.5,
            phase_rad: 0.0,
        },
        sample_rate,
        n_probe,
    );
    let probe_in = vec![probe_sig.clone(); in_ch];
    let mut flush_out = Vec::new();
    instance.activate(sample_rate, 1, 512)?;
    instance.start_processing()?;
    instance.reset();
    let _ = instance.flush_params(default_reset_events, &mut flush_out);
    if !base_events.is_empty() {
        let _ = instance.flush_params(base_events, &mut flush_out);
    }
    let (probe_out, _, _) = process_audio_stream(
        instance,
        &probe_in,
        total_out_ch,
        n_probe,
        256,
        &probe_events,
    )?;
    instance.stop_processing();
    instance.deactivate();

    for (bus_idx, entry) in input_buses.iter_mut().enumerate() {
        if in_ch > 0 && bus_idx == 0 {
            let (pk, rm) = measure_channel_peak_and_rms_dbfs(&probe_sig);
            entry.measured_peak_dbfs = Some(pk);
            entry.measured_rms_dbfs = Some(rm);
            entry.active = true;
        } else {
            entry.active = layout.aux_in_channels > 0;
        }
    }

    let mut active_output_stems = 0usize;
    let mut out_offset = 0usize;
    for (bus_idx, entry) in output_buses.iter_mut().enumerate() {
        let bus_ch = layout
            .out_bus_counts
            .get(bus_idx)
            .copied()
            .unwrap_or(entry.channel_count as usize)
            .max(1);
        let mut bus_peak = -140.0f64;
        let mut bus_rms = -140.0f64;
        for ch in 0..bus_ch {
            if let Some(samples) = probe_out.get(out_offset + ch) {
                let (pk, rm) = measure_channel_peak_and_rms_dbfs(samples);
                bus_peak = bus_peak.max(pk);
                bus_rms = bus_rms.max(rm);
            }
        }
        entry.measured_peak_dbfs = Some(bus_peak);
        entry.measured_rms_dbfs = Some(bus_rms);
        entry.active = bus_peak > -90.0;
        if entry.active {
            active_output_stems += 1;
        }
        out_offset += bus_ch;
    }

    // 2. Sidechain Input Excitation & Ducking / Keying Characterization
    let has_sidechain_input = layout.aux_in_channels > 0;
    let has_aux_outputs = layout.out_bus_counts.len() > 1;
    let mut sidechain_ducking_db = None;
    let mut sidechain_key_param_name = None;
    let mut sidechain_dynamics = None;

    if has_sidechain_input && in_ch > 0 {
        let main_carrier = generate_signal(
            &SignalSpec::Sine {
                freq_hz: 1000.0,
                amplitude: 0.125, // -18.06 dBFS steady carrier on Main Input
                phase_rad: 0.0,
            },
            sample_rate,
            n_probe,
        );
        let sc_key_burst = generate_signal(
            &SignalSpec::Sine {
                freq_hz: 250.0,
                amplitude: 2.0, // +6.02 dBFS strong key signal on Sidechain Input (crosses 0 dBFS thresholds)
                phase_rad: 0.0,
            },
            sample_rate,
            n_probe,
        );
        let sc_silence = vec![0.0f32; n_probe];

        let run_sc_probe = |inst: &mut dyn PluginInstance,
                            sc_signal: &[f32],
                            extra_events: &[ProcessEvent]|
         -> f64 {
            let mut in_bufs = Vec::with_capacity(total_in_ch);
            for _ in 0..in_ch {
                in_bufs.push(main_carrier.clone());
            }
            for _ in in_ch..total_in_ch {
                in_bufs.push(sc_signal.to_vec());
            }
            let mut fl = Vec::new();
            let _ = inst.activate(sample_rate, 1, 512);
            let _ = inst.start_processing();
            inst.reset();
            let _ = inst.flush_params(default_reset_events, &mut fl);
            if !extra_events.is_empty() {
                let _ = inst.flush_params(extra_events, &mut fl);
            }
            let res = process_audio_stream(
                inst,
                &in_bufs,
                layout.main_out_channels.max(1),
                n_probe,
                256,
                extra_events,
            );
            inst.stop_processing();
            inst.deactivate();
            if let Ok((chs, _, _)) = res {
                let tail = if chs[0].len() > 2048 {
                    &chs[0][2048..]
                } else {
                    &chs[0][..]
                };
                measure_channel_peak_and_rms_dbfs(tail).1
            } else {
                -140.0
            }
        };

        let rms_unkeyed = run_sc_probe(instance, &sc_silence, base_events);
        let rms_keyed = run_sc_probe(instance, &sc_key_burst, base_events);
        let mut best_delta = rms_keyed - rms_unkeyed;
        let mut active_sc_events = base_events.to_vec();

        // If default parameters don't route the external sidechain bus, check for an explicit Sidechain enable parameter
        if best_delta.abs() < 0.20 {
            let sc_candidates: Vec<&ParamInfo> = param_infos
                .iter()
                .filter(|p| {
                    if p.flags.is_readonly || p.flags.is_hidden {
                        return false;
                    }
                    let nl = p.name.to_ascii_lowercase();
                    nl.contains("sidechain")
                        || nl.contains("side chain")
                        || nl.contains("ext sc")
                        || nl.contains("external")
                        || nl.contains("key source")
                        || nl.contains("detector")
                })
                .take(4)
                .collect();

            for sc_param in sc_candidates {
                let mut cand_events = base_events.to_vec();
                cand_events.push(ProcessEvent::ParamValue {
                    sample_offset: 0,
                    param_id: sc_param.id,
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: sc_param.max_value,
                });
                let rms_u2 = run_sc_probe(instance, &sc_silence, &cand_events);
                let rms_k2 = run_sc_probe(instance, &sc_key_burst, &cand_events);
                let delta2 = rms_k2 - rms_u2;
                if delta2.abs() > best_delta.abs() {
                    best_delta = delta2;
                    sidechain_key_param_name = Some(sc_param.name.clone());
                    active_sc_events = cand_events;
                    if best_delta.abs() >= 0.20 {
                        break;
                    }
                }
            }
        }

        for entry in input_buses.iter_mut().skip(1) {
            entry.measured_peak_dbfs = Some(6.02);
            entry.measured_rms_dbfs = Some(3.01);
            entry.active = true;
        }
        sidechain_ducking_db = Some(best_delta);

        // Capture a full Sidechain-Coupled Dynamics Transfer Curve when sidechain keying is active
        if best_delta.abs() >= 0.20 {
            let lat = (instance.latency_samples().unwrap_or(0) as usize).min(65536);
            let sc_dyn = analyze_dynamics_envelope(sample_rate, |mono_stim| {
                let frames = mono_stim.len();
                let total_frames = frames + lat;
                let mut padded_stim = mono_stim.to_vec();
                padded_stim.resize(total_frames, 0.0);
                // Excite both Main Input and Sidechain Input with the level sweep stimulus
                let in_bufs = vec![padded_stim; total_in_ch];
                let mut fl = Vec::new();
                let _ = instance.activate(sample_rate, 1, 512);
                let _ = instance.start_processing();
                instance.reset();
                let _ = instance.flush_params(default_reset_events, &mut fl);
                let _ = instance.flush_params(&active_sc_events, &mut fl);
                let res = process_audio_stream(
                    instance,
                    &in_bufs,
                    layout.main_out_channels.max(1),
                    total_frames,
                    256,
                    &active_sc_events,
                );
                instance.stop_processing();
                instance.deactivate();
                res.map(|(chs, _, _)| chs[0][lat..lat + frames].to_vec())
                    .unwrap_or_else(|_| vec![0.0; frames])
            });
            sidechain_dynamics = Some(sc_dyn);
        }

        // Restore default parameters after sidechain probe
        let _ = instance.flush_params(default_reset_events, &mut flush_out);
        if !base_events.is_empty() {
            let _ = instance.flush_params(base_events, &mut flush_out);
        }
    }

    Ok(MultiBusRoutingReport {
        input_buses,
        output_buses,
        has_sidechain_input,
        has_aux_outputs,
        active_output_stems,
        sidechain_ducking_db,
        sidechain_key_param_name,
        sidechain_dynamics,
    })
}

fn measure_instrument_sweep_matrix(
    instance: &mut dyn PluginInstance,
    sample_rate: f64,
    base_events: &[ProcessEvent],
    default_reset_events: &[ProcessEvent],
) -> HostResult<InstrumentSweepReport> {
    let layout = infer_multibus_layout(instance);
    let in_ch = layout.main_in_channels;
    let total_out_ch = layout.total_out_channels.max(1);
    let is_percussive = default_instrument_midi_key(instance) == 36;

    let register_specs: &[(&str, i16)] =
        &[("C2", 36), ("C3", 48), ("C4", 60), ("A4", 69), ("C6", 84)];
    let velocities: &[f64] = &[0.25, 0.65, 1.00];

    let mut flush_out = Vec::new();
    let lat = (instance.latency_samples().unwrap_or(0) as usize).min(32768);
    let n_note = 16384usize + lat;
    let silence_in = vec![vec![0.0f32; n_note]; in_ch];

    let mut registers = Vec::with_capacity(register_specs.len());
    let mut pitch_err_sum = 0.0f64;
    let mut pitch_err_count = 0usize;
    let mut max_vel_dr_db = 0.0f64;
    let mut tilt_sum = 0.0f64;
    let mut tilt_count = 0usize;

    for &(note_name, midi_key) in register_specs {
        let nominal_freq_hz = 440.0 * 2.0f64.powf(((midi_key as f64) - 69.0) / 12.0);
        let mut velocity_points = Vec::with_capacity(velocities.len());
        let mut spectral_envelope = Vec::new();
        let mut measured_f0 = nominal_freq_hz;

        for &vel in velocities {
            instance.activate(sample_rate, 1, 512)?;
            instance.start_processing()?;
            instance.reset();
            let _ = instance.flush_params(default_reset_events, &mut flush_out);
            if !base_events.is_empty() {
                let _ = instance.flush_params(base_events, &mut flush_out);
            }
            let mut evs = base_events.to_vec();
            evs.push(ProcessEvent::NoteOn {
                sample_offset: 0,
                port_index: 0,
                channel: 0,
                key: midi_key,
                note_id: (midi_key as i32) * 10 + ((vel * 100.0) as i32),
                velocity: vel,
            });
            let (raw_out, _, _) =
                process_audio_stream(instance, &silence_in, total_out_ch, n_note, 256, &evs)?;
            instance.stop_processing();
            instance.deactivate();
            let mixed = mix_down_output_buses(&raw_out, &layout, true);
            let start = if is_percussive {
                lat.min(mixed[0].len().saturating_sub(4096))
            } else {
                (lat + 1024).min(mixed[0].len().saturating_sub(4096))
            };
            let slice = &mixed[0][start..];
            let (peak_dbfs, rms_dbfs) = measure_channel_peak_and_rms_dbfs(slice);
            let spec = compute_spectrum(slice, sample_rate, WindowType::BlackmanHarris);
            let thdn = analyze_thdn(slice, sample_rate, nominal_freq_hz);

            // Identify fundamental frequency (preferring a strong peak near nominal_freq_hz if upper harmonics dominate)
            let candidate_f0 = spec
                .top_peaks
                .iter()
                .filter(|p| p.magnitude_db >= spec.peak_magnitude_db - 24.0)
                .find(|p| {
                    let cents = (1200.0 * (p.freq_hz.max(1.0) / nominal_freq_hz).log2()).abs();
                    cents <= 150.0
                })
                .map(|p| p.freq_hz)
                .unwrap_or(spec.peak_freq_hz);

            if (vel - 1.00).abs() < 1e-6 {
                measured_f0 = candidate_f0;
                spectral_envelope = compute_third_octave_envelope(slice, sample_rate);
            }

            velocity_points.push(InstrumentVelocityPoint {
                velocity: vel,
                peak_dbfs,
                rms_dbfs,
                fundamental_hz: candidate_f0,
                spectral_centroid_hz: spec.spectral_centroid_hz,
                thd_percent: thdn.thd_percent,
            });
        }

        let raw_cents = if measured_f0 > 10.0 {
            1200.0 * (measured_f0 / nominal_freq_hz).log2()
        } else {
            0.0
        };
        let ff_active = velocity_points
            .last()
            .map(|p| p.peak_dbfs > -80.0)
            .unwrap_or(false);
        let pitch_tracked = !is_percussive && ff_active && raw_cents.abs() <= 200.0;
        let pitch_error_cents = if ff_active {
            raw_cents.clamp(-2400.0, 2400.0)
        } else {
            0.0
        };
        if pitch_tracked {
            pitch_err_sum += pitch_error_cents.abs();
            pitch_err_count += 1;
        }

        let pp_rms = velocity_points
            .first()
            .map(|p| p.rms_dbfs)
            .unwrap_or(-140.0);
        let ff_rms = velocity_points.last().map(|p| p.rms_dbfs).unwrap_or(-140.0);
        let velocity_dynamic_range_db = if pp_rms > -120.0 && ff_rms > -120.0 {
            (ff_rms - pp_rms).max(0.0)
        } else {
            0.0
        };
        max_vel_dr_db = max_vel_dr_db.max(velocity_dynamic_range_db);

        let pp_cent = velocity_points
            .first()
            .map(|p| p.spectral_centroid_hz)
            .unwrap_or(0.0);
        let ff_cent = velocity_points
            .last()
            .map(|p| p.spectral_centroid_hz)
            .unwrap_or(0.0);
        let brightness_tilt_ratio = if pp_cent > 20.0 && ff_cent > 20.0 {
            ff_cent / pp_cent
        } else {
            1.0
        };
        if ff_active {
            tilt_sum += brightness_tilt_ratio;
            tilt_count += 1;
        }

        registers.push(InstrumentRegisterRow {
            note_name: note_name.to_string(),
            midi_key,
            nominal_freq_hz,
            measured_fundamental_hz: measured_f0,
            pitch_error_cents,
            pitch_tracked,
            velocity_points,
            velocity_dynamic_range_db,
            brightness_tilt_ratio,
            spectral_envelope,
        });
    }

    Ok(InstrumentSweepReport {
        is_percussive,
        registers,
        mean_abs_pitch_error_cents: if pitch_err_count > 0 {
            pitch_err_sum / (pitch_err_count as f64)
        } else {
            0.0
        },
        max_velocity_dynamic_range_db: max_vel_dr_db,
        mean_brightness_tilt_ratio: if tilt_count > 0 {
            tilt_sum / (tilt_count as f64)
        } else {
            1.0
        },
    })
}

/// Generate a complete, print-ready white HTML + vector SVG engineering datasheet for `instance`
/// (with optional `sweep_param` multi-curve family and optional `reference_instance` A/B overlay).
pub fn generate_plugin_datasheet(
    instance: &mut dyn PluginInstance,
    reference_instance: Option<&mut dyn PluginInstance>,
    output_html: &Path,
    sample_rate: f64,
    sweep_param: Option<u32>,
    base_events: &[ProcessEvent],
) -> HostResult<DatasheetReport> {
    // Snapshot initial factory state and parameter values before validation mutates anything
    let initial_state = if instance.supports_state() {
        instance.save_state().ok()
    } else {
        None
    };
    let param_infos = instance.params();
    let default_reset_events: Vec<ProcessEvent> = param_infos
        .iter()
        .filter(|p| {
            if p.flags.is_readonly || p.flags.is_hidden || (p.flags.raw_bits & (1 << 15)) != 0 {
                return false;
            }
            let name_lc = p.name.to_ascii_lowercase();
            !name_lc.contains("preset")
                && !name_lc.contains("program")
                && !name_lc.contains("midi cc")
                && !name_lc.contains("trigger")
        })
        .take(32)
        .map(|p| {
            let init_val = instance.get_param_value(p.id).unwrap_or(p.default_value);
            ProcessEvent::ParamValue {
                sample_offset: 0,
                param_id: p.id,
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value: init_val,
            }
        })
        .collect();

    // Activate in pristine state first to capture metadata, active parameter table, and offscreen GUI screenshot
    let mut flush_out = Vec::new();
    let _ = instance.activate(sample_rate, 1, 1024);
    let _ = instance.start_processing();
    instance.reset();
    if !base_events.is_empty() {
        let _ = instance.flush_params(base_events, &mut flush_out);
    }
    let descriptor = instance.descriptor().clone();
    let audio_ports = instance.audio_ports();
    let note_ports = instance.note_ports();
    let mut param_rows = Vec::new();
    let mut queried_non_cc = 0usize;
    for p in &param_infos {
        let is_non_cc = !p.flags.is_hidden
            && !p.name.trim().is_empty()
            && !p.name.trim().to_ascii_lowercase().starts_with("midi cc");
        let query_live = is_non_cc && queried_non_cc < 32;
        if query_live {
            queried_non_cc += 1;
        }
        let cur = if query_live {
            instance.get_param_value(p.id).unwrap_or(p.default_value)
        } else {
            p.default_value
        };
        let txt = if query_live {
            instance
                .param_value_to_text(p.id, cur)
                .unwrap_or_else(|| format!("{cur:.3}"))
        } else {
            format!("{cur:.3}")
        };
        param_rows.push((p.clone(), cur, txt));
    }
    let latency_samples = instance.latency_samples();
    let tail_samples = instance.tail_samples();
    instance.stop_processing();
    instance.deactivate();

    let layout = infer_multibus_layout(instance);
    let in_ch = layout.main_in_channels;
    let total_out_ch = layout.total_out_channels.max(1);
    let is_inst = is_instrument_plugin(instance, in_ch);
    let inst_key = default_instrument_midi_key(instance);

    // Primary trace snapshot (measured on pristine factory instance before validation/GUI)
    let primary =
        measure_trace_snapshot(instance, sample_rate, base_events, descriptor.name.clone())?;

    // Multi-Bus Audio Routing Inspection & Sidechain Excitation
    let multibus = measure_multibus_and_sidechain(
        instance,
        sample_rate,
        &audio_ports,
        &param_infos,
        base_events,
        &default_reset_events,
    )?;

    // Multi-Velocity & Multi-Register Note Sweep for Instrument Datasheets
    let instrument_sweep = if is_inst {
        measure_instrument_sweep_matrix(instance, sample_rate, base_events, &default_reset_events)
            .ok()
    } else {
        None
    };

    // Two-Tone IMD, RT60, Delay & Stereo
    let n_sec = (sample_rate.round() as usize).max(16384);
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
        n_sec,
    );
    let tt_in = vec![two_tone; in_ch];
    let mut tt_events = base_events.to_vec();
    if is_inst
        && !tt_events
            .iter()
            .any(|e| matches!(e, ProcessEvent::NoteOn { .. }))
    {
        tt_events.push(ProcessEvent::NoteOn {
            sample_offset: 0,
            port_index: 0,
            channel: 0,
            key: inst_key,
            note_id: 1,
            velocity: 0.85,
        });
    }
    instance.activate(sample_rate, 1, 512)?;
    instance.start_processing()?;
    instance.reset();
    let (tt_raw, _, _) =
        process_audio_stream(instance, &tt_in, total_out_ch, n_sec, 256, &tt_events)?;
    instance.stop_processing();
    instance.deactivate();
    let tt_out = mix_down_output_buses(&tt_raw, &layout, is_inst);
    let tt_steady = if tt_out[0].len() > 8192 {
        &tt_out[0][4096..]
    } else {
        &tt_out[0][..]
    };
    let primary_imd = analyze_imd(tt_steady, sample_rate, f1, f2);
    let right_steady = if tt_out.len() >= 2 && tt_out[1].len() > 8192 {
        &tt_out[1][4096..]
    } else {
        tt_steady
    };
    let primary_stereo = analyze_stereo_image(tt_steady, right_steady);

    let n_tail = ((2.0 * sample_rate).round() as usize).max(32768);
    let impulse = generate_signal(
        &SignalSpec::Impulse {
            offset_samples: 0,
            amplitude: 0.8,
        },
        sample_rate,
        n_tail,
    );
    let imp_in = vec![impulse.clone(); in_ch];
    let mut imp_events = base_events.to_vec();
    if is_inst
        && !imp_events
            .iter()
            .any(|e| matches!(e, ProcessEvent::NoteOn { .. }))
    {
        let gate_len = ((0.05 * sample_rate).round() as u32).max(512);
        imp_events.push(ProcessEvent::NoteOn {
            sample_offset: 0,
            port_index: 0,
            channel: 0,
            key: inst_key,
            note_id: 1,
            velocity: 0.85,
        });
        imp_events.push(ProcessEvent::NoteOff {
            sample_offset: gate_len,
            port_index: 0,
            channel: 0,
            key: inst_key,
            note_id: 1,
            velocity: 0.0,
        });
    }
    instance.activate(sample_rate, 1, 512)?;
    instance.start_processing()?;
    instance.reset();
    let (imp_raw, _, _) =
        process_audio_stream(instance, &imp_in, total_out_ch, n_tail, 256, &imp_events)?;
    instance.stop_processing();
    instance.deactivate();
    let imp_out = mix_down_output_buses(&imp_raw, &layout, is_inst);
    let primary_rt60 = analyze_rt60(&imp_out[0], sample_rate);
    let primary_delay =
        analyze_delay_and_latency(&impulse, &imp_out[0], sample_rate, latency_samples);

    // Optional Parameter-Family Sweep (--sweep-param <ID>)
    let mut sweep_traces = Vec::new();
    if let Some(pid) = sweep_param {
        if let Some(pinfo) = param_infos.iter().find(|p| p.id == pid) {
            let use_log = pinfo.min_value >= 5.0 && (pinfo.max_value / pinfo.min_value) >= 50.0;
            let name_lc = pinfo.name.to_ascii_lowercase();
            let is_fullband_freq = !pinfo.flags.is_stepped
                && (name_lc.contains("cutoff") || name_lc.contains("freq"))
                && instance
                    .param_value_to_text(pid, pinfo.max_value)
                    .map(|t| t.contains("kHz") || t.contains("20000"))
                    .unwrap_or(false);
            for step_idx in 0..5 {
                let raw_frac = (step_idx as f64) / 4.0;
                let frac = if is_fullband_freq {
                    0.15 + raw_frac * 0.70
                } else {
                    raw_frac
                };
                let val = if use_log {
                    pinfo.min_value * (pinfo.max_value / pinfo.min_value).powf(frac)
                } else {
                    pinfo.min_value + frac * (pinfo.max_value - pinfo.min_value)
                };
                let val_txt = instance
                    .param_value_to_text(pid, val)
                    .unwrap_or_else(|| format!("{val:.2}"));
                let mut evs = default_reset_events.clone();
                evs.extend_from_slice(base_events);
                evs.push(ProcessEvent::ParamValue {
                    sample_offset: 0,
                    param_id: pid,
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: val,
                });
                let lbl = format!("{} = {}", pinfo.name, val_txt);
                if let Ok(snap) = measure_trace_snapshot(instance, sample_rate, &evs, lbl) {
                    sweep_traces.push(snap);
                }
            }
        }
    }

    // Optional Reference Plugin A/B Comparison (--reference <PATH>)
    let ref_snapshot = if let Some(ref_inst) = reference_instance {
        let ref_lbl = format!("Ref: {}", ref_inst.descriptor().name);
        measure_trace_snapshot(ref_inst, sample_rate, base_events, ref_lbl).ok()
    } else {
        None
    };

    // Run 9-category validation & DSP benchmarks BEFORE spinning up any GUI threads
    let validation = run_full_validation(instance, sample_rate);
    if let Some(ref bytes) = initial_state {
        let _ = instance.load_state(bytes);
    }
    let _ = instance.flush_params(&default_reset_events, &mut flush_out);
    if !base_events.is_empty() {
        let _ = instance.flush_params(base_events, &mut flush_out);
    }

    let benchmark = run_benchmarks(
        instance,
        &[sample_rate],
        &[64, 128, 256, 512, 1024],
        60,
        false,
    )?;

    // Restore pristine initial state & capture offscreen GUI screenshot last
    if let Some(ref bytes) = initial_state {
        let _ = instance.load_state(bytes);
    }
    let _ = instance.activate(sample_rate, 1, 1024);
    let _ = instance.start_processing();
    instance.reset();
    let _ = instance.flush_params(&default_reset_events, &mut flush_out);
    if !base_events.is_empty() {
        let _ = instance.flush_params(base_events, &mut flush_out);
    }
    let mut gui_data_uri: Option<(u32, u32, String)> = None;
    let tmp_png = std::env::temp_dir().join(format!("hph_datasheet_{}.png", std::process::id()));
    if let Ok(shot) = capture_plugin_screenshot(instance, &tmp_png, 250, None) {
        if let Ok(png_bytes) = std::fs::read(&tmp_png) {
            gui_data_uri = Some((
                shot.width,
                shot.height,
                format!("data:image/png;base64,{}", base64_encode(&png_bytes)),
            ));
        }
        let _ = std::fs::remove_file(&tmp_png);
    }
    instance.stop_processing();
    instance.deactivate();

    let fingerprint = DatasheetFingerprint {
        frequency_shaping_active: (primary.bode.max_gain_db - primary.bode.min_gain_db).abs() > 0.5
            || !sweep_traces.is_empty(),
        nonlinear_harmonics_active: primary.thdn.thd_percent > 0.01
            || primary_imd.imd_percent > 0.01,
        dynamic_compression_active: primary.dynamics.max_gain_reduction_db > 0.5
            || (primary.dynamics.estimated_ratio - 1.0).abs() > 0.15
            || multibus
                .sidechain_ducking_db
                .map(|d| d.abs() >= 0.5)
                .unwrap_or(false),
        spatial_tail_active: primary_rt60.rt60_sec > 0.05 || primary_delay.echo_taps.len() > 1,
    };

    let html = build_printable_html_datasheet(
        &descriptor,
        sample_rate,
        &audio_ports,
        &note_ports,
        &param_rows,
        latency_samples,
        tail_samples,
        gui_data_uri.as_ref(),
        &fingerprint,
        &validation,
        &benchmark,
        &primary,
        &primary_imd,
        &primary_rt60,
        &primary_delay,
        &primary_stereo,
        &multibus,
        instrument_sweep.as_ref(),
        &sweep_traces,
        ref_snapshot.as_ref(),
    );

    if let Some(parent) = output_html.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                HostError::ProcessError(format!(
                    "Failed to create directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
    }
    std::fs::write(output_html, &html).map_err(|e| {
        HostError::ProcessError(format!(
            "Failed to write HTML datasheet to {}: {e}",
            output_html.display()
        ))
    })?;

    Ok(DatasheetReport {
        descriptor,
        sample_rate,
        output_html: output_html.display().to_string(),
        fingerprint,
        latency_samples,
        tail_samples,
        validation,
        benchmark,
        primary_bode: primary.bode,
        primary_thdn: primary.thdn,
        primary_imd,
        primary_rt60,
        primary_delay,
        primary_dynamics: primary.dynamics,
        primary_stereo,
        multibus,
        instrument_sweep,
        sweep_traces,
        reference_name: ref_snapshot.map(|r| r.label),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestFilterSaturationPlugin {
        desc: PluginDescriptorInfo,
        cutoff_hz: f64,
        drive: f64,
        sample_rate: f64,
        z1: [f32; 2],
    }

    impl TestFilterSaturationPlugin {
        fn new() -> Self {
            Self {
                desc: PluginDescriptorInfo {
                    id: "com.hph.test.datasheet-synth".into(),
                    name: "Test Filter Saturation".into(),
                    vendor: "HPH Engineering".into(),
                    version: "1.0.0".into(),
                    url: String::new(),
                    manual_url: String::new(),
                    support_url: String::new(),
                    description: "Synthetic plugin for datasheet verification".into(),
                    features: vec!["audio-effect".into(), "filter".into()],
                    format: PluginFormat::Clap,
                },
                cutoff_hz: 4000.0,
                drive: 0.35,
                sample_rate: 48000.0,
                z1: [0.0; 2],
            }
        }
    }

    impl PluginGui for TestFilterSaturationPlugin {}
    impl PluginContractChecks for TestFilterSaturationPlugin {}

    impl PluginInstance for TestFilterSaturationPlugin {
        fn descriptor(&self) -> &PluginDescriptorInfo {
            &self.desc
        }

        fn activate(
            &mut self,
            sample_rate: f64,
            _min_frames: u32,
            _max_frames: u32,
        ) -> HostResult<()> {
            self.sample_rate = sample_rate;
            self.z1 = [0.0; 2];
            Ok(())
        }

        fn deactivate(&mut self) {}
        fn start_processing(&mut self) -> HostResult<()> {
            Ok(())
        }
        fn stop_processing(&mut self) {}
        fn reset(&mut self) {
            self.z1 = [0.0; 2];
        }

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
                        101 => self.cutoff_hz = value.clamp(100.0, 20000.0),
                        102 => self.drive = value.clamp(0.0, 1.0),
                        _ => {}
                    }
                }
            }
            let rc = 1.0 / (2.0 * std::f64::consts::PI * self.cutoff_hz);
            let dt = 1.0 / self.sample_rate;
            let alpha = (dt / (rc + dt)) as f32;
            let pre_gain = 1.0 + (self.drive as f32) * 3.0;
            let frames = buffers.num_frames;

            for (ch, out) in buffers.outputs.iter_mut().enumerate() {
                let inp = buffers
                    .inputs
                    .get(ch)
                    .or_else(|| buffers.inputs.first())
                    .copied();
                let mut z = self.z1[ch.min(1)];
                for i in 0..frames {
                    let x = inp.map(|b| b[i]).unwrap_or(0.0);
                    let sat = (x * pre_gain).tanh();
                    z += alpha * (sat - z);
                    if z.abs() < 1e-20 {
                        z = 0.0;
                    }
                    out[i] = z;
                }
                if ch < 2 {
                    self.z1[ch] = z;
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
                    name: "Cutoff".into(),
                    module: "Filter".into(),
                    min_value: 200.0,
                    max_value: 18000.0,
                    default_value: 4000.0,
                    flags: flags.clone(),
                },
                ParamInfo {
                    id: 102,
                    index: 1,
                    name: "Drive".into(),
                    module: "Saturation".into(),
                    min_value: 0.0,
                    max_value: 1.0,
                    default_value: 0.35,
                    flags,
                },
            ]
        }

        fn get_param_value(&self, param_id: u32) -> Option<f64> {
            match param_id {
                101 => Some(self.cutoff_hz),
                102 => Some(self.drive),
                _ => None,
            }
        }

        fn param_value_to_text(&self, param_id: u32, value: f64) -> Option<String> {
            match param_id {
                101 => Some(format!("{value:.0} Hz")),
                102 => Some(format!("{:.0}%", value * 100.0)),
                _ => Some(format!("{value:.2}")),
            }
        }

        fn param_text_to_value(&self, _param_id: u32, text: &str) -> Option<f64> {
            text.parse().ok()
        }

        fn audio_ports(&self) -> Vec<AudioPortInfo> {
            vec![
                AudioPortInfo {
                    id: 0,
                    index: 0,
                    name: "Stereo In".into(),
                    channel_count: 2,
                    is_main: true,
                    port_type: "stereo".into(),
                    is_input: true,
                    in_place_pair_id: None,
                    prefers_64bit: false,
                },
                AudioPortInfo {
                    id: 1,
                    index: 0,
                    name: "Stereo Out".into(),
                    channel_count: 2,
                    is_main: true,
                    port_type: "stereo".into(),
                    is_input: false,
                    in_place_pair_id: None,
                    prefers_64bit: false,
                },
            ]
        }

        fn note_ports(&self) -> Vec<NotePortInfo> {
            vec![]
        }

        fn latency_samples(&self) -> Option<u32> {
            Some(0)
        }

        fn tail_samples(&self) -> Option<u32> {
            Some(0)
        }

        fn save_state(&self) -> HostResult<Vec<u8>> {
            let mut b = Vec::with_capacity(16);
            b.extend_from_slice(&self.cutoff_hz.to_le_bytes());
            b.extend_from_slice(&self.drive.to_le_bytes());
            Ok(b)
        }

        fn load_state(&mut self, data: &[u8]) -> HostResult<()> {
            if data.len() == 16 {
                let mut c = [0u8; 8];
                let mut d = [0u8; 8];
                c.copy_from_slice(&data[0..8]);
                d.copy_from_slice(&data[8..16]);
                self.cutoff_hz = f64::from_le_bytes(c).clamp(100.0, 20000.0);
                self.drive = f64::from_le_bytes(d).clamp(0.0, 1.0);
            }
            Ok(())
        }
    }

    #[test]
    fn test_generate_printable_html_datasheet_with_sweep_and_reference() {
        let out_path = std::env::temp_dir().join("hph_unit_test_datasheet.html");
        let mut dut = TestFilterSaturationPlugin::new();
        let mut ref_plug = TestFilterSaturationPlugin::new();
        let base_events = vec![ProcessEvent::ParamValue {
            sample_offset: 0,
            param_id: 102,
            note_id: -1,
            port_index: 0,
            channel: 0,
            key: -1,
            value: 0.5,
        }];
        let report = generate_plugin_datasheet(
            &mut dut,
            Some(&mut ref_plug),
            &out_path,
            48000.0,
            Some(101),
            &base_events,
        )
        .expect("datasheet generation should succeed");

        assert_eq!(report.descriptor.name, "Test Filter Saturation");
        assert_eq!(report.primary_bode.points.len(), 31);
        assert_eq!(report.primary_rt60.octave_bands.len(), 9);
        assert_eq!(report.sweep_traces.len(), 5);
        assert!(report.reference_name.is_some());
        assert!(report.fingerprint.frequency_shaping_active);
        assert!(report.fingerprint.nonlinear_harmonics_active);

        let html = std::fs::read_to_string(&out_path).expect("HTML file should exist");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("--bg: #ffffff;"));
        assert!(html.contains("Figure 1A"));
        assert!(html.contains("Figure 2A"));
        assert!(html.contains("Figure 3"));
        assert!(html.contains("Figure 4"));
        assert!(html.contains("Figure 5"));
        let _ = std::fs::remove_file(&out_path);
    }

    struct TestSidechainDuckerMultiOutPlugin {
        desc: PluginDescriptorInfo,
        ext_sc_enabled: f64,
    }

    impl TestSidechainDuckerMultiOutPlugin {
        fn new() -> Self {
            Self {
                desc: PluginDescriptorInfo {
                    id: "com.hph.test.sc-ducker-multiout".into(),
                    name: "Test Sidechain Ducker MultiOut".into(),
                    vendor: "HPH Engineering".into(),
                    version: "1.0.0".into(),
                    url: String::new(),
                    manual_url: String::new(),
                    support_url: String::new(),
                    description: "Synthetic sidechain ducker with aux output stem".into(),
                    features: vec!["audio-effect".into(), "compressor".into()],
                    format: PluginFormat::Clap,
                },
                ext_sc_enabled: 0.0,
            }
        }
    }

    impl PluginGui for TestSidechainDuckerMultiOutPlugin {}
    impl PluginContractChecks for TestSidechainDuckerMultiOutPlugin {}

    impl PluginInstance for TestSidechainDuckerMultiOutPlugin {
        fn descriptor(&self) -> &PluginDescriptorInfo {
            &self.desc
        }

        fn activate(
            &mut self,
            _sample_rate: f64,
            _min_frames: u32,
            _max_frames: u32,
        ) -> HostResult<()> {
            Ok(())
        }

        fn deactivate(&mut self) {}

        fn flush_params(
            &mut self,
            events: &[ProcessEvent],
            _out_events: &mut Vec<ProcessEvent>,
        ) -> HostResult<()> {
            for ev in events {
                if let ProcessEvent::ParamValue {
                    param_id: 201,
                    value,
                    ..
                } = *ev
                {
                    self.ext_sc_enabled = value.clamp(0.0, 1.0);
                }
            }
            Ok(())
        }

        fn process(
            &mut self,
            buffers: &mut AudioProcessBuffers<'_>,
            in_events: &[ProcessEvent],
            _out_events: &mut Vec<ProcessEvent>,
        ) -> HostResult<ProcessStatus> {
            for ev in in_events {
                if let ProcessEvent::ParamValue {
                    param_id: 201,
                    value,
                    ..
                } = *ev
                {
                    self.ext_sc_enabled = value.clamp(0.0, 1.0);
                }
            }
            let frames = buffers.num_frames;
            // Sidechain Bus 1 is on input channels 2..4
            let mut sc_rms_sq = 0.0f32;
            if self.ext_sc_enabled > 0.5 && buffers.inputs.len() >= 3 {
                for &s in &buffers.inputs[2][..frames] {
                    sc_rms_sq += s * s;
                }
                sc_rms_sq /= (frames.max(1)) as f32;
            }
            let sc_env = sc_rms_sq.sqrt();
            // Duck main signal by up to -12 dB (0.25x) when sidechain key is hot
            let gain = 1.0 / (1.0 + 3.0 * sc_env);

            for (ch, out) in buffers.outputs.iter_mut().enumerate() {
                if ch < 2 {
                    // Main Stereo Output (Bus 0)
                    let inp = buffers.inputs.get(ch).copied();
                    for i in 0..frames {
                        out[i] = inp.map(|b| b[i]).unwrap_or(0.0) * gain;
                    }
                } else {
                    // Aux Stem Output (Bus 1: -6 dB monitor copy)
                    let inp = buffers.inputs.get(ch % 2).copied();
                    for i in 0..frames {
                        out[i] = inp.map(|b| b[i]).unwrap_or(0.0) * 0.5;
                    }
                }
            }
            Ok(ProcessStatus::Continue)
        }

        fn params(&self) -> Vec<ParamInfo> {
            vec![ParamInfo {
                id: 201,
                index: 0,
                name: "External Sidechain".into(),
                module: "Detector".into(),
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
                flags: ParamFlags {
                    is_stepped: true,
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
                },
            }]
        }

        fn get_param_value(&self, param_id: u32) -> Option<f64> {
            (param_id == 201).then_some(self.ext_sc_enabled)
        }

        fn audio_ports(&self) -> Vec<AudioPortInfo> {
            vec![
                AudioPortInfo {
                    id: 0,
                    index: 0,
                    name: "Main In".into(),
                    is_input: true,
                    channel_count: 2,
                    is_main: true,
                    port_type: "stereo".into(),
                    in_place_pair_id: None,
                    prefers_64bit: false,
                },
                AudioPortInfo {
                    id: 1,
                    index: 1,
                    name: "Sidechain Key In".into(),
                    is_input: true,
                    channel_count: 2,
                    is_main: false,
                    port_type: "stereo".into(),
                    in_place_pair_id: None,
                    prefers_64bit: false,
                },
                AudioPortInfo {
                    id: 2,
                    index: 0,
                    name: "Main Out".into(),
                    is_input: false,
                    channel_count: 2,
                    is_main: true,
                    port_type: "stereo".into(),
                    in_place_pair_id: None,
                    prefers_64bit: false,
                },
                AudioPortInfo {
                    id: 3,
                    index: 1,
                    name: "Aux Stem Out".into(),
                    is_input: false,
                    channel_count: 2,
                    is_main: false,
                    port_type: "stereo".into(),
                    in_place_pair_id: None,
                    prefers_64bit: false,
                },
            ]
        }
    }

    #[test]
    fn test_multibus_sidechain_and_aux_stem_routing() {
        let out_path = std::env::temp_dir().join("hph_unit_test_multibus_sc.html");
        let mut dut = TestSidechainDuckerMultiOutPlugin::new();
        let report = generate_plugin_datasheet(&mut dut, None, &out_path, 48000.0, None, &[])
            .expect("multibus sidechain datasheet should succeed");

        assert!(report.multibus.has_sidechain_input);
        assert!(report.multibus.has_aux_outputs);
        assert_eq!(report.multibus.input_buses.len(), 2);
        assert_eq!(report.multibus.output_buses.len(), 2);
        assert_eq!(report.multibus.active_output_stems, 2);
        assert_eq!(
            report.multibus.sidechain_key_param_name.as_deref(),
            Some("External Sidechain")
        );
        let duck_db = report
            .multibus
            .sidechain_ducking_db
            .expect("should measure sidechain ducking");
        assert!(
            duck_db < -6.0,
            "Expected sidechain ducking < -6 dB, got {duck_db:.2} dB"
        );
        assert!(report.multibus.sidechain_dynamics.is_some());

        let html = std::fs::read_to_string(&out_path).expect("HTML file should exist");
        assert!(html.contains("SIDECHAIN IN"));
        assert!(html.contains("AUX STEM OUT"));
        assert!(html.contains("Sidechain-Keyed (Aux In)"));
        let _ = std::fs::remove_file(&out_path);
    }

    struct TestPolySynthPlugin {
        desc: PluginDescriptorInfo,
        sample_rate: f64,
        active_freq_hz: f64,
        active_vel: f64,
        phase: f64,
        z1: f32,
    }

    impl TestPolySynthPlugin {
        fn new() -> Self {
            Self {
                desc: PluginDescriptorInfo {
                    id: "com.hph.test.polysynth".into(),
                    name: "Test Velocity PolySynth".into(),
                    vendor: "HPH Engineering".into(),
                    version: "1.0.0".into(),
                    url: String::new(),
                    manual_url: String::new(),
                    support_url: String::new(),
                    description: "Key-tracked saw synth with velocity filter brightness".into(),
                    features: vec!["instrument".into(), "synthesizer".into()],
                    format: PluginFormat::Clap,
                },
                sample_rate: 48000.0,
                active_freq_hz: 0.0,
                active_vel: 0.0,
                phase: 0.0,
                z1: 0.0,
            }
        }
    }

    impl PluginGui for TestPolySynthPlugin {}
    impl PluginContractChecks for TestPolySynthPlugin {}

    impl PluginInstance for TestPolySynthPlugin {
        fn descriptor(&self) -> &PluginDescriptorInfo {
            &self.desc
        }

        fn activate(
            &mut self,
            sample_rate: f64,
            _min_frames: u32,
            _max_frames: u32,
        ) -> HostResult<()> {
            self.sample_rate = sample_rate;
            self.reset();
            Ok(())
        }

        fn deactivate(&mut self) {}

        fn reset(&mut self) {
            self.active_freq_hz = 0.0;
            self.active_vel = 0.0;
            self.phase = 0.0;
            self.z1 = 0.0;
        }

        fn process(
            &mut self,
            buffers: &mut AudioProcessBuffers<'_>,
            in_events: &[ProcessEvent],
            _out_events: &mut Vec<ProcessEvent>,
        ) -> HostResult<ProcessStatus> {
            for ev in in_events {
                match *ev {
                    ProcessEvent::NoteOn { key, velocity, .. } => {
                        self.active_freq_hz = 440.0 * 2.0f64.powf(((key as f64) - 69.0) / 12.0);
                        self.active_vel = velocity.clamp(0.05, 1.0);
                        self.phase = 0.0;
                    }
                    ProcessEvent::NoteOff { .. } => {
                        self.active_vel = 0.0;
                    }
                    _ => {}
                }
            }
            let frames = buffers.num_frames;
            // Velocity controls both amplitude and 1-pole low-pass cutoff (brightness)
            let cutoff_hz = 300.0 + self.active_vel * self.active_vel * 8000.0;
            let rc = 1.0 / (2.0 * std::f64::consts::PI * cutoff_hz);
            let dt = 1.0 / self.sample_rate;
            let alpha = (dt / (rc + dt)) as f32;
            let dt_phase = self.active_freq_hz / self.sample_rate;

            for i in 0..frames {
                let sample = if self.active_vel > 0.0 && self.active_freq_hz > 0.0 {
                    // Rich harmonic wave (fundamental + saw harmonics)
                    let p = self.phase;
                    self.phase = (self.phase + dt_phase).fract();
                    let raw = ((2.0 * std::f64::consts::PI * p).sin() * 0.65
                        + (2.0 * p - 1.0) * 0.35)
                        * self.active_vel;
                    self.z1 += alpha * ((raw as f32) - self.z1);
                    self.z1 * 0.5
                } else {
                    0.0
                };
                for out in buffers.outputs.iter_mut() {
                    out[i] = sample;
                }
            }
            Ok(ProcessStatus::Continue)
        }

        fn audio_ports(&self) -> Vec<AudioPortInfo> {
            vec![AudioPortInfo {
                id: 0,
                index: 0,
                name: "Stereo Out".into(),
                is_input: false,
                channel_count: 2,
                is_main: true,
                port_type: "stereo".into(),
                in_place_pair_id: None,
                prefers_64bit: false,
            }]
        }

        fn note_ports(&self) -> Vec<NotePortInfo> {
            vec![NotePortInfo {
                id: 0,
                index: 0,
                name: "MIDI In".into(),
                is_input: true,
                supported_dialects: 1,
                preferred_dialect: 1,
            }]
        }
    }

    #[test]
    fn test_instrument_multi_velocity_and_register_sweep() {
        let out_path = std::env::temp_dir().join("hph_unit_test_instrument_matrix.html");
        let mut synth = TestPolySynthPlugin::new();
        let report = generate_plugin_datasheet(&mut synth, None, &out_path, 48000.0, None, &[])
            .expect("instrument datasheet should succeed");

        let inst = report
            .instrument_sweep
            .as_ref()
            .expect("instrument_sweep should be populated for synth");
        assert!(!inst.is_percussive);
        assert_eq!(inst.registers.len(), 5);
        assert!(
            inst.mean_abs_pitch_error_cents < 15.0,
            "Expected accurate key tracking < 15 cents, got {:.2} cents",
            inst.mean_abs_pitch_error_cents
        );
        assert!(
            inst.max_velocity_dynamic_range_db > 8.0,
            "Expected velocity dynamic range > 8 dB, got {:.2} dB",
            inst.max_velocity_dynamic_range_db
        );
        assert!(
            inst.mean_brightness_tilt_ratio > 1.15,
            "Expected positive velocity-to-brightness spectral tilt > 1.15x, got {:.2}x",
            inst.mean_brightness_tilt_ratio
        );

        let html = std::fs::read_to_string(&out_path).expect("HTML file should exist");
        assert!(html.contains("Figure 6A"));
        assert!(html.contains("Figure 6B"));
        assert!(html.contains("Instrument Multi-Register &amp; Multi-Velocity Matrix"));
        let _ = std::fs::remove_file(&out_path);
    }
}
