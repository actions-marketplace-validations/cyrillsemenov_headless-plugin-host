use crate::gui::EXT_GUI;
use crate::state::{
    copy_str_to_fixed, plug_reset, MockPluginState, PARAM_BYPASS, PARAM_CUTOFF, PARAM_DRIVE,
    PARAM_GAIN,
};
use hph_clap_sys::*;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;

unsafe extern "C" fn params_count(_plugin: *const clap_plugin_t) -> u32 {
    4
}

unsafe extern "C" fn params_get_info(
    _plugin: *const clap_plugin_t,
    param_index: u32,
    param_info: *mut clap_param_info_t,
) -> bool {
    if param_info.is_null() {
        return false;
    }
    let info = &mut *param_info;
    *info = std::mem::zeroed();
    match param_index {
        0 => {
            info.id = PARAM_GAIN;
            info.flags = CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_MODULATABLE;
            copy_str_to_fixed(&mut info.name, "Output Gain");
            copy_str_to_fixed(&mut info.module, "Output");
            info.min_value = 0.0;
            info.max_value = 2.0;
            info.default_value = 1.0;
            true
        }
        1 => {
            info.id = PARAM_CUTOFF;
            info.flags = CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_MODULATABLE;
            copy_str_to_fixed(&mut info.name, "Filter Cutoff");
            copy_str_to_fixed(&mut info.module, "Filter");
            info.min_value = 20.0;
            info.max_value = 20000.0;
            info.default_value = 4000.0;
            true
        }
        2 => {
            info.id = PARAM_DRIVE;
            info.flags = CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_MODULATABLE;
            copy_str_to_fixed(&mut info.name, "Saturation Drive");
            copy_str_to_fixed(&mut info.module, "Distortion");
            info.min_value = 0.0;
            info.max_value = 1.0;
            info.default_value = 0.0;
            true
        }
        3 => {
            info.id = PARAM_BYPASS;
            info.flags = CLAP_PARAM_IS_AUTOMATABLE | CLAP_PARAM_IS_STEPPED | CLAP_PARAM_IS_BYPASS;
            copy_str_to_fixed(&mut info.name, "Bypass");
            copy_str_to_fixed(&mut info.module, "Global");
            info.min_value = 0.0;
            info.max_value = 1.0;
            info.default_value = 0.0;
            true
        }
        _ => false,
    }
}

unsafe extern "C" fn params_get_value(
    plugin: *const clap_plugin_t,
    param_id: clap_id,
    out_value: *mut f64,
) -> bool {
    let st = &*((*plugin).plugin_data as *const MockPluginState);
    match param_id {
        PARAM_GAIN => {
            *out_value = st.gain;
            true
        }
        PARAM_CUTOFF => {
            *out_value = st.cutoff_hz;
            true
        }
        PARAM_DRIVE => {
            *out_value = st.drive;
            true
        }
        PARAM_BYPASS => {
            *out_value = st.bypass;
            true
        }
        _ => false,
    }
}

unsafe extern "C" fn params_value_to_text(
    _plugin: *const clap_plugin_t,
    param_id: clap_id,
    value: f64,
    out_buffer: *mut c_char,
    out_buffer_capacity: u32,
) -> bool {
    let s = match param_id {
        PARAM_GAIN => format!("{:.2}", value),
        PARAM_CUTOFF => format!("{:.1} Hz", value),
        PARAM_DRIVE => format!("{:.1} %", value * 100.0),
        PARAM_BYPASS => {
            if value >= 0.5 {
                "Bypassed".to_string()
            } else {
                "Active".to_string()
            }
        }
        _ => return false,
    };
    let Ok(cstr) = CString::new(s) else {
        return false;
    };
    let bytes = cstr.as_bytes_with_nul();
    let cap = out_buffer_capacity as usize;
    if cap == 0 {
        return false;
    }
    let copy_len = bytes.len().min(cap - 1);
    ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, out_buffer, copy_len);
    *out_buffer.add(copy_len) = 0;
    true
}

