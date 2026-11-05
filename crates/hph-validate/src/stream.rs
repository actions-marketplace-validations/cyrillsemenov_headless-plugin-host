use hph_core::{AudioProcessBuffers, HostResult, PluginInstance, ProcessEvent};
use hph_rt_check::{with_rt_alloc_guard, RtAllocStats};

/// Helper to process planar multi-channel audio through a `PluginInstance` in chunks of `block_size`.
pub fn process_audio_stream(
    plugin: &mut dyn PluginInstance,
    in_channels: &[Vec<f32>],
    num_out_channels: usize,
    num_frames: usize,
    block_size: usize,
    events: &[ProcessEvent],
) -> HostResult<(Vec<Vec<f32>>, Vec<ProcessEvent>, RtAllocStats)> {
    let mut out_channels = vec![vec![0.0f32; num_frames]; num_out_channels.max(1)];
    let mut all_out_events = Vec::new();
    let mut total_rt_allocs = RtAllocStats::default();

    let mut pos = 0;
    let blk = block_size.max(1);
    while pos < num_frames {
        let frames = (num_frames - pos).min(blk);
        let in_slices: Vec<&[f32]> = in_channels
            .iter()
            .map(|ch| &ch[pos..pos + frames])
            .collect();
        let mut out_slices: Vec<&mut [f32]> = out_channels
            .iter_mut()
            .map(|ch| &mut ch[pos..pos + frames])
            .collect();

        // Filter events falling into [pos, pos + frames) and rebase their sample_offset
        let block_events: Vec<ProcessEvent> = events
            .iter()
            .filter_map(|e| {
                let t = e.sample_offset() as usize;
                if t >= pos && t < pos + frames {
                    let rel = (t - pos) as u32;
                    Some(match *e {
                        ProcessEvent::NoteOn {
                            port_index,
                            channel,
                            key,
                            note_id,
                            velocity,
                            ..
                        } => ProcessEvent::NoteOn {
                            sample_offset: rel,
                            port_index,
                            channel,
                            key,
                            note_id,
                            velocity,
                        },
                        ProcessEvent::NoteOff {
                            port_index,
                            channel,
                            key,
                            note_id,
                            velocity,
                            ..
                        } => ProcessEvent::NoteOff {
                            sample_offset: rel,
                            port_index,
                            channel,
                            key,
                            note_id,
                            velocity,
                        },
                        ProcessEvent::ParamValue {
                            param_id,
                            note_id,
                            port_index,
                            channel,
                            key,
                            value,
                            ..
                        } => ProcessEvent::ParamValue {
                            sample_offset: rel,
                            param_id,
                            note_id,
                            port_index,
                            channel,
                            key,
                            value,
                        },
                        ProcessEvent::ParamMod {
                            param_id,
                            note_id,
                            port_index,
                            channel,
                            key,
                            amount,
                            ..
                        } => ProcessEvent::ParamMod {
                            sample_offset: rel,
                            param_id,
                            note_id,
                            port_index,
                            channel,
                            key,
                            amount,
                        },
                        ProcessEvent::Midi1 {
                            port_index, data, ..
                        } => ProcessEvent::Midi1 {
                            sample_offset: rel,
                            port_index,
                            data,
                        },
                    })
                } else {
                    None
                }
            })
            .collect();

        let mut block_out_events = Vec::new();
        let mut proc_bufs = AudioProcessBuffers {
            inputs: &in_slices,
            outputs: &mut out_slices,
            num_frames: frames,
            steady_time: pos as i64,
        };

        let (res, rt_stats) = with_rt_alloc_guard(|| {
            plugin.process(&mut proc_bufs, &block_events, &mut block_out_events)
        });
        res?;

        total_rt_allocs.alloc_calls += rt_stats.alloc_calls;
        total_rt_allocs.free_calls += rt_stats.free_calls;
        total_rt_allocs.bytes_allocated += rt_stats.bytes_allocated;

        all_out_events.extend(block_out_events);
        pos += frames;
    }

    Ok((out_channels, all_out_events, total_rt_allocs))
}

