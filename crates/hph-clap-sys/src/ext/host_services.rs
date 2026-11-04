use std::ffi::c_char;

use crate::core::{clap_host_t, clap_id, clap_plugin_t, CLAP_NAME_SIZE};

// Latency & Tail
#[repr(C)]
pub struct clap_plugin_latency_t {
    pub get: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> u32>,
}

#[repr(C)]
pub struct clap_host_latency_t {
    pub changed: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}

#[repr(C)]
pub struct clap_plugin_tail_t {
    pub get: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> u32>,
}

#[repr(C)]
pub struct clap_host_tail_t {
    pub changed: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}

// Render
pub type clap_plugin_render_mode = i32;
pub const CLAP_RENDER_REALTIME: clap_plugin_render_mode = 0;
pub const CLAP_RENDER_OFFLINE: clap_plugin_render_mode = 1;

#[repr(C)]
pub struct clap_plugin_render_t {
    pub has_hard_realtime_requirement:
        Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> bool>,
    pub set: Option<
        unsafe extern "C" fn(plugin: *const clap_plugin_t, mode: clap_plugin_render_mode) -> bool,
    >,
}

// Thread check
#[repr(C)]
pub struct clap_host_thread_check_t {
    pub is_main_thread: Option<unsafe extern "C" fn(host: *const clap_host_t) -> bool>,
    pub is_audio_thread: Option<unsafe extern "C" fn(host: *const clap_host_t) -> bool>,
}

// Timer support
#[repr(C)]
pub struct clap_plugin_timer_support_t {
    pub on_timer: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t, timer_id: clap_id)>,
}

#[repr(C)]
pub struct clap_host_timer_support_t {
    pub register_timer: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            period_ms: u32,
            timer_id: *mut clap_id,
        ) -> bool,
    >,
    pub unregister_timer:
        Option<unsafe extern "C" fn(host: *const clap_host_t, timer_id: clap_id) -> bool>,
}

// POSIX FD support
pub type clap_posix_fd_flags_t = u32;
pub const CLAP_POSIX_FD_READ: clap_posix_fd_flags_t = 1 << 0;
pub const CLAP_POSIX_FD_WRITE: clap_posix_fd_flags_t = 1 << 1;
pub const CLAP_POSIX_FD_ERROR: clap_posix_fd_flags_t = 1 << 2;

#[repr(C)]
pub struct clap_plugin_posix_fd_support_t {
    pub on_fd: Option<
        unsafe extern "C" fn(plugin: *const clap_plugin_t, fd: i32, flags: clap_posix_fd_flags_t),
    >,
}

#[repr(C)]
pub struct clap_host_posix_fd_support_t {
    pub register_fd: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            fd: i32,
            flags: clap_posix_fd_flags_t,
        ) -> bool,
    >,
    pub modify_fd: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            fd: i32,
            flags: clap_posix_fd_flags_t,
        ) -> bool,
    >,
    pub unregister_fd: Option<unsafe extern "C" fn(host: *const clap_host_t, fd: i32) -> bool>,
}

// Log
pub type clap_log_severity = i32;
pub const CLAP_LOG_DEBUG: clap_log_severity = 0;
pub const CLAP_LOG_INFO: clap_log_severity = 1;
pub const CLAP_LOG_WARNING: clap_log_severity = 2;
pub const CLAP_LOG_ERROR: clap_log_severity = 3;
pub const CLAP_LOG_FATAL: clap_log_severity = 4;
pub const CLAP_LOG_HOST_MISBEHAVING: clap_log_severity = 5;
pub const CLAP_LOG_PLUGIN_MISBEHAVING: clap_log_severity = 6;

#[repr(C)]
pub struct clap_host_log_t {
    pub log: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            severity: clap_log_severity,
            msg: *const c_char,
        ),
    >,
}

// Voice info
pub const CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES: u64 = 1 << 0;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct clap_voice_info_t {
    pub voice_count: u32,
    pub voice_capacity: u32,
    pub flags: u64,
}

#[repr(C)]
pub struct clap_plugin_voice_info_t {
    pub get: Option<
        unsafe extern "C" fn(plugin: *const clap_plugin_t, info: *mut clap_voice_info_t) -> bool,
    >,
}

#[repr(C)]
pub struct clap_host_voice_info_t {
    pub changed: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}

// Thread Pool
#[repr(C)]
pub struct clap_plugin_thread_pool_t {
    pub exec: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t, task_index: u32)>,
}

