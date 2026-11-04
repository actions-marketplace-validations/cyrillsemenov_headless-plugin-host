use hph_clap_sys::*;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::NSView;
use std::f64::consts::PI;
use std::ffi::c_char;
#[cfg(target_os = "windows")]
use std::ffi::c_void;
use std::ptr;

pub const PARAM_GAIN: clap_id = 100;
pub const PARAM_CUTOFF: clap_id = 101;
pub const PARAM_DRIVE: clap_id = 102;
pub const PARAM_BYPASS: clap_id = 103;

static FEAT_AUDIO_EFFECT: &[u8] = b"audio-effect\0";
static FEAT_FILTER: &[u8] = b"filter\0";
static FEAT_STEREO: &[u8] = b"stereo\0";

#[repr(C)]
struct FeatureList([*const c_char; 4]);
unsafe impl Sync for FeatureList {}

static FEATURES: FeatureList = FeatureList([
    FEAT_AUDIO_EFFECT.as_ptr() as *const c_char,
    FEAT_FILTER.as_ptr() as *const c_char,
    FEAT_STEREO.as_ptr() as *const c_char,
    ptr::null(),
]);

#[repr(C)]
pub struct SyncDescriptor(pub clap_plugin_descriptor_t);
unsafe impl Sync for SyncDescriptor {}

pub static MOCK_DESCRIPTOR: SyncDescriptor = SyncDescriptor(clap_plugin_descriptor_t {
    clap_version: CLAP_VERSION,
    id: c"com.hph.mock-filter-saturator".as_ptr(),
    name: c"HPH Mock Filter & Saturator".as_ptr(),
    vendor: c"HPH Audio Labs".as_ptr(),
    url: c"https://github.com/afab/headless-plugin-host".as_ptr(),
    manual_url: c"https://github.com/afab/headless-plugin-host".as_ptr(),
    support_url: c"https://github.com/afab/headless-plugin-host".as_ptr(),
    version: c"1.0.0".as_ptr(),
    description:
        c"Reference CLAP test plugin with SVF filter, saturator, synth voice and Cocoa GUI".as_ptr(),
    features: FEATURES.0.as_ptr(),
});

pub struct MockPluginState {
    pub _host: *const clap_host_t,
    pub sample_rate: f64,
    pub gain: f64,
    pub cutoff_hz: f64,
    pub drive: f64,
    pub bypass: f64,
    pub lp_state: [f64; 2],
    pub synth_phase: f64,
    pub synth_freq: f64,
    pub synth_gate: f64,
    pub gui_width: u32,
    pub gui_height: u32,
    #[cfg(target_os = "macos")]
    pub child_view: Option<Retained<NSView>>,
    #[cfg(target_os = "windows")]
    pub child_hwnd: *mut c_void,
    #[cfg(target_os = "linux")]
    pub x11_state: Option<crate::gui::linux::MockX11State>,
}

impl MockPluginState {
    pub fn new(host: *const clap_host_t) -> Self {
        Self {
            _host: host,
            sample_rate: 48000.0,
            gain: 1.0,
            cutoff_hz: 4000.0,
            drive: 0.0,
            bypass: 0.0,
            lp_state: [0.0; 2],
            synth_phase: 0.0,
            synth_freq: 440.0,
            synth_gate: 0.0,
            gui_width: 480,
            gui_height: 300,
            #[cfg(target_os = "macos")]
            child_view: None,
            #[cfg(target_os = "windows")]
            child_hwnd: ptr::null_mut(),
            #[cfg(target_os = "linux")]
            x11_state: None,
        }
    }

    #[inline]
    pub fn apply_event(&mut self, hdr: &clap_event_header_t) {
        if hdr.space_id != CLAP_CORE_EVENT_SPACE_ID {
            return;
        }
        unsafe {
            match hdr.type_ {
                CLAP_EVENT_PARAM_VALUE => {
                    let ev = &*(hdr as *const _ as *const clap_event_param_value_t);
                    match ev.param_id {
                        PARAM_GAIN => self.gain = ev.value.clamp(0.0, 2.0),
                        PARAM_CUTOFF => self.cutoff_hz = ev.value.clamp(20.0, 20000.0),
                        PARAM_DRIVE => self.drive = ev.value.clamp(0.0, 1.0),
                        PARAM_BYPASS => self.bypass = if ev.value >= 0.5 { 1.0 } else { 0.0 },
                        _ => {}
                    }
                }
                CLAP_EVENT_NOTE_ON => {
                    let ev = &*(hdr as *const _ as *const clap_event_note_t);
                    self.synth_freq = 440.0 * 2.0f64.powf((ev.key as f64 - 69.0) / 12.0);
                    self.synth_gate = ev.velocity.clamp(0.0, 1.0);
                }
                CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE => {
                    self.synth_gate = 0.0;
                }
                _ => {}
            }
        }
    }
}

pub unsafe fn copy_str_to_fixed<const N: usize>(dst: &mut [c_char; N], s: &str) {
    dst.fill(0);
    let bytes = s.as_bytes();
    let len = bytes.len().min(N - 1);
    for i in 0..len {
        dst[i] = bytes[i] as c_char;
    }
}

// --- Plugin Callbacks ---

pub unsafe extern "C" fn plug_init(_plugin: *const clap_plugin_t) -> bool {
    true
}