unsafe extern "C" fn params_text_to_value(
    _plugin: *const clap_plugin_t,
    param_id: clap_id,
    param_value_text: *const c_char,
    out_value: *mut f64,
) -> bool {
    if param_value_text.is_null() || out_value.is_null() {
        return false;
    }
    let s = CStr::from_ptr(param_value_text).to_string_lossy();
    let token = s.split_whitespace().next().unwrap_or("");
    if param_id == PARAM_BYPASS {
        *out_value = if s.eq_ignore_ascii_case("bypassed") || token == "1" {
            1.0
        } else {
            0.0
        };
        return true;
    }
    if let Ok(v) = token.parse::<f64>() {
        *out_value = if param_id == PARAM_DRIVE && s.contains('%') {
            v * 0.01
        } else {
            v
        };
        true
    } else {
        false
    }
}

unsafe extern "C" fn params_flush(
    plugin: *const clap_plugin_t,
    in_events: *const clap_input_events_t,
    _out_events: *const clap_output_events_t,
) {
    let st = &mut *((*plugin).plugin_data as *mut MockPluginState);
    if !in_events.is_null() {
        if let (Some(sz_fn), Some(get_fn)) = ((*in_events).size, (*in_events).get) {
            let n = sz_fn(in_events);
            for i in 0..n {
                let hdr = get_fn(in_events, i);
                if !hdr.is_null() {
                    st.apply_event(&*hdr);
                }
            }
        }
    }
}

pub static EXT_PARAMS: clap_plugin_params_t = clap_plugin_params_t {
    count: Some(params_count),
    get_info: Some(params_get_info),
    get_value: Some(params_get_value),
    value_to_text: Some(params_value_to_text),
    text_to_value: Some(params_text_to_value),
    flush: Some(params_flush),
};

unsafe extern "C" fn audio_ports_count(_plugin: *const clap_plugin_t, _is_input: bool) -> u32 {
    1
}

unsafe extern "C" fn audio_ports_get(
    _plugin: *const clap_plugin_t,
    index: u32,
    is_input: bool,
    info: *mut clap_audio_port_info_t,
) -> bool {
    if index != 0 || info.is_null() {
        return false;
    }
    let p = &mut *info;
    *p = std::mem::zeroed();
    p.id = 0;
    copy_str_to_fixed(
        &mut p.name,
        if is_input { "Stereo In" } else { "Stereo Out" },
    );
    p.flags = CLAP_AUDIO_PORT_IS_MAIN;
    p.channel_count = 2;
    p.port_type = CLAP_PORT_STEREO.as_ptr() as *const c_char;
    p.in_place_pair = 0;
    true
}

pub static EXT_AUDIO_PORTS: clap_plugin_audio_ports_t = clap_plugin_audio_ports_t {
    count: Some(audio_ports_count),
    get: Some(audio_ports_get),
};

unsafe extern "C" fn note_ports_count(_plugin: *const clap_plugin_t, is_input: bool) -> u32 {
    if is_input {
        1
    } else {
        0
    }
}

unsafe extern "C" fn note_ports_get(
    _plugin: *const clap_plugin_t,
    index: u32,
    is_input: bool,
    info: *mut clap_note_port_info_t,
) -> bool {
    if !is_input || index != 0 || info.is_null() {
        return false;
    }
    let p = &mut *info;
    *p = std::mem::zeroed();
    p.id = 0;
    p.supported_dialects = CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI;
    p.preferred_dialect = CLAP_NOTE_DIALECT_CLAP;
    copy_str_to_fixed(&mut p.name, "Notes In");
    true
}

pub static EXT_NOTE_PORTS: clap_plugin_note_ports_t = clap_plugin_note_ports_t {
    count: Some(note_ports_count),
    get: Some(note_ports_get),
};

unsafe extern "C" fn state_save(
    plugin: *const clap_plugin_t,
    stream: *const clap_ostream_t,
) -> bool {
    let st = &*((*plugin).plugin_data as *const MockPluginState);
    let mut bytes = [0u8; 32];
    bytes[0..8].copy_from_slice(&st.gain.to_le_bytes());
    bytes[8..16].copy_from_slice(&st.cutoff_hz.to_le_bytes());
    bytes[16..24].copy_from_slice(&st.drive.to_le_bytes());
    bytes[24..32].copy_from_slice(&st.bypass.to_le_bytes());

    if let Some(write_fn) = (*stream).write {
        write_fn(stream, bytes.as_ptr() as *const c_void, 32) == 32
    } else {
        false
    }
}

