use std::ffi::{c_char, c_void};

use crate::core::{clap_host_t, clap_id, clap_plugin_t, CLAP_NAME_SIZE, CLAP_PATH_SIZE};
use crate::events::{clap_input_events_t, clap_output_events_t};

// Params
pub const CLAP_PARAM_IS_STEPPED: u32 = 1 << 0;
pub const CLAP_PARAM_IS_PERIODIC: u32 = 1 << 1;
pub const CLAP_PARAM_IS_HIDDEN: u32 = 1 << 2;
pub const CLAP_PARAM_IS_READONLY: u32 = 1 << 3;
pub const CLAP_PARAM_IS_BYPASS: u32 = 1 << 4;
pub const CLAP_PARAM_IS_AUTOMATABLE: u32 = 1 << 5;
pub const CLAP_PARAM_IS_AUTOMATABLE_PER_NOTE_ID: u32 = 1 << 6;
pub const CLAP_PARAM_IS_AUTOMATABLE_PER_KEY: u32 = 1 << 7;
pub const CLAP_PARAM_IS_AUTOMATABLE_PER_CHANNEL: u32 = 1 << 8;
pub const CLAP_PARAM_IS_AUTOMATABLE_PER_PORT: u32 = 1 << 9;
pub const CLAP_PARAM_IS_MODULATABLE: u32 = 1 << 10;
pub const CLAP_PARAM_IS_MODULATABLE_PER_NOTE_ID: u32 = 1 << 11;
pub const CLAP_PARAM_IS_MODULATABLE_PER_KEY: u32 = 1 << 12;
pub const CLAP_PARAM_IS_MODULATABLE_PER_CHANNEL: u32 = 1 << 13;
pub const CLAP_PARAM_IS_MODULATABLE_PER_PORT: u32 = 1 << 14;
pub const CLAP_PARAM_REQUIRES_PROCESS: u32 = 1 << 15;
pub const CLAP_PARAM_IS_ENUM: u32 = 1 << 16;

#[repr(C)]
pub struct clap_param_info_t {
    pub id: clap_id,
    pub flags: u32,
    pub cookie: *mut c_void,
    pub name: [c_char; CLAP_NAME_SIZE],
    pub module: [c_char; CLAP_PATH_SIZE],
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
}

#[repr(C)]
pub struct clap_plugin_params_t {
    pub count: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> u32>,
    pub get_info: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            param_index: u32,
            param_info: *mut clap_param_info_t,
        ) -> bool,
    >,
    pub get_value: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            param_id: clap_id,
            out_value: *mut f64,
        ) -> bool,
    >,
    pub value_to_text: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            param_id: clap_id,
            value: f64,
            out_buffer: *mut c_char,
            out_buffer_capacity: u32,
        ) -> bool,
    >,
    pub text_to_value: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            param_id: clap_id,
            param_value_text: *const c_char,
            out_value: *mut f64,
        ) -> bool,
    >,
    pub flush: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            in_events: *const clap_input_events_t,
            out_events: *const clap_output_events_t,
        ),
    >,
}

#[repr(C)]
pub struct clap_host_params_t {
    pub rescan: Option<unsafe extern "C" fn(host: *const clap_host_t, flags: u32)>,
    pub clear:
        Option<unsafe extern "C" fn(host: *const clap_host_t, param_id: clap_id, flags: u32)>,
    pub request_flush: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}

// Audio ports
pub const CLAP_AUDIO_PORT_IS_MAIN: u32 = 1 << 0;
pub const CLAP_AUDIO_PORT_SUPPORTS_64BITS: u32 = 1 << 1;
pub const CLAP_AUDIO_PORT_PREFERS_64BITS: u32 = 1 << 2;
pub const CLAP_AUDIO_PORT_REQUIRES_COMMON_SAMPLE_SIZE: u32 = 1 << 3;

pub const CLAP_PORT_MONO: &[u8] = b"mono\0";
pub const CLAP_PORT_STEREO: &[u8] = b"stereo\0";