#[repr(C)]
pub struct clap_host_thread_pool_t {
    pub request_exec:
        Option<unsafe extern "C" fn(host: *const clap_host_t, num_tasks: u32) -> bool>,
}

// Remote Controls
pub const CLAP_REMOTE_CONTROLS_COUNT: usize = 8;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct clap_remote_controls_page_t {
    pub section_name: [c_char; CLAP_NAME_SIZE],
    pub page_id: clap_id,
    pub page_name: [c_char; CLAP_NAME_SIZE],
    pub param_ids: [clap_id; CLAP_REMOTE_CONTROLS_COUNT],
    pub is_for_preset: bool,
}

#[repr(C)]
pub struct clap_plugin_remote_controls_t {
    pub count: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> u32>,
    pub get: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            page_index: u32,
            page: *mut clap_remote_controls_page_t,
        ) -> bool,
    >,
}

#[repr(C)]
pub struct clap_host_remote_controls_t {
    pub changed: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
    pub suggest_page: Option<unsafe extern "C" fn(host: *const clap_host_t, page_id: clap_id)>,
}

// Track Info & Color
pub const CLAP_TRACK_INFO_HAS_TRACK_NAME: u64 = 1 << 0;
pub const CLAP_TRACK_INFO_HAS_TRACK_COLOR: u64 = 1 << 1;
pub const CLAP_TRACK_INFO_HAS_AUDIO_CHANNEL: u64 = 1 << 2;
pub const CLAP_TRACK_INFO_IS_FOR_RETURN_TRACK: u64 = 1 << 3;
pub const CLAP_TRACK_INFO_IS_FOR_BUS: u64 = 1 << 4;
pub const CLAP_TRACK_INFO_IS_FOR_MASTER: u64 = 1 << 5;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct clap_color_t {
    pub alpha: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct clap_track_info_t {
    pub flags: u64,
    pub name: [c_char; CLAP_NAME_SIZE],
    pub color: clap_color_t,
    pub audio_channel_count: i32,
    pub audio_port_type: *const c_char,
}

#[repr(C)]
pub struct clap_plugin_track_info_t {
    pub changed: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t)>,
}

#[repr(C)]
pub struct clap_host_track_info_t {
    pub get: Option<
        unsafe extern "C" fn(host: *const clap_host_t, info: *mut clap_track_info_t) -> bool,
    >,
}

// Ambisonic
pub const CLAP_AMBISONIC_ORDERING_FUMA: u32 = 0;
pub const CLAP_AMBISONIC_ORDERING_ACN: u32 = 1;
pub const CLAP_AMBISONIC_NORMALIZATION_MAXN: u32 = 0;
pub const CLAP_AMBISONIC_NORMALIZATION_SN3D: u32 = 1;
pub const CLAP_AMBISONIC_NORMALIZATION_N3D: u32 = 2;
pub const CLAP_AMBISONIC_NORMALIZATION_SN2D: u32 = 3;
pub const CLAP_AMBISONIC_NORMALIZATION_N2D: u32 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct clap_ambisonic_config_t {
    pub ordering: u32,
    pub normalization: u32,
}

#[repr(C)]
pub struct clap_plugin_ambisonic_t {
    pub is_config_supported: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            config: *const clap_ambisonic_config_t,
        ) -> bool,
    >,
    pub get_config: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            is_input: bool,
            port_index: u32,
            config: *mut clap_ambisonic_config_t,
        ) -> bool,
    >,
}

#[repr(C)]
pub struct clap_host_ambisonic_t {
    pub changed: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}

// Surround
#[repr(C)]
pub struct clap_plugin_surround_t {
    pub is_channel_mask_supported:
        Option<unsafe extern "C" fn(plugin: *const clap_plugin_t, channel_mask: u64) -> bool>,
    pub get_channel_map: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            is_input: bool,
            port_index: u32,
            channel_map: *mut u8,
            channel_map_capacity: u32,
        ) -> u32,
    >,
}

#[repr(C)]
pub struct clap_host_surround_t {
    pub changed: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}

// Note Name
#[repr(C)]
#[derive(Clone, Copy)]
pub struct clap_note_name_t {
    pub name: [c_char; CLAP_NAME_SIZE],
    pub port: i16,
    pub key: i16,
    pub channel: i16,
}

#[repr(C)]
pub struct clap_plugin_note_name_t {
    pub count: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> u32>,
    pub get: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            index: u32,
            note_name: *mut clap_note_name_t,
        ) -> bool,
    >,
}

#[repr(C)]
pub struct clap_host_note_name_t {
    pub changed: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}
