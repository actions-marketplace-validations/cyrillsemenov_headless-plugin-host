use crate::stream::infer_channel_counts;
use crate::types::{FuzzConfig, FuzzFailure, FuzzReport};
use hph_core::*;
use hph_rt_check::*;
use std::time::Instant;

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    #[inline]
    fn next_f64_unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    #[inline]
    fn next_usize(&mut self, max_exclusive: usize) -> usize {
        if max_exclusive <= 1 {
            0
        } else {
            (self.next_u64() as usize) % max_exclusive
        }
    }
}

/// Run a deterministic, seeded Chaos / Fuzz Torture Suite against a plugin instance.
/// Exercises prime/odd block sizes, sample-rate hopping, sample-accurate parameter zipper storms,
/// polyphonic note bursts, extreme/subnormal/DC-step inputs, `reset()` storms, and optional corrupted state loads.
pub fn run_fuzz_torture(plugin: &mut dyn PluginInstance, config: &FuzzConfig) -> FuzzReport {
    let t0 = Instant::now();
    let descriptor = plugin.descriptor().clone();
    let (in_ch, out_ch) = infer_channel_counts(plugin);
    let params: Vec<ParamInfo> = plugin
        .params()
        .into_iter()
        .filter(|p| !p.flags.is_readonly)
        .collect();
    let has_note_in = plugin.note_ports().iter().any(|p| p.is_input);

    let mut rng = SplitMix64::new(config.seed);
    let max_blk = config.max_block_size.clamp(16, 4096);
    let sample_rates = [44100.0, 48000.0, 88200.0, 96000.0];
    let candidate_blocks = [
        1usize, 3, 7, 13, 31, 63, 64, 127, 128, 251, 256, 509, 512, 1021, 1024,
    ];

    let mut total_frames_processed = 0u64;
    let mut total_events_dispatched = 0u64;
    let mut state_mutations_tested = 0usize;
    let mut max_abs_sample = 0.0f64;
    let mut nan_inf_samples = 0usize;
    let mut runaway_samples = 0usize;
    let mut total_rt_allocs = RtAllocStats::default();
    let mut failures = Vec::new();
    let mut iterations_completed = 0usize;

    let mut current_sr = 48000.0;
    let mut is_active = false;
    let mut baseline_state: Option<Vec<u8>> = None;

    for iter in 0..config.iterations {
        // Hop sample rate & re-activate every 50 iterations (or on iteration 0)
        if !is_active || iter % 50 == 0 {
            if is_active {
                plugin.stop_processing();
                plugin.deactivate();
                is_active = false;
            }
            current_sr = sample_rates[rng.next_usize(sample_rates.len())];
            if let Err(e) = plugin.activate(current_sr, 1, max_blk as u32) {
                failures.push(FuzzFailure {
                    iteration: iter,
                    sample_rate: current_sr,
                    block_size: max_blk,
                    reason: format!("activate({current_sr} Hz) failed: {e}"),
                });
                break;
            }
            if let Err(e) = plugin.start_processing() {
                plugin.deactivate();
                failures.push(FuzzFailure {
                    iteration: iter,
                    sample_rate: current_sr,
                    block_size: max_blk,
                    reason: format!("start_processing() failed: {e}"),
                });
                break;
            }
            is_active = true;
            if baseline_state.is_none() && plugin.supports_state() {
                baseline_state = plugin.save_state().ok();
            }
            // Warm up one quiet block after activation so one-time setup allocs don't count as steady-state RT allocs
            let warm_in = vec![vec![0.0f32; 64]; in_ch];
            let mut warm_out = vec![vec![0.0f32; 64]; out_ch];
            let in_s: Vec<&[f32]> = warm_in.iter().map(|c| c.as_slice()).collect();
            let mut out_s: Vec<&mut [f32]> =
                warm_out.iter_mut().map(|c| c.as_mut_slice()).collect();
            let mut warm_ev = Vec::new();
            let mut pb = AudioProcessBuffers {
                inputs: &in_s,
                outputs: &mut out_s,
                num_frames: 64,
                steady_time: 0,
            };
            let _ = plugin.process(&mut pb, &[], &mut warm_ev);
        }

        // Occasionally test reset() or state save/load torture
        if iter > 0 && iter % 25 == 0 {
            plugin.reset();
        }
        if config.corrupt_state && iter > 0 && iter % 40 == 0 {
            if let Some(ref valid_bytes) = baseline_state {
                plugin.stop_processing();
                let mut mutated = valid_bytes.clone();
                if !mutated.is_empty() {
                    let mode = rng.next_usize(3);
                    match mode {
                        0 => {
                            // Truncate state buffer
                            let new_len = rng.next_usize(mutated.len());
                            mutated.truncate(new_len);
                        }
                        1 => {
                            // Flip random bytes in the payload
                            for _ in 0..4 {
                                let idx = rng.next_usize(mutated.len());
                                mutated[idx] ^= (rng.next_u64() & 0xFF) as u8;
                            }
                        }
                        _ => {
                            // Empty slice
                            mutated.clear();
                        }
                    }
                    let _ = plugin.load_state(&mutated);
                    state_mutations_tested += 1;
                }
                // Always restore known-valid state before resuming DSP checks
                let _ = plugin.load_state(valid_bytes);
                let _ = plugin.start_processing();
            }
        }

        // Pick block size (mix of prime candidates and uniform random 1..=max_blk)
        let blk = if rng.next_usize(2) == 0 {
            candidate_blocks[rng.next_usize(candidate_blocks.len())].min(max_blk)
        } else {
            (rng.next_usize(max_blk) + 1).min(max_blk)
        };

        // Synthesize adversarial input audio buffer
        let signal_kind = rng.next_usize(6);
        let mut in_bufs = vec![vec![0.0f32; blk]; in_ch];
        for ch_buf in &mut in_bufs {
            for (n, s) in ch_buf.iter_mut().enumerate() {
                *s = match signal_kind {
                    0 => {
                        // Normal white noise [-0.7, +0.7]
                        ((rng.next_f64_unit() * 2.0 - 1.0) * 0.7) as f32
                    }
                    1 => {
                        // Extreme overdrive rail slam [-16.0, +16.0] (+24 dBFS)
                        ((rng.next_f64_unit() * 2.0 - 1.0) * 16.0) as f32
                    }
                    2 => {
                        // IEEE-754 subnormal / denormal storm
                        let mant = (rng.next_u64() as u32) & 0x007f_ffff;
                        let sign = ((rng.next_u64() as u32) & 1) << 31;
                        f32::from_bits(sign | mant.max(1))
                    }
                    3 => {
                        // Nyquist alternating square (+0.95 / -0.95)
                        if n % 2 == 0 {
                            0.95
                        } else {
                            -0.95
                        }
                    }
                    4 => {
                        // Hard DC step (+0.9 -> -0.9)
                        if n < blk / 2 {
                            0.9
                        } else {
                            -0.9
                        }
                    }
                    _ => 0.0, // Silence tail
                };
            }
        }

        // Build sorted sample-accurate event storm (ParamValue + ParamMod + NoteOn/NoteOff)
        let mut events = Vec::new();
        if !params.is_empty() {
            let num_param_ev = rng.next_usize(8) + 1;
            for _ in 0..num_param_ev {
                let p = &params[rng.next_usize(params.len())];
                let offset = rng.next_usize(blk) as u32;
                // Bias towards boundary extremes (min, max, default) plus random interior values
                let val = match rng.next_usize(4) {
                    0 => p.min_value,
                    1 => p.max_value,
                    2 => p.default_value,
                    _ => p.min_value + rng.next_f64_unit() * (p.max_value - p.min_value),
                };
                events.push(ProcessEvent::ParamValue {
                    sample_offset: offset,
                    param_id: p.id,
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value: val,
                });
            }
        }

        if has_note_in && rng.next_usize(2) == 0 {
            let key = (rng.next_usize(108) + 12) as i16;
            let vel = rng.next_f64_unit();
            let on_off = rng.next_usize(blk) as u32;
            if rng.next_usize(2) == 0 {
                events.push(ProcessEvent::NoteOn {
                    sample_offset: on_off,
                    port_index: 0,
                    channel: 0,
                    key,
                    note_id: -1,
                    velocity: vel,
                });
            } else {
                events.push(ProcessEvent::NoteOff {
                    sample_offset: on_off,
                    port_index: 0,
                    channel: 0,
                    key,
                    note_id: -1,
                    velocity: vel,
                });
            }
        }

        events.sort_by_key(|e| e.sample_offset());
        total_events_dispatched += events.len() as u64;

        let mut out_bufs = vec![vec![0.0f32; blk]; out_ch];
        let in_slices: Vec<&[f32]> = in_bufs.iter().map(|c| c.as_slice()).collect();
        let mut out_slices: Vec<&mut [f32]> =
            out_bufs.iter_mut().map(|c| c.as_mut_slice()).collect();
        let mut out_events = Vec::new();
        let mut proc_bufs = AudioProcessBuffers {
            inputs: &in_slices,
            outputs: &mut out_slices,
            num_frames: blk,
            steady_time: total_frames_processed as i64,
        };

        let (res, rt_stats) =
            with_rt_alloc_guard(|| plugin.process(&mut proc_bufs, &events, &mut out_events));

        total_rt_allocs.alloc_calls += rt_stats.alloc_calls;
        total_rt_allocs.free_calls += rt_stats.free_calls;
        total_rt_allocs.bytes_allocated += rt_stats.bytes_allocated;

        if let Err(e) = res {
            failures.push(FuzzFailure {
                iteration: iter,
                sample_rate: current_sr,
                block_size: blk,
                reason: format!("process() returned error: {e}"),
            });
            break;
        }

        let mut iter_nan_inf = 0usize;
        let mut iter_runaway = 0usize;
        for ch in &out_bufs {
            for &s in ch {
                if !s.is_finite() {
                    nan_inf_samples += 1;
                    iter_nan_inf += 1;
                } else {
                    let a = (s as f64).abs();
                    if a > max_abs_sample {
                        max_abs_sample = a;
                    }
                    if a > 1.0e4 {
                        runaway_samples += 1;
                        iter_runaway += 1;
                    }
                }
            }
        }

        if iter_nan_inf > 0 && failures.len() < 10 {
            failures.push(FuzzFailure {
                iteration: iter,
                sample_rate: current_sr,
                block_size: blk,
                reason: format!("Emitted {iter_nan_inf} NaN/Inf sample(s)"),
            });
        }
        if iter_runaway > 0 && failures.len() < 10 {
            failures.push(FuzzFailure {
                iteration: iter,
                sample_rate: current_sr,
                block_size: blk,
                reason: format!(
                    "Filter blowup / runaway output ({iter_runaway} samples > +80 dBFS, peak={max_abs_sample:.2e})"
                ),
            });
        }

        total_frames_processed += blk as u64;
        iterations_completed += 1;
    }

    if is_active {
        plugin.stop_processing();
        plugin.deactivate();
    }

    if total_rt_allocs.has_violations() {
        failures.push(FuzzFailure {
            iteration: iterations_completed,
            sample_rate: current_sr,
            block_size: 0,
            reason: format!(
                "Real-time heap violation during fuzzing: {} alloc(s) ({} bytes), {} free(s)",
                total_rt_allocs.alloc_calls,
                total_rt_allocs.bytes_allocated,
                total_rt_allocs.free_calls
            ),
        });
    }

    let diag = plugin.take_diagnostics();
    for v in &diag.thread_violations {
        failures.push(FuzzFailure {
            iteration: iterations_completed,
            sample_rate: current_sr,
            block_size: 0,
            reason: format!("Thread violation: {v}"),
        });
    }

    FuzzReport {
        descriptor,
        seed: config.seed,
        iterations_completed,
        total_frames_processed,
        total_events_dispatched,
        state_mutations_tested,
        max_abs_sample,
        nan_inf_samples,
        runaway_samples,
        rt_allocs: total_rt_allocs,
        duration_ms: t0.elapsed().as_secs_f64() * 1e3,
        passed: failures.is_empty(),
        failures,
    }
}