pub unsafe extern "C" fn plug_destroy(plugin: *const clap_plugin_t) {
    if !plugin.is_null() {
        let boxed_plugin = Box::from_raw(plugin as *mut clap_plugin_t);
        if !boxed_plugin.plugin_data.is_null() {
            let _ = Box::from_raw(boxed_plugin.plugin_data as *mut MockPluginState);
        }
    }
}

pub unsafe extern "C" fn plug_activate(
    plugin: *const clap_plugin_t,
    sample_rate: f64,
    _min_frames: u32,
    _max_frames: u32,
) -> bool {
    let st = &mut *((*plugin).plugin_data as *mut MockPluginState);
    st.sample_rate = sample_rate.max(8000.0);
    st.lp_state = [0.0; 2];
    st.synth_phase = 0.0;
    true
}

pub unsafe extern "C" fn plug_deactivate(_plugin: *const clap_plugin_t) {}

pub unsafe extern "C" fn plug_start_processing(_plugin: *const clap_plugin_t) -> bool {
    true
}

pub unsafe extern "C" fn plug_stop_processing(_plugin: *const clap_plugin_t) {}

pub unsafe extern "C" fn plug_reset(plugin: *const clap_plugin_t) {
    let st = &mut *((*plugin).plugin_data as *mut MockPluginState);
    st.lp_state = [0.0; 2];
    st.synth_phase = 0.0;
    st.synth_gate = 0.0;
}

pub unsafe extern "C" fn plug_process(
    plugin: *const clap_plugin_t,
    process: *const clap_process_t,
) -> clap_process_status {
    let st = &mut *((*plugin).plugin_data as *mut MockPluginState);
    if !st._host.is_null() {
        if let Some(get_ext) = (*st._host).get_extension {
            let host_tp = get_ext(st._host, CLAP_EXT_THREAD_POOL.as_ptr() as *const c_char)
                as *const clap_host_thread_pool_t;
            if !host_tp.is_null() {
                if let Some(req_exec) = (*host_tp).request_exec {
                    let _ = req_exec(st._host, 2);
                }
            }
        }
    }
    let proc_ref = &*process;
    let frames = proc_ref.frames_count as usize;

    let in_events = proc_ref.in_events;
    let num_events = if !in_events.is_null() {
        if let Some(sz_fn) = (*in_events).size {
            sz_fn(in_events)
        } else {
            0
        }
    } else {
        0
    };
    let mut ev_idx = 0u32;

    for frame in 0..frames {
        while ev_idx < num_events {
            if let Some(get_fn) = (*in_events).get {
                let hdr_ptr = get_fn(in_events, ev_idx);
                if !hdr_ptr.is_null() && (*hdr_ptr).time <= (frame as u32) {
                    st.apply_event(&*hdr_ptr);
                    ev_idx += 1;
                    continue;
                }
            }
            break;
        }

        let alpha = (1.0 - (-2.0 * PI * st.cutoff_hz / st.sample_rate).exp()).clamp(0.0001, 0.9999);
        let synth_val = if st.synth_gate > 0.0 {
            let v = (2.0 * PI * st.synth_phase).sin() * st.synth_gate * 0.2;
            st.synth_phase = (st.synth_phase + st.synth_freq / st.sample_rate).fract();
            v
        } else {
            0.0
        };

        let out_count = if proc_ref.audio_outputs_count > 0 && !proc_ref.audio_outputs.is_null() {
            (*proc_ref.audio_outputs).channel_count as usize
        } else {
            0
        };
        let in_count = if proc_ref.audio_inputs_count > 0 && !proc_ref.audio_inputs.is_null() {
            (*proc_ref.audio_inputs).channel_count as usize
        } else {
            0
        };

        for ch in 0..out_count {
            let x_in = if ch < in_count && !(*proc_ref.audio_inputs).data32.is_null() {
                let ch_ptr = *(*proc_ref.audio_inputs).data32.add(ch);
                *ch_ptr.add(frame) as f64
            } else {
                0.0
            };

            let x = x_in + synth_val;
            let y = if st.bypass >= 0.5 {
                x
            } else {
                let state_slot = &mut st.lp_state[ch.min(1)];
                *state_slot += alpha * (x - *state_slot);
                // Flush subnormals to zero
                if state_slot.abs() < 1e-30 {
                    *state_slot = 0.0;
                }
                let mut sig = *state_slot * st.gain;
                if st.drive > 1e-4 {
                    let k = 1.0 + st.drive * 4.0;
                    sig = (sig * k).tanh() / k.tanh();
                }
                sig
            };

            if !(*proc_ref.audio_outputs).data32.is_null() {
                let out_ptr = *(*proc_ref.audio_outputs).data32.add(ch);
                *out_ptr.add(frame) = y as f32;
            }
        }
    }

    // Consume any trailing events
    while ev_idx < num_events {
        if let Some(get_fn) = (*in_events).get {
            let hdr_ptr = get_fn(in_events, ev_idx);
            if !hdr_ptr.is_null() {
                st.apply_event(&*hdr_ptr);
            }
        }
        ev_idx += 1;
    }

    CLAP_PROCESS_CONTINUE
}

pub unsafe extern "C" fn plug_on_main_thread(_plugin: *const clap_plugin_t) {}
