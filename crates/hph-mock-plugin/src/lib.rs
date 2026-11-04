#![allow(clippy::missing_safety_doc)]

pub mod extensions;
pub mod gui;
pub mod state;

use extensions::plug_get_extension;
use hph_clap_sys::*;
use state::{
    plug_activate, plug_deactivate, plug_destroy, plug_init, plug_on_main_thread, plug_process,
    plug_reset, plug_start_processing, plug_stop_processing, MockPluginState, MOCK_DESCRIPTOR,
};
use std::ffi::{c_char, c_void, CStr};
use std::ptr;

unsafe extern "C" fn factory_get_plugin_count(_factory: *const clap_plugin_factory_t) -> u32 {
    1
}

unsafe extern "C" fn factory_get_plugin_descriptor(
    _factory: *const clap_plugin_factory_t,
    index: u32,
) -> *const clap_plugin_descriptor_t {
    if index == 0 {
        &MOCK_DESCRIPTOR.0
    } else {
        ptr::null()
    }
}

unsafe extern "C" fn factory_create_plugin(
    _factory: *const clap_plugin_factory_t,
    host: *const clap_host_t,
    plugin_id: *const c_char,
) -> *const clap_plugin_t {
    if host.is_null() || plugin_id.is_null() {
        return ptr::null();
    }
    let req_id = CStr::from_ptr(plugin_id).to_bytes();
    let our_id = CStr::from_ptr(MOCK_DESCRIPTOR.0.id).to_bytes();
    if req_id != our_id {
        return ptr::null();
    }

    let state = Box::new(MockPluginState::new(host));
    let plug = Box::new(clap_plugin_t {
        desc: &MOCK_DESCRIPTOR.0,
        plugin_data: Box::into_raw(state) as *mut c_void,
        init: Some(plug_init),
        destroy: Some(plug_destroy),
        activate: Some(plug_activate),
        deactivate: Some(plug_deactivate),
        start_processing: Some(plug_start_processing),
        stop_processing: Some(plug_stop_processing),
        reset: Some(plug_reset),
        process: Some(plug_process),
        get_extension: Some(plug_get_extension),
        on_main_thread: Some(plug_on_main_thread),
    });
    Box::into_raw(plug)
}

static MOCK_FACTORY: clap_plugin_factory_t = clap_plugin_factory_t {
    get_plugin_count: Some(factory_get_plugin_count),
    get_plugin_descriptor: Some(factory_get_plugin_descriptor),
    create_plugin: Some(factory_create_plugin),
};

unsafe extern "C" fn entry_init(_plugin_path: *const c_char) -> bool {
    true
}

unsafe extern "C" fn entry_deinit() {}

unsafe extern "C" fn entry_get_factory(factory_id: *const c_char) -> *const c_void {
    if factory_id.is_null() {
        return ptr::null();
    }
    if CStr::from_ptr(factory_id).to_bytes_with_nul() == CLAP_PLUGIN_FACTORY_ID {
        &MOCK_FACTORY as *const _ as *const c_void
    } else {
        ptr::null()
    }
}

#[no_mangle]
pub static clap_entry: clap_plugin_entry_t = clap_plugin_entry_t {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(entry_get_factory),
};
