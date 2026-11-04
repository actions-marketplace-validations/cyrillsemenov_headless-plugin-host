use std::ffi::{c_char, c_void};

use crate::events::{clap_event_transport_t, clap_input_events_t, clap_output_events_t};

pub type clap_id = u32;
pub const CLAP_INVALID_ID: clap_id = u32::MAX;

pub type clap_beattime = i64;
pub type clap_sectime = i64;
pub const CLAP_BEATTIME_FACTOR: i64 = 1i64 << 31;
pub const CLAP_SECTIME_FACTOR: i64 = 1i64 << 31;

pub const CLAP_NAME_SIZE: usize = 256;
pub const CLAP_PATH_SIZE: usize = 1024;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct clap_version_t {
    pub major: u32,
    pub minor: u32,
    pub revision: u32,
}

pub const CLAP_VERSION: clap_version_t = clap_version_t {
    major: 1,
    minor: 2,
    revision: 10,
};

#[inline]
pub const fn clap_version_is_compatible(v: clap_version_t) -> bool {
    v.major >= 1
}

#[repr(C)]
pub struct clap_plugin_entry_t {
    pub clap_version: clap_version_t,
    pub init: Option<unsafe extern "C" fn(plugin_path: *const c_char) -> bool>,
    pub deinit: Option<unsafe extern "C" fn()>,
    pub get_factory: Option<unsafe extern "C" fn(factory_id: *const c_char) -> *const c_void>,
}

pub const CLAP_PLUGIN_FACTORY_ID: &[u8] = b"clap.plugin-factory\0";

#[repr(C)]
pub struct clap_plugin_descriptor_t {
    pub clap_version: clap_version_t,
    pub id: *const c_char,
    pub name: *const c_char,
    pub vendor: *const c_char,
    pub url: *const c_char,
    pub manual_url: *const c_char,
    pub support_url: *const c_char,
    pub version: *const c_char,
    pub description: *const c_char,
    pub features: *const *const c_char,
}

#[repr(C)]
pub struct clap_host_t {
    pub clap_version: clap_version_t,
    pub host_data: *mut c_void,
    pub name: *const c_char,
    pub vendor: *const c_char,
    pub url: *const c_char,
    pub version: *const c_char,
    pub get_extension: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            extension_id: *const c_char,
        ) -> *const c_void,
    >,
    pub request_restart: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
    pub request_process: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
    pub request_callback: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}

#[repr(C)]
pub struct clap_plugin_t {
    pub desc: *const clap_plugin_descriptor_t,
    pub plugin_data: *mut c_void,
    pub init: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> bool>,
    pub destroy: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t)>,
    pub activate: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            sample_rate: f64,
            min_frames_count: u32,
            max_frames_count: u32,
        ) -> bool,
    >,
    pub deactivate: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t)>,
    pub start_processing: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> bool>,
    pub stop_processing: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t)>,
    pub reset: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t)>,
    pub process: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            process: *const clap_process_t,
        ) -> clap_process_status,
    >,
    pub get_extension: Option<
        unsafe extern "C" fn(plugin: *const clap_plugin_t, id: *const c_char) -> *const c_void,
    >,
    pub on_main_thread: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t)>,
}

#[repr(C)]
pub struct clap_plugin_factory_t {
    pub get_plugin_count:
        Option<unsafe extern "C" fn(factory: *const clap_plugin_factory_t) -> u32>,
    pub get_plugin_descriptor: Option<
        unsafe extern "C" fn(
            factory: *const clap_plugin_factory_t,
            index: u32,
        ) -> *const clap_plugin_descriptor_t,
    >,
    pub create_plugin: Option<
        unsafe extern "C" fn(
            factory: *const clap_plugin_factory_t,
            host: *const clap_host_t,
            plugin_id: *const c_char,
        ) -> *const clap_plugin_t,
    >,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct clap_audio_buffer_t {
    pub data32: *mut *mut f32,
    pub data64: *mut *mut f64,
    pub channel_count: u32,
    pub latency: u32,
    pub constant_mask: u64,
}

pub type clap_process_status = i32;
pub const CLAP_PROCESS_ERROR: clap_process_status = 0;
pub const CLAP_PROCESS_CONTINUE: clap_process_status = 1;
pub const CLAP_PROCESS_CONTINUE_IF_NOT_QUIET: clap_process_status = 2;
pub const CLAP_PROCESS_TAIL: clap_process_status = 3;
pub const CLAP_PROCESS_SLEEP: clap_process_status = 4;

#[repr(C)]
pub struct clap_process_t {
    pub steady_time: i64,
    pub frames_count: u32,
    pub transport: *const clap_event_transport_t,
    pub audio_inputs: *const clap_audio_buffer_t,
    pub audio_outputs: *mut clap_audio_buffer_t,
    pub audio_inputs_count: u32,
    pub audio_outputs_count: u32,
    pub in_events: *const clap_input_events_t,
    pub out_events: *const clap_output_events_t,
}
