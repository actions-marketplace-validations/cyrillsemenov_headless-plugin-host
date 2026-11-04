#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

use crate::state::MockPluginState;
use hph_clap_sys::*;
use std::ffi::{c_char, CStr};
#[cfg(target_os = "windows")]
use std::ptr;

pub unsafe extern "C" fn gui_is_api_supported(
    _plugin: *const clap_plugin_t,
    api: *const c_char,
    is_floating: bool,
) -> bool {
    if is_floating || api.is_null() {
        return false;
    }
    #[cfg(target_os = "windows")]
    return CStr::from_ptr(api).to_bytes_with_nul() == CLAP_WINDOW_API_WIN32;
    #[cfg(target_os = "linux")]
    return CStr::from_ptr(api).to_bytes_with_nul() == CLAP_WINDOW_API_X11;
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    return CStr::from_ptr(api).to_bytes_with_nul() == CLAP_WINDOW_API_COCOA;
}

pub unsafe extern "C" fn gui_create(
    plugin: *const clap_plugin_t,
    api: *const c_char,
    is_floating: bool,
) -> bool {
    if !gui_is_api_supported(plugin, api, is_floating) {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        let st = &mut *((*plugin).plugin_data as *mut MockPluginState);
        st.child_view = macos::create_mock_cocoa_view(st.gui_width, st.gui_height);
        st.child_view.is_some()
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let _ = plugin;
        true
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = plugin;
        false
    }
}

pub unsafe extern "C" fn gui_destroy(plugin: *const clap_plugin_t) {
    let st = &mut *((*plugin).plugin_data as *mut MockPluginState);
    #[cfg(target_os = "macos")]
    macos::destroy_mock_cocoa_view(&mut st.child_view);
    #[cfg(target_os = "windows")]
    {
        if !st.child_hwnd.is_null() {
            let _ = windows::DestroyWindow(st.child_hwnd);
            st.child_hwnd = ptr::null_mut();
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(mut x11_st) = st.x11_state.take() {
            x11_st.destroy(st._host);
        }
    }
}

pub unsafe extern "C" fn gui_set_scale(_plugin: *const clap_plugin_t, _scale: f64) -> bool {
    false
}

pub unsafe extern "C" fn gui_get_size(
    plugin: *const clap_plugin_t,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    let st = &*((*plugin).plugin_data as *const MockPluginState);
    *width = st.gui_width;
    *height = st.gui_height;
    true
}

pub unsafe extern "C" fn gui_can_resize(_plugin: *const clap_plugin_t) -> bool {
    true
}

pub unsafe extern "C" fn gui_adjust_size(
    _plugin: *const clap_plugin_t,
    width: *mut u32,
    height: *mut u32,
) -> bool {
    *width = (*width).clamp(240, 1920);
    *height = (*height).clamp(160, 1080);
    true
}

pub unsafe extern "C" fn gui_set_size(
    plugin: *const clap_plugin_t,
    width: u32,
    height: u32,
) -> bool {
    let st = &mut *((*plugin).plugin_data as *mut MockPluginState);
    st.gui_width = width;
    st.gui_height = height;
    #[cfg(target_os = "macos")]
    macos::resize_mock_cocoa_view(&st.child_view, width, height);
    #[cfg(target_os = "windows")]
    windows::resize_child_window(st.child_hwnd, width, height);
    #[cfg(target_os = "linux")]
    if let Some(ref x11_st) = st.x11_state {
        x11_st.resize(width, height);
    }
    true
}

pub unsafe extern "C" fn gui_set_parent(
    plugin: *const clap_plugin_t,
    window: *const clap_window_t,
) -> bool {
    if window.is_null() {
        return false;
    }
    let st = &mut *((*plugin).plugin_data as *mut MockPluginState);
    #[cfg(target_os = "macos")]
    {
        macos::attach_mock_cocoa_view(&st.child_view, (*window).handle.cocoa)
    }
    #[cfg(target_os = "windows")]
    {
        let parent_hwnd = (*window).handle.win32;
        if parent_hwnd.is_null() {
            return false;
        }
        if !st.child_hwnd.is_null() {
            let _ = windows::DestroyWindow(st.child_hwnd);
        }
        st.child_hwnd = windows::create_child_window(parent_hwnd, st.gui_width, st.gui_height);
        !st.child_hwnd.is_null()
    }
    #[cfg(target_os = "linux")]
    {
        let parent_xid = (*window).handle.x11;
        if parent_xid == 0 {
            return false;
        }
        if let Some(mut old) = st.x11_state.take() {
            old.destroy(st._host);
        }
        st.x11_state =
            linux::MockX11State::create(st._host, parent_xid, st.gui_width, st.gui_height);
        st.x11_state.is_some()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = st;
        false
    }
}

pub unsafe extern "C" fn gui_show(plugin: *const clap_plugin_t) -> bool {
    #[cfg(target_os = "linux")]
    {
        let st = &*((*plugin).plugin_data as *const MockPluginState);
        if let Some(ref x11_st) = st.x11_state {
            x11_st.paint(st.gui_width, st.gui_height);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = plugin;
    true
}

pub unsafe extern "C" fn gui_hide(_plugin: *const clap_plugin_t) -> bool {
    true
}

pub static EXT_GUI: clap_plugin_gui_t = clap_plugin_gui_t {
    is_api_supported: Some(gui_is_api_supported),
    get_preferred_api: None,
    create: Some(gui_create),
    destroy: Some(gui_destroy),
    set_scale: Some(gui_set_scale),
    get_size: Some(gui_get_size),
    can_resize: Some(gui_can_resize),
    get_resize_hints: None,
    adjust_size: Some(gui_adjust_size),
    set_size: Some(gui_set_size),
    set_parent: Some(gui_set_parent),
    set_transient: None,
    suggest_title: None,
    show: Some(gui_show),
    hide: Some(gui_hide),
};