unsafe extern "C" fn state_load(
    plugin: *const clap_plugin_t,
    stream: *const clap_istream_t,
) -> bool {
    let st = &mut *((*plugin).plugin_data as *mut MockPluginState);
    let mut bytes = [0u8; 32];
    if let Some(read_fn) = (*stream).read {
        if read_fn(stream, bytes.as_mut_ptr() as *mut c_void, 32) == 32 {
            let (Ok(gain_b), Ok(cutoff_b), Ok(drive_b), Ok(bypass_b)) = (
                bytes[0..8].try_into(),
                bytes[8..16].try_into(),
                bytes[16..24].try_into(),
                bytes[24..32].try_into(),
            ) else {
                return false;
            };
            st.gain = f64::from_le_bytes(gain_b);
            st.cutoff_hz = f64::from_le_bytes(cutoff_b);
            st.drive = f64::from_le_bytes(drive_b);
            st.bypass = f64::from_le_bytes(bypass_b);
            return true;
        }
    }
    false
}

pub static EXT_STATE: clap_plugin_state_t = clap_plugin_state_t {
    save: Some(state_save),
    load: Some(state_load),
};

unsafe extern "C" fn latency_get(_plugin: *const clap_plugin_t) -> u32 {
    0
}

pub static EXT_LATENCY: clap_plugin_latency_t = clap_plugin_latency_t {
    get: Some(latency_get),
};

unsafe extern "C" fn tail_get(_plugin: *const clap_plugin_t) -> u32 {
    64
}

pub static EXT_TAIL: clap_plugin_tail_t = clap_plugin_tail_t {
    get: Some(tail_get),
};

