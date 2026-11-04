use std::ffi::{c_char, c_void};

use crate::core::{clap_host_t, clap_plugin_t};

// Streams & State
#[repr(C)]
pub struct clap_istream_t {
    pub ctx: *mut c_void,
    pub read: Option<
        unsafe extern "C" fn(stream: *const clap_istream_t, buffer: *mut c_void, size: u64) -> i64,
    >,
}

#[repr(C)]
pub struct clap_ostream_t {
    pub ctx: *mut c_void,
    pub write: Option<
        unsafe extern "C" fn(
            stream: *const clap_ostream_t,
            buffer: *const c_void,
            size: u64,
        ) -> i64,
    >,
}

#[repr(C)]
pub struct clap_plugin_state_t {
    pub save: Option<
        unsafe extern "C" fn(plugin: *const clap_plugin_t, stream: *const clap_ostream_t) -> bool,
    >,
    pub load: Option<
        unsafe extern "C" fn(plugin: *const clap_plugin_t, stream: *const clap_istream_t) -> bool,
    >,
}

#[repr(C)]
pub struct clap_host_state_t {
    pub mark_dirty: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}

// State Context
pub type clap_plugin_state_context_type = u32;
pub const CLAP_STATE_CONTEXT_FOR_PRESET: clap_plugin_state_context_type = 1;
pub const CLAP_STATE_CONTEXT_FOR_DUPLICATE: clap_plugin_state_context_type = 2;
pub const CLAP_STATE_CONTEXT_FOR_PROJECT: clap_plugin_state_context_type = 3;

#[repr(C)]
pub struct clap_plugin_state_context_t {
    pub save: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            stream: *const clap_ostream_t,
            context_type: u32,
        ) -> bool,
    >,
    pub load: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            stream: *const clap_istream_t,
            context_type: u32,
        ) -> bool,
    >,
}

// Preset Load
pub const CLAP_PRESET_DISCOVERY_LOCATION_FILE: u32 = 0;
pub const CLAP_PRESET_DISCOVERY_LOCATION_PLUGIN: u32 = 1;

#[repr(C)]
pub struct clap_plugin_preset_load_t {
    pub from_location: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            location_kind: u32,
            location: *const c_char,
            load_key: *const c_char,
        ) -> bool,
    >,
}

#[repr(C)]
pub struct clap_host_preset_load_t {
    pub on_error: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            location_kind: u32,
            location: *const c_char,
            load_key: *const c_char,
            os_error: i32,
            msg: *const c_char,
        ),
    >,
    pub loaded: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            location_kind: u32,
            location: *const c_char,
            load_key: *const c_char,
        ),
    >,
}
