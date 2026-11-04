use hph_au_sys::*;
use std::ffi::c_void;
use std::ptr;

/// Pre-allocated context passed into `AURenderCallback` and `HostCallbackInfo` during `AudioUnitRender`.
pub(crate) struct AuRenderContext {
    pub(crate) input_ptrs: Vec<*const f32>,
    pub(crate) num_input_channels: usize,
    pub(crate) num_frames: usize,
    pub(crate) sample_rate: f64,
    pub(crate) sample_time: f64,
}

pub(crate) unsafe extern "C" fn au_render_input_callback(
    in_ref_con: *mut c_void,
    io_action_flags: *mut AudioUnitRenderActionFlags,
    _in_time_stamp: *const AudioTimeStamp,
    _in_bus_number: UInt32,
    in_number_frames: UInt32,
    io_data: *mut AudioBufferList,
) -> OSStatus {
    if in_ref_con.is_null() || io_data.is_null() {
        return noErr;
    }
    let ctx = &*(in_ref_con as *const AuRenderContext);
    let num_buffers = (*io_data).mNumberBuffers as usize;
    let buffers_ptr = (*io_data).mBuffers.as_ptr() as *mut AudioBuffer;
    let frames = (in_number_frames as usize).min(ctx.num_frames);
    let byte_len = frames * std::mem::size_of::<f32>();

    if ctx.num_input_channels == 0 {
        if !io_action_flags.is_null() {
            *io_action_flags |= kAudioUnitRenderAction_OutputIsSilence;
        }
        for b in 0..num_buffers {
            let buf = &mut *buffers_ptr.add(b);
            if !buf.mData.is_null() && (buf.mDataByteSize as usize) >= byte_len {
                ptr::write_bytes(buf.mData as *mut u8, 0, byte_len);
            }
        }
        return noErr;
    }

    for b in 0..num_buffers {
        let buf = &mut *buffers_ptr.add(b);
        let src_ch = b % ctx.num_input_channels;
        let src_ptr = ctx.input_ptrs[src_ch];
        if !buf.mData.is_null() && !src_ptr.is_null() && frames > 0 {
            ptr::copy_nonoverlapping(src_ptr, buf.mData as *mut f32, frames);
            buf.mDataByteSize = byte_len as u32;
        }
    }
    noErr
}

pub(crate) unsafe extern "C" fn host_get_beat_and_tempo(
    in_user_data: *mut c_void,
    out_beat: *mut Float64,
    out_tempo: *mut Float64,
) -> OSStatus {
    let tempo = 120.0;
    if !out_tempo.is_null() {
        *out_tempo = tempo;
    }
    if !out_beat.is_null() && !in_user_data.is_null() {
        let ctx = &*(in_user_data as *const AuRenderContext);
        let seconds = ctx.sample_time / ctx.sample_rate.max(1.0);
        *out_beat = seconds * (tempo / 60.0);
    }
    noErr
}

pub(crate) unsafe extern "C" fn host_get_musical_time_location(
    _in_user_data: *mut c_void,
    out_delta: *mut UInt32,
    out_num: *mut Float32,
    out_denom: *mut UInt32,
    out_downbeat: *mut Float64,
) -> OSStatus {
    if !out_delta.is_null() {
        *out_delta = 0;
    }
    if !out_num.is_null() {
        *out_num = 4.0;
    }
    if !out_denom.is_null() {
        *out_denom = 4;
    }
    if !out_downbeat.is_null() {
        *out_downbeat = 0.0;
    }
    noErr
}

pub(crate) unsafe extern "C" fn host_get_transport_state(
    in_user_data: *mut c_void,
    out_playing: *mut Boolean,
    out_changed: *mut Boolean,
    out_sample_in_timeline: *mut Float64,
    out_cycling: *mut Boolean,
    out_cycle_start: *mut Float64,
    out_cycle_end: *mut Float64,
) -> OSStatus {
    if !out_playing.is_null() {
        *out_playing = 1;
    }
    if !out_changed.is_null() {
        *out_changed = 0;
    }
    if !out_sample_in_timeline.is_null() && !in_user_data.is_null() {
        let ctx = &*(in_user_data as *const AuRenderContext);
        *out_sample_in_timeline = ctx.sample_time;
    }
    if !out_cycling.is_null() {
        *out_cycling = 0;
    }
    if !out_cycle_start.is_null() {
        *out_cycle_start = 0.0;
    }
    if !out_cycle_end.is_null() {
        *out_cycle_end = 0.0;
    }
    noErr
}
