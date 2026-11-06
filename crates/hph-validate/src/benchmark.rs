use crate::stream::infer_channel_counts;
use crate::types::{BenchmarkReport, DspBenchBlockStats};
use hph_core::*;
use hph_dsp::*;
use hph_rt_check::*;
use std::time::Instant;

/// Run DSP & optional GUI benchmarks across sample rates and buffer sizes.
pub fn run_benchmarks(
    plugin: &mut dyn PluginInstance,
    sample_rates: &[f64],
    block_sizes: &[usize],
    blocks_per_config: usize,
    bench_gui: bool,
) -> HostResult<BenchmarkReport> {
    let descriptor = plugin.descriptor().clone();
    let (in_ch, out_ch) = infer_channel_counts(plugin);
    let mut dsp_configs = Vec::new();

    for &sr in sample_rates {
        for &blk in block_sizes {
            let stim = generate_signal(
                &SignalSpec::PinkNoise {
                    amplitude: 0.5,
                    seed: 42,
                },
                sr,
                blk,
            );
            let in_bufs = vec![stim; in_ch];
            let mut out_bufs = vec![vec![0.0f32; blk]; out_ch];
            let note_on = [ProcessEvent::NoteOn {
                sample_offset: 0,
                port_index: 0,
                channel: 0,
                key: 60,
                note_id: 1,
                velocity: 0.8,
            }];

            plugin.activate(sr, 1, blk as u32)?;
            plugin.start_processing()?;
            plugin.reset();

            // Trigger note for synth/instrument plugins during warmup
            {
                let in_slices: Vec<&[f32]> = in_bufs.iter().map(|c| c.as_slice()).collect();
                let mut out_slices: Vec<&mut [f32]> =
                    out_bufs.iter_mut().map(|c| c.as_mut_slice()).collect();
                let mut out_ev = Vec::new();
                let mut pb = AudioProcessBuffers {
                    inputs: &in_slices,
                    outputs: &mut out_slices,
                    num_frames: blk,
                    steady_time: 0,
                };
                let _ = plugin.process(&mut pb, &note_on, &mut out_ev);
            }

            let n_iters = blocks_per_config.max(20);
            let mut times_us = Vec::with_capacity(n_iters);
            let mut total_rt_allocs = RtAllocStats::default();

            for i in 0..n_iters {
                let in_slices: Vec<&[f32]> = in_bufs.iter().map(|c| c.as_slice()).collect();
                let mut out_slices: Vec<&mut [f32]> =
                    out_bufs.iter_mut().map(|c| c.as_mut_slice()).collect();
                let mut out_ev = Vec::new();
                let mut pb = AudioProcessBuffers {
                    inputs: &in_slices,
                    outputs: &mut out_slices,
                    num_frames: blk,
                    steady_time: ((i + 1) * blk) as i64,
                };

                let t0 = Instant::now();
                let (res, rt) = with_rt_alloc_guard(|| plugin.process(&mut pb, &[], &mut out_ev));
                let elapsed_us = t0.elapsed().as_secs_f64() * 1e6;
                res?;

                times_us.push(elapsed_us);
                total_rt_allocs.alloc_calls += rt.alloc_calls;
                total_rt_allocs.free_calls += rt.free_calls;
                total_rt_allocs.bytes_allocated += rt.bytes_allocated;
            }

            plugin.stop_processing();
            plugin.deactivate();

            times_us.sort_by(f64::total_cmp);
            let mean_us = times_us.iter().sum::<f64>() / (n_iters as f64);
            let p50_us = times_us[n_iters / 2];
            let p90_us = times_us[(n_iters * 90) / 100];
            let p99_us = times_us[(n_iters * 99) / 100];
            let max_us = *times_us.last().unwrap_or(&0.0);

            let buffer_deadline_us = (blk as f64) / sr * 1e6;
            let cpu_load_percent = (mean_us / buffer_deadline_us) * 100.0;
            let realtime_speedup_x = buffer_deadline_us / mean_us.max(1e-6);

            dsp_configs.push(DspBenchBlockStats {
                sample_rate: sr,
                block_size: blk,
                blocks_processed: n_iters,
                mean_us,
                p50_us,
                p90_us,
                p99_us,
                max_us,
                buffer_deadline_us,
                cpu_load_percent,
                realtime_speedup_x,
                rt_allocs: total_rt_allocs,
            });
        }
    }

    // Measure Denormal CPU Slowdown Ratio (normal amplitude vs subnormal noise at 48kHz / 256)
    let denormal_slowdown_ratio = {
        let sr = 48000.0;
        let blk = 256;
        let normal_in = vec![
            generate_signal(
                &SignalSpec::WhiteNoise {
                    amplitude: 0.25,
                    seed: 1,
                },
                sr,
                blk,
            );
            in_ch
        ];
        let sub_in = vec![generate_signal(&SignalSpec::SubnormalNoise { seed: 2 }, sr, blk); in_ch];
        let mut out_bufs = vec![vec![0.0f32; blk]; out_ch];

        plugin.activate(sr, 1, blk as u32)?;
        plugin.start_processing()?;
        plugin.reset();

        let t_norm = Instant::now();
        for i in 0..100 {
            let in_s: Vec<&[f32]> = normal_in.iter().map(|c| c.as_slice()).collect();
            let mut out_s: Vec<&mut [f32]> =
                out_bufs.iter_mut().map(|c| c.as_mut_slice()).collect();
            let mut ev = Vec::new();
            let mut pb = AudioProcessBuffers {
                inputs: &in_s,
                outputs: &mut out_s,
                num_frames: blk,
                steady_time: (i * blk) as i64,
            };
            let _ = plugin.process(&mut pb, &[], &mut ev);
        }
        let norm_us = t_norm.elapsed().as_secs_f64().max(1e-9);

        plugin.reset();
        let t_sub = Instant::now();
        for i in 0..100 {
            let in_s: Vec<&[f32]> = sub_in.iter().map(|c| c.as_slice()).collect();
            let mut out_s: Vec<&mut [f32]> =
                out_bufs.iter_mut().map(|c| c.as_mut_slice()).collect();
            let mut ev = Vec::new();
            let mut pb = AudioProcessBuffers {
                inputs: &in_s,
                outputs: &mut out_s,
                num_frames: blk,
                steady_time: (i * blk) as i64,
            };
            let _ = plugin.process(&mut pb, &[], &mut ev);
        }
        let sub_us = t_sub.elapsed().as_secs_f64();

        plugin.stop_processing();
        plugin.deactivate();

        sub_us / norm_us
    };

    let gui_benchmark = if bench_gui {
        hph_gui::benchmark_plugin_gui(plugin, 30).ok()
    } else {
        None
    };

    Ok(BenchmarkReport {
        descriptor,
        dsp_configs,
        denormal_slowdown_ratio,
        gui_benchmark,
    })
}