pub fn infer_channel_counts(plugin: &dyn PluginInstance) -> (usize, usize) {
    let ports = plugin.audio_ports();
    if ports.is_empty() {
        return (2, 2);
    }
    let in_ch = ports
        .iter()
        .find(|p| p.is_input && p.is_main)
        .or_else(|| ports.iter().find(|p| p.is_input))
        .map(|p| p.channel_count as usize)
        .unwrap_or(0);
    let out_ch = ports
        .iter()
        .find(|p| !p.is_input && p.is_main)
        .or_else(|| ports.iter().find(|p| !p.is_input))
        .map(|p| p.channel_count as usize)
        .unwrap_or(2);
    (in_ch, out_ch.max(1))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MultiBusLayout {
    pub main_in_channels: usize,
    pub aux_in_channels: usize,
    pub total_in_channels: usize,
    pub in_bus_counts: Vec<usize>,
    pub main_out_channels: usize,
    pub aux_out_channels: usize,
    pub total_out_channels: usize,
    pub out_bus_counts: Vec<usize>,
}

pub fn infer_multibus_layout(plugin: &dyn PluginInstance) -> MultiBusLayout {
    let ports = plugin.audio_ports();
    if ports.is_empty() {
        return MultiBusLayout {
            main_in_channels: 2,
            aux_in_channels: 0,
            total_in_channels: 2,
            in_bus_counts: vec![2],
            main_out_channels: 2,
            aux_out_channels: 0,
            total_out_channels: 2,
            out_bus_counts: vec![2],
        };
    }

    let in_ports: Vec<_> = ports.iter().filter(|p| p.is_input).collect();
    let out_ports: Vec<_> = ports.iter().filter(|p| !p.is_input).collect();

    let in_bus_counts: Vec<usize> = in_ports
        .iter()
        .map(|p| (p.channel_count as usize).max(1))
        .collect();
    let main_in_channels = in_bus_counts.first().copied().unwrap_or(0);
    let total_in_channels: usize = in_bus_counts.iter().sum();
    let aux_in_channels = total_in_channels.saturating_sub(main_in_channels);

    let mut out_bus_counts: Vec<usize> = out_ports
        .iter()
        .map(|p| (p.channel_count as usize).max(1))
        .collect();
    if out_bus_counts.is_empty() {
        out_bus_counts.push(2);
    }
    let main_out_channels = out_bus_counts.first().copied().unwrap_or(2).max(1);
    let total_out_channels: usize = out_bus_counts.iter().sum();
    let aux_out_channels = total_out_channels.saturating_sub(main_out_channels);

    MultiBusLayout {
        main_in_channels,
        aux_in_channels,
        total_in_channels,
        in_bus_counts,
        main_out_channels,
        aux_out_channels,
        total_out_channels,
        out_bus_counts,
    }
}

/// Mix down multi-bus output channels (`total_out_channels`) into the main output bus (`main_out_channels`)
/// when a multi-output instrument routes voices across auxiliary output stems `1..N` or when Bus 0 is silent.
pub fn mix_down_output_buses(
    out_channels: &[Vec<f32>],
    layout: &MultiBusLayout,
    sum_all_active_stems: bool,
) -> Vec<Vec<f32>> {
    let main_ch = layout.main_out_channels.max(1);
    let frames = out_channels.first().map(|c| c.len()).unwrap_or(0);
    let mut mixed: Vec<Vec<f32>> = (0..main_ch)
        .map(|ch| {
            out_channels
                .get(ch)
                .cloned()
                .unwrap_or_else(|| vec![0.0f32; frames])
        })
        .collect();

    if layout.out_bus_counts.len() <= 1 || out_channels.len() <= main_ch {
        return mixed;
    }

    let main_energy: f64 = mixed
        .iter()
        .flat_map(|c| c.iter())
        .map(|&s| (s as f64) * (s as f64))
        .sum();
    let main_silent = main_energy < 1e-12;

    if !sum_all_active_stems && !main_silent {
        return mixed;
    }

    let mut ch_offset = main_ch;
    for &bus_ch in layout.out_bus_counts.iter().skip(1) {
        if ch_offset + bus_ch > out_channels.len() {
            break;
        }
        for local_ch in 0..bus_ch {
            let src = &out_channels[ch_offset + local_ch];
            let dst = &mut mixed[local_ch % main_ch];
            for (d, &s) in dst.iter_mut().zip(src.iter()) {
                *d += s;
            }
        }
        ch_offset += bus_ch;
    }

    mixed
}