#[repr(C)]
pub struct clap_audio_port_info_t {
    pub id: clap_id,
    pub name: [c_char; CLAP_NAME_SIZE],
    pub flags: u32,
    pub channel_count: u32,
    pub port_type: *const c_char,
    pub in_place_pair: clap_id,
}

#[repr(C)]
pub struct clap_plugin_audio_ports_t {
    pub count: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t, is_input: bool) -> u32>,
    pub get: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            index: u32,
            is_input: bool,
            info: *mut clap_audio_port_info_t,
        ) -> bool,
    >,
}

#[repr(C)]
pub struct clap_host_audio_ports_t {
    pub is_rescan_flag_supported:
        Option<unsafe extern "C" fn(host: *const clap_host_t, flag: u32) -> bool>,
    pub rescan: Option<unsafe extern "C" fn(host: *const clap_host_t, flags: u32)>,
}

// Note ports
pub const CLAP_NOTE_DIALECT_CLAP: u32 = 1 << 0;
pub const CLAP_NOTE_DIALECT_MIDI: u32 = 1 << 1;
pub const CLAP_NOTE_DIALECT_MIDI_MPE: u32 = 1 << 2;
pub const CLAP_NOTE_DIALECT_MIDI2: u32 = 1 << 3;

#[repr(C)]
pub struct clap_note_port_info_t {
    pub id: clap_id,
    pub supported_dialects: u32,
    pub preferred_dialect: u32,
    pub name: [c_char; CLAP_NAME_SIZE],
}

#[repr(C)]
pub struct clap_plugin_note_ports_t {
    pub count: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t, is_input: bool) -> u32>,
    pub get: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            index: u32,
            is_input: bool,
            info: *mut clap_note_port_info_t,
        ) -> bool,
    >,
}

#[repr(C)]
pub struct clap_host_note_ports_t {
    pub supported_dialects: Option<unsafe extern "C" fn(host: *const clap_host_t) -> u32>,
    pub rescan: Option<unsafe extern "C" fn(host: *const clap_host_t, flags: u32)>,
}

// Audio Ports Config
#[repr(C)]
#[derive(Clone, Copy)]
pub struct clap_audio_ports_config_t {
    pub id: clap_id,
    pub name: [c_char; CLAP_NAME_SIZE],
    pub input_port_count: u32,
    pub output_port_count: u32,
    pub has_main_input: bool,
    pub main_input_channel_count: u32,
    pub main_input_port_type: *const c_char,
    pub has_main_output: bool,
    pub main_output_channel_count: u32,
    pub main_output_port_type: *const c_char,
}

#[repr(C)]
pub struct clap_plugin_audio_ports_config_t {
    pub count: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> u32>,
    pub get: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            index: u32,
            config: *mut clap_audio_ports_config_t,
        ) -> bool,
    >,
    pub select:
        Option<unsafe extern "C" fn(plugin: *const clap_plugin_t, config_id: clap_id) -> bool>,
}

#[repr(C)]
pub struct clap_host_audio_ports_config_t {
    pub rescan: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
}

// Audio Ports Activation
#[repr(C)]
pub struct clap_plugin_audio_ports_activation_t {
    pub can_activate_while_processing:
        Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> bool>,
    pub set_active: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            is_input: bool,
            port_index: u32,
            is_active: bool,
            sample_size: u32,
        ) -> bool,
    >,
}

// Configurable Audio Ports
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct clap_audio_port_configuration_request_t {
    pub is_input: bool,
    pub port_index: u32,
    pub channel_count: u32,
    pub port_type: *const c_char,
    pub port_details: *const c_void,
}

#[repr(C)]
pub struct clap_plugin_configurable_audio_ports_t {
    pub can_apply_configuration: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            requests: *const clap_audio_port_configuration_request_t,
            request_count: u32,
        ) -> bool,
    >,
    pub apply_configuration: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            requests: *const clap_audio_port_configuration_request_t,
            request_count: u32,
        ) -> bool,
    >,
}