unsafe extern "C" fn posix_fd_on_fd(
    plugin: *const clap_plugin_t,
    _fd: i32,
    _flags: clap_posix_fd_flags_t,
) {
    #[cfg(target_os = "linux")]
    {
        let st = &*((*plugin).plugin_data as *const MockPluginState);
        if let Some(ref x11_st) = st.x11_state {
            x11_st.on_fd(st.gui_width, st.gui_height);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = plugin;
}

pub static EXT_POSIX_FD_SUPPORT: clap_plugin_posix_fd_support_t = clap_plugin_posix_fd_support_t {
    on_fd: Some(posix_fd_on_fd),
};

unsafe extern "C" fn state_context_save(
    plugin: *const clap_plugin_t,
    stream: *const clap_ostream_t,
    _context_type: u32,
) -> bool {
    state_save(plugin, stream)
}

unsafe extern "C" fn state_context_load(
    plugin: *const clap_plugin_t,
    stream: *const clap_istream_t,
    _context_type: u32,
) -> bool {
    state_load(plugin, stream)
}

pub static EXT_STATE_CONTEXT: clap_plugin_state_context_t = clap_plugin_state_context_t {
    save: Some(state_context_save),
    load: Some(state_context_load),
};

unsafe extern "C" fn render_has_hard_rt(_plugin: *const clap_plugin_t) -> bool {
    false
}

unsafe extern "C" fn render_set(
    _plugin: *const clap_plugin_t,
    _mode: clap_plugin_render_mode,
) -> bool {
    true
}

pub static EXT_RENDER: clap_plugin_render_t = clap_plugin_render_t {
    has_hard_realtime_requirement: Some(render_has_hard_rt),
    set: Some(render_set),
};

unsafe extern "C" fn voice_info_get(
    _plugin: *const clap_plugin_t,
    info: *mut clap_voice_info_t,
) -> bool {
    if info.is_null() {
        return false;
    }
    *info = clap_voice_info_t {
        voice_count: 1,
        voice_capacity: 8,
        flags: CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES,
    };
    true
}

pub static EXT_VOICE_INFO: clap_plugin_voice_info_t = clap_plugin_voice_info_t {
    get: Some(voice_info_get),
};

unsafe extern "C" fn remote_controls_count(_plugin: *const clap_plugin_t) -> u32 {
    1
}

unsafe extern "C" fn remote_controls_get(
    _plugin: *const clap_plugin_t,
    page_index: u32,
    page: *mut clap_remote_controls_page_t,
) -> bool {
    if page_index != 0 || page.is_null() {
        return false;
    }
    let out = &mut *page;
    *out = std::mem::zeroed();
    copy_str_to_fixed(&mut out.section_name, "Main");
    out.page_id = 1;
    copy_str_to_fixed(&mut out.page_name, "Filter & Saturator");
    out.param_ids = [
        PARAM_GAIN,
        PARAM_CUTOFF,
        PARAM_DRIVE,
        PARAM_BYPASS,
        CLAP_INVALID_ID,
        CLAP_INVALID_ID,
        CLAP_INVALID_ID,
        CLAP_INVALID_ID,
    ];
    out.is_for_preset = false;
    true
}

pub static EXT_REMOTE_CONTROLS: clap_plugin_remote_controls_t = clap_plugin_remote_controls_t {
    count: Some(remote_controls_count),
    get: Some(remote_controls_get),
};

unsafe extern "C" fn track_info_changed(plugin: *const clap_plugin_t) {
    let st = &*((*plugin).plugin_data as *const MockPluginState);
    if !st._host.is_null() {
        if let Some(get_ext) = (*st._host).get_extension {
            let host_ti = get_ext(st._host, CLAP_EXT_TRACK_INFO.as_ptr() as *const c_char)
                as *const clap_host_track_info_t;
            if !host_ti.is_null() {
                if let Some(get_fn) = (*host_ti).get {
                    let mut info: clap_track_info_t = std::mem::zeroed();
                    let _ = get_fn(st._host, &mut info);
                }
            }
        }
    }
}

pub static EXT_TRACK_INFO: clap_plugin_track_info_t = clap_plugin_track_info_t {
    changed: Some(track_info_changed),
};

unsafe extern "C" fn note_name_count(_plugin: *const clap_plugin_t) -> u32 {
    2
}

unsafe extern "C" fn note_name_get(
    _plugin: *const clap_plugin_t,
    index: u32,
    note_name: *mut clap_note_name_t,
) -> bool {
    if note_name.is_null() || index >= 2 {
        return false;
    }
    let out = &mut *note_name;
    *out = std::mem::zeroed();
    out.port = -1;
    out.channel = -1;
    if index == 0 {
        out.key = 60;
        copy_str_to_fixed(&mut out.name, "Root C4 Ping");
    } else {
        out.key = 69;
        copy_str_to_fixed(&mut out.name, "Concert A4 Reference");
    }
    true
}

pub static EXT_NOTE_NAME: clap_plugin_note_name_t = clap_plugin_note_name_t {
    count: Some(note_name_count),
    get: Some(note_name_get),
};

unsafe extern "C" fn audio_ports_config_count(_plugin: *const clap_plugin_t) -> u32 {
    1
}

unsafe extern "C" fn audio_ports_config_get(
    _plugin: *const clap_plugin_t,
    index: u32,
    config: *mut clap_audio_ports_config_t,
) -> bool {
    if index != 0 || config.is_null() {
        return false;
    }
    let out = &mut *config;
    *out = std::mem::zeroed();
    out.id = 1;
    copy_str_to_fixed(&mut out.name, "Stereo Main");
    out.input_port_count = 1;
    out.output_port_count = 1;
    out.has_main_input = true;
    out.main_input_channel_count = 2;
    out.main_input_port_type = CLAP_PORT_STEREO.as_ptr() as *const c_char;
    out.has_main_output = true;
    out.main_output_channel_count = 2;
    out.main_output_port_type = CLAP_PORT_STEREO.as_ptr() as *const c_char;
    true
}

unsafe extern "C" fn audio_ports_config_select(
    _plugin: *const clap_plugin_t,
    config_id: clap_id,
) -> bool {
    config_id == 1
}

pub static EXT_AUDIO_PORTS_CONFIG: clap_plugin_audio_ports_config_t =
    clap_plugin_audio_ports_config_t {
        count: Some(audio_ports_config_count),
        get: Some(audio_ports_config_get),
        select: Some(audio_ports_config_select),
    };

unsafe extern "C" fn param_indication_set_mapping(
    _plugin: *const clap_plugin_t,
    _param_id: clap_id,
    _has_mapping: bool,
    _color: *const clap_color_t,
    _label: *const c_char,
    _description: *const c_char,
) {
}

unsafe extern "C" fn param_indication_set_automation(
    _plugin: *const clap_plugin_t,
    _param_id: clap_id,
    _automation_state: u32,
    _color: *const clap_color_t,
) {
}

pub static EXT_PARAM_INDICATION: clap_plugin_param_indication_t = clap_plugin_param_indication_t {
    set_mapping: Some(param_indication_set_mapping),
    set_automation: Some(param_indication_set_automation),
};

unsafe extern "C" fn context_menu_populate(
    _plugin: *const clap_plugin_t,
    _target: *const clap_context_menu_target_t,
    builder: *const clap_context_menu_builder_t,
) -> bool {
    if builder.is_null() {
        return false;
    }
    if let Some(add_fn) = (*builder).add_item {
        let entry = clap_context_menu_entry_t {
            label: c"Reset Filter State".as_ptr(),
            is_enabled: true,
            action_id: 1,
        };
        let _ = add_fn(
            builder,
            CLAP_CONTEXT_MENU_ITEM_ENTRY,
            &entry as *const _ as *const c_void,
        );
    }
    true
}

unsafe extern "C" fn context_menu_perform(
    plugin: *const clap_plugin_t,
    _target: *const clap_context_menu_target_t,
    action_id: clap_id,
) -> bool {
    if action_id == 1 {
        plug_reset(plugin);
        true
    } else {
        false
    }
}

pub static EXT_CONTEXT_MENU: clap_plugin_context_menu_t = clap_plugin_context_menu_t {
    populate: Some(context_menu_populate),
    perform: Some(context_menu_perform),
};

unsafe extern "C" fn thread_pool_exec(_plugin: *const clap_plugin_t, _task_index: u32) {}

pub static EXT_THREAD_POOL: clap_plugin_thread_pool_t = clap_plugin_thread_pool_t {
    exec: Some(thread_pool_exec),
};

pub unsafe extern "C" fn plug_get_extension(
    _plugin: *const clap_plugin_t,
    id: *const c_char,
) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    let bytes = CStr::from_ptr(id).to_bytes_with_nul();
    if bytes == CLAP_EXT_PARAMS {
        &EXT_PARAMS as *const _ as *const c_void
    } else if bytes == CLAP_EXT_AUDIO_PORTS {
        &EXT_AUDIO_PORTS as *const _ as *const c_void
    } else if bytes == CLAP_EXT_NOTE_PORTS {
        &EXT_NOTE_PORTS as *const _ as *const c_void
    } else if bytes == CLAP_EXT_STATE {
        &EXT_STATE as *const _ as *const c_void
    } else if bytes == CLAP_EXT_STATE_CONTEXT {
        &EXT_STATE_CONTEXT as *const _ as *const c_void
    } else if bytes == CLAP_EXT_LATENCY {
        &EXT_LATENCY as *const _ as *const c_void
    } else if bytes == CLAP_EXT_TAIL {
        &EXT_TAIL as *const _ as *const c_void
    } else if bytes == CLAP_EXT_RENDER {
        &EXT_RENDER as *const _ as *const c_void
    } else if bytes == CLAP_EXT_VOICE_INFO {
        &EXT_VOICE_INFO as *const _ as *const c_void
    } else if bytes == CLAP_EXT_REMOTE_CONTROLS {
        &EXT_REMOTE_CONTROLS as *const _ as *const c_void
    } else if bytes == CLAP_EXT_TRACK_INFO {
        &EXT_TRACK_INFO as *const _ as *const c_void
    } else if bytes == CLAP_EXT_NOTE_NAME {
        &EXT_NOTE_NAME as *const _ as *const c_void
    } else if bytes == CLAP_EXT_AUDIO_PORTS_CONFIG {
        &EXT_AUDIO_PORTS_CONFIG as *const _ as *const c_void
    } else if bytes == CLAP_EXT_PARAM_INDICATION {
        &EXT_PARAM_INDICATION as *const _ as *const c_void
    } else if bytes == CLAP_EXT_CONTEXT_MENU {
        &EXT_CONTEXT_MENU as *const _ as *const c_void
    } else if bytes == CLAP_EXT_THREAD_POOL {
        &EXT_THREAD_POOL as *const _ as *const c_void
    } else if bytes == CLAP_EXT_GUI {
        &EXT_GUI as *const _ as *const c_void
    } else if bytes == CLAP_EXT_POSIX_FD_SUPPORT {
        &EXT_POSIX_FD_SUPPORT as *const _ as *const c_void
    } else {
        ptr::null()
    }
}
