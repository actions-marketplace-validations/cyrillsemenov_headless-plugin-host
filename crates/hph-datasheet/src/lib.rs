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
    let (in_ch, out_ch) = infer_channel_counts(instance);
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
    let (warm_out, _, _) = process_audio_stream(instance, &silence, out_ch, n_warmup, 256, events)?;
    let dc_baseline = warm_out
        .first()
        .and_then(|ch| ch.last().copied())
        .unwrap_or(0.0);
    let ir_trigger_events: &[ProcessEvent] = if in_ch == 0 { &inst_events } else { &[] };
    let (ir_out, _, _) =
        process_audio_stream(instance, &ir_in, out_ch, total_ir, 256, ir_trigger_events)?;
    instance.stop_processing();
    instance.deactivate();
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
        let sw_in = vec![sw_padded; in_ch];
        instance.activate(sample_rate, 1, 512)?;
        instance.start_processing()?;
        instance.reset();
        let _ = instance.flush_params(events, &mut flush_out);
        let _ = process_audio_stream(instance, &silence, out_ch, n_warmup, 256, events)?;
        let (sw_out, _, _) = process_audio_stream(instance, &sw_in, out_ch, total_ir, 256, &[])?;
        instance.stop_processing();
        instance.deactivate();
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
    let (sine_out, _, _) =
        process_audio_stream(instance, &sine_in, out_ch, n_sine, 256, &inst_events)?;
    instance.stop_processing();
    instance.deactivate();
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
        let _ = process_audio_stream(instance, &silence, out_ch, n_warmup, 256, events);
        let res = process_audio_stream(instance, &in_bufs, out_ch, total_frames, 256, &[]);
        instance.stop_processing();
        instance.deactivate();
        res.map(|(chs, _, _)| chs[0][lat..lat + frames].to_vec())
            .unwrap_or_else(|_| vec![0.0; frames])
    });

    Ok(SweepTraceSnapshot {
        label,
        bode,
        thdn,
        dynamics,
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
    for (idx, p) in param_infos.iter().enumerate() {
        let cur = if idx < 32 {
            instance.get_param_value(p.id).unwrap_or(p.default_value)
        } else {
            p.default_value
        };
        let txt = if idx < 32 {
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

    let (in_ch, out_ch) = infer_channel_counts(instance);
    let is_inst = is_instrument_plugin(instance, in_ch);
    let inst_key = default_instrument_midi_key(instance);

    // Primary trace snapshot (measured on pristine factory instance before validation/GUI)
    let primary =
        measure_trace_snapshot(instance, sample_rate, base_events, descriptor.name.clone())?;

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
    let (tt_out, _, _) = process_audio_stream(instance, &tt_in, out_ch, n_sec, 256, &tt_events)?;
    instance.stop_processing();
    instance.deactivate();
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
    let (imp_out, _, _) =
        process_audio_stream(instance, &imp_in, out_ch, n_tail, 256, &imp_events)?;
    instance.stop_processing();
    instance.deactivate();
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
            || (primary.dynamics.estimated_ratio - 1.0).abs() > 0.15,
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
}
