use std::ffi::{c_char, c_void};

use crate::core::{clap_host_t, clap_id, clap_plugin_t};
use crate::ext::host_services::clap_color_t;

// GUI
pub const CLAP_WINDOW_API_COCOA: &[u8] = b"cocoa\0";
pub const CLAP_WINDOW_API_WIN32: &[u8] = b"win32\0";
pub const CLAP_WINDOW_API_X11: &[u8] = b"x11\0";
pub const CLAP_WINDOW_API_WAYLAND: &[u8] = b"wayland\0";

#[repr(C)]
#[derive(Clone, Copy)]
pub union clap_window_handle_t {
    pub cocoa: *mut c_void,
    pub win32: *mut c_void,
    pub x11: libc::c_ulong,
    pub ptr: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct clap_window_t {
    pub api: *const c_char,
    pub handle: clap_window_handle_t,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct clap_gui_resize_hints_t {
    pub can_resize_horizontally: bool,
    pub can_resize_vertically: bool,
    pub preserve_aspect_ratio: bool,
    pub aspect_ratio_width: u32,
    pub aspect_ratio_height: u32,
}

#[repr(C)]
pub struct clap_plugin_gui_t {
    pub is_api_supported: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            api: *const c_char,
            is_floating: bool,
        ) -> bool,
    >,
    pub get_preferred_api: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            api: *mut *const c_char,
            is_floating: *mut bool,
        ) -> bool,
    >,
    pub create: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            api: *const c_char,
            is_floating: bool,
        ) -> bool,
    >,
    pub destroy: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t)>,
    pub set_scale: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t, scale: f64) -> bool>,
    pub get_size: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            width: *mut u32,
            height: *mut u32,
        ) -> bool,
    >,
    pub can_resize: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> bool>,
    pub get_resize_hints: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            hints: *mut clap_gui_resize_hints_t,
        ) -> bool,
    >,
    pub adjust_size: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            width: *mut u32,
            height: *mut u32,
        ) -> bool,
    >,
    pub set_size:
        Option<unsafe extern "C" fn(plugin: *const clap_plugin_t, width: u32, height: u32) -> bool>,
    pub set_parent: Option<
        unsafe extern "C" fn(plugin: *const clap_plugin_t, window: *const clap_window_t) -> bool,
    >,
    pub set_transient: Option<
        unsafe extern "C" fn(plugin: *const clap_plugin_t, window: *const clap_window_t) -> bool,
    >,
    pub suggest_title:
        Option<unsafe extern "C" fn(plugin: *const clap_plugin_t, title: *const c_char)>,
    pub show: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> bool>,
    pub hide: Option<unsafe extern "C" fn(plugin: *const clap_plugin_t) -> bool>,
}

#[repr(C)]
pub struct clap_host_gui_t {
    pub resize_hints_changed: Option<unsafe extern "C" fn(host: *const clap_host_t)>,
    pub request_resize:
        Option<unsafe extern "C" fn(host: *const clap_host_t, width: u32, height: u32) -> bool>,
    pub request_show: Option<unsafe extern "C" fn(host: *const clap_host_t) -> bool>,
    pub request_hide: Option<unsafe extern "C" fn(host: *const clap_host_t) -> bool>,
    pub closed: Option<unsafe extern "C" fn(host: *const clap_host_t, was_destroyed: bool)>,
}

// Context Menu
pub const CLAP_CONTEXT_MENU_TARGET_KIND_GLOBAL: u32 = 0;
pub const CLAP_CONTEXT_MENU_TARGET_KIND_PARAM: u32 = 1;

pub type clap_context_menu_item_kind_t = u32;
pub const CLAP_CONTEXT_MENU_ITEM_ENTRY: clap_context_menu_item_kind_t = 0;
pub const CLAP_CONTEXT_MENU_ITEM_CHECK_ENTRY: clap_context_menu_item_kind_t = 1;
pub const CLAP_CONTEXT_MENU_ITEM_SEPARATOR: clap_context_menu_item_kind_t = 2;
pub const CLAP_CONTEXT_MENU_ITEM_BEGIN_SUBMENU: clap_context_menu_item_kind_t = 3;
pub const CLAP_CONTEXT_MENU_ITEM_END_SUBMENU: clap_context_menu_item_kind_t = 4;
pub const CLAP_CONTEXT_MENU_ITEM_TITLE: clap_context_menu_item_kind_t = 5;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct clap_context_menu_target_t {
    pub kind: u32,
    pub id: clap_id,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct clap_context_menu_entry_t {
    pub label: *const c_char,
    pub is_enabled: bool,
    pub action_id: clap_id,
}

#[repr(C)]
pub struct clap_context_menu_builder_t {
    pub ctx: *mut c_void,
    pub add_item: Option<
        unsafe extern "C" fn(
            builder: *const clap_context_menu_builder_t,
            item_kind: clap_context_menu_item_kind_t,
            item_data: *const c_void,
        ) -> bool,
    >,
    pub supports: Option<
        unsafe extern "C" fn(
            builder: *const clap_context_menu_builder_t,
            item_kind: clap_context_menu_item_kind_t,
        ) -> bool,
    >,
}

#[repr(C)]
pub struct clap_plugin_context_menu_t {
    pub populate: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            target: *const clap_context_menu_target_t,
            builder: *const clap_context_menu_builder_t,
        ) -> bool,
    >,
    pub perform: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            target: *const clap_context_menu_target_t,
            action_id: clap_id,
        ) -> bool,
    >,
}

#[repr(C)]
pub struct clap_host_context_menu_t {
    pub populate: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            target: *const clap_context_menu_target_t,
            builder: *const clap_context_menu_builder_t,
        ) -> bool,
    >,
    pub perform: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            target: *const clap_context_menu_target_t,
            action_id: clap_id,
        ) -> bool,
    >,
    pub can_popup: Option<unsafe extern "C" fn(host: *const clap_host_t) -> bool>,
    pub popup: Option<
        unsafe extern "C" fn(
            host: *const clap_host_t,
            target: *const clap_context_menu_target_t,
            screen_index: i32,
            x: i32,
            y: i32,
        ) -> bool,
    >,
}

// Param Indication
pub const CLAP_PARAM_INDICATION_AUTOMATION_NONE: u32 = 0;
pub const CLAP_PARAM_INDICATION_AUTOMATION_PRESENT: u32 = 1;
pub const CLAP_PARAM_INDICATION_AUTOMATION_PLAYING: u32 = 2;
pub const CLAP_PARAM_INDICATION_AUTOMATION_RECORDING: u32 = 3;
pub const CLAP_PARAM_INDICATION_AUTOMATION_OVERRIDING: u32 = 4;

#[repr(C)]
pub struct clap_plugin_param_indication_t {
    pub set_mapping: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            param_id: clap_id,
            has_mapping: bool,
            color: *const clap_color_t,
            label: *const c_char,
            description: *const c_char,
        ),
    >,
    pub set_automation: Option<
        unsafe extern "C" fn(
            plugin: *const clap_plugin_t,
            param_id: clap_id,
            automation_state: u32,
            color: *const clap_color_t,
        ),
    >,
}
