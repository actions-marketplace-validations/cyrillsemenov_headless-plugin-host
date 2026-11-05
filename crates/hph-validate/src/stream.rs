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
