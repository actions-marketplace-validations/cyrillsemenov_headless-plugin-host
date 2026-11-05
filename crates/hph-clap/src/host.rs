use crate::bundle::cstr_to_string;
use hph_clap_sys::*;
use std::ffi::{c_char, c_void, CStr};
use std::ptr;
use std::sync::Mutex;
use std::thread::ThreadId;

#[derive(Debug, Clone)]
pub struct RegisteredTimer {
    pub id: clap_id,
    pub period_ms: u32,
    pub accumulated_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct RegisteredPosixFd {
    pub fd: i32,
    pub flags: clap_posix_fd_flags_t,
}

pub struct HostStateInner {
    pub main_thread_id: ThreadId,
    pub audio_thread_id: Option<ThreadId>,
    pub logs: Vec<String>,
    pub thread_violations: Vec<String>,
    pub restart_requested: bool,
    pub process_requested: bool,
    pub callback_requested: bool,
    pub flush_requested: bool,
    pub state_dirty: bool,
    pub next_timer_id: clap_id,
    pub timers: Vec<RegisteredTimer>,
    pub posix_fds: Vec<RegisteredPosixFd>,
    pub requested_resize: Option<(u32, u32)>,
}

impl HostStateInner {
    pub(crate) fn new() -> Self {
        Self {
            main_thread_id: std::thread::current().id(),
            audio_thread_id: None,
            logs: Vec::new(),
            thread_violations: Vec::new(),
            restart_requested: false,
            process_requested: false,
            callback_requested: false,
            flush_requested: false,
            state_dirty: false,
            next_timer_id: 1,
            timers: Vec::new(),
            posix_fds: Vec::new(),
            requested_resize: None,
        }
    }

    pub(crate) fn check_main_thread(&mut self, caller: &str) -> bool {
        let current = std::thread::current().id();
        let ok = current == self.main_thread_id;
        if !ok {
            self.thread_violations.push(format!(
                "[main-thread] violation in `{caller}`: called from {:?} instead of {:?}",
                current, self.main_thread_id
            ));
        }
        ok
    }
}

/// Pin-Allocated Host Context passed in `clap_host_t.host_data`.
pub struct ClapHostContext {
    pub inner: Mutex<HostStateInner>,
    pub plugin_ptr: std::sync::atomic::AtomicPtr<clap_plugin_t>,
    pub host_struct: clap_host_t,
}

unsafe fn with_host<R>(
    host: *const clap_host_t,
    f: impl FnOnce(&mut HostStateInner) -> R,
) -> Option<R> {
    if host.is_null() || (*host).host_data.is_null() {
        return None;
    }
    let ctx = &*((*host).host_data as *const ClapHostContext);
    let mut guard = ctx.inner.lock().ok()?;
    Some(f(&mut guard))
}

unsafe fn with_main_thread<R>(
    host: *const clap_host_t,
    caller: &str,
    f: impl FnOnce(&mut HostStateInner) -> R,
) -> Option<R> {
    with_host(host, |state| {
        state.check_main_thread(caller);
        f(state)
    })
}

static EXT_THREAD_CHECK: clap_host_thread_check_t = clap_host_thread_check_t {
    is_main_thread: Some(host_is_main_thread),
    is_audio_thread: Some(host_is_audio_thread),
};
static EXT_LOG: clap_host_log_t = clap_host_log_t {
    log: Some(host_log),
};
static EXT_PARAMS: clap_host_params_t = clap_host_params_t {
    rescan: Some(host_params_rescan),
    clear: Some(host_params_clear),
    request_flush: Some(host_params_request_flush),
};
static EXT_AUDIO_PORTS: clap_host_audio_ports_t = clap_host_audio_ports_t {
    is_rescan_flag_supported: Some(host_audio_ports_is_rescan_supported),
    rescan: Some(host_audio_ports_rescan),
};
static EXT_NOTE_PORTS: clap_host_note_ports_t = clap_host_note_ports_t {
    supported_dialects: Some(host_note_ports_supported_dialects),
    rescan: Some(host_note_ports_rescan),
};
static EXT_STATE: clap_host_state_t = clap_host_state_t {
    mark_dirty: Some(host_state_mark_dirty),
};
static EXT_LATENCY: clap_host_latency_t = clap_host_latency_t {
    changed: Some(host_latency_changed),
};
static EXT_TAIL: clap_host_tail_t = clap_host_tail_t {
    changed: Some(host_tail_changed),
};
static EXT_TIMER_SUPPORT: clap_host_timer_support_t = clap_host_timer_support_t {
    register_timer: Some(host_register_timer),
    unregister_timer: Some(host_unregister_timer),
};
static EXT_POSIX_FD_SUPPORT: clap_host_posix_fd_support_t = clap_host_posix_fd_support_t {
    register_fd: Some(host_register_fd),
    modify_fd: Some(host_modify_fd),
    unregister_fd: Some(host_unregister_fd),
};
static EXT_GUI: clap_host_gui_t = clap_host_gui_t {
    resize_hints_changed: Some(host_gui_resize_hints_changed),
    request_resize: Some(host_gui_request_resize),
    request_show: Some(host_gui_request_show),
    request_hide: Some(host_gui_request_hide),
    closed: Some(host_gui_closed),
};
static EXT_VOICE_INFO: clap_host_voice_info_t = clap_host_voice_info_t {
    changed: Some(host_voice_info_changed),
};
static EXT_THREAD_POOL: clap_host_thread_pool_t = clap_host_thread_pool_t {
    request_exec: Some(host_thread_pool_request_exec),
};
static EXT_PRESET_LOAD: clap_host_preset_load_t = clap_host_preset_load_t {
    on_error: Some(host_preset_load_on_error),
    loaded: Some(host_preset_load_loaded),
};
static EXT_REMOTE_CONTROLS: clap_host_remote_controls_t = clap_host_remote_controls_t {
    changed: Some(host_remote_controls_changed),
    suggest_page: Some(host_remote_controls_suggest_page),
};
static EXT_TRACK_INFO: clap_host_track_info_t = clap_host_track_info_t {
    get: Some(host_track_info_get),
};
static EXT_AMBISONIC: clap_host_ambisonic_t = clap_host_ambisonic_t {
    changed: Some(host_ambisonic_changed),
};
static EXT_SURROUND: clap_host_surround_t = clap_host_surround_t {
    changed: Some(host_surround_changed),
};
static EXT_CONTEXT_MENU: clap_host_context_menu_t = clap_host_context_menu_t {
    populate: Some(host_context_menu_populate),
    perform: Some(host_context_menu_perform),
    can_popup: Some(host_context_menu_can_popup),
    popup: Some(host_context_menu_popup),
};
static EXT_NOTE_NAME: clap_host_note_name_t = clap_host_note_name_t {
    changed: Some(host_note_name_changed),
};
static EXT_AUDIO_PORTS_CONFIG: clap_host_audio_ports_config_t = clap_host_audio_ports_config_t {
    rescan: Some(host_audio_ports_config_rescan),
};

struct HostExtEntry {
    id: &'static [u8],
    ptr: *const c_void,
}

unsafe impl Sync for HostExtEntry {}

static HOST_EXTENSIONS: &[HostExtEntry] = &[
    HostExtEntry {
        id: CLAP_EXT_THREAD_CHECK,
        ptr: &EXT_THREAD_CHECK as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_LOG,
        ptr: &EXT_LOG as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_PARAMS,
        ptr: &EXT_PARAMS as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_AUDIO_PORTS,
        ptr: &EXT_AUDIO_PORTS as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_NOTE_PORTS,
        ptr: &EXT_NOTE_PORTS as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_STATE,
        ptr: &EXT_STATE as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_LATENCY,
        ptr: &EXT_LATENCY as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_TAIL,
        ptr: &EXT_TAIL as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_TIMER_SUPPORT,
        ptr: &EXT_TIMER_SUPPORT as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_POSIX_FD_SUPPORT,
        ptr: &EXT_POSIX_FD_SUPPORT as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_GUI,
        ptr: &EXT_GUI as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_VOICE_INFO,
        ptr: &EXT_VOICE_INFO as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_THREAD_POOL,
        ptr: &EXT_THREAD_POOL as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_PRESET_LOAD,
        ptr: &EXT_PRESET_LOAD as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_REMOTE_CONTROLS,
        ptr: &EXT_REMOTE_CONTROLS as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_TRACK_INFO,
        ptr: &EXT_TRACK_INFO as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_AMBISONIC,
        ptr: &EXT_AMBISONIC as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_SURROUND,
        ptr: &EXT_SURROUND as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_CONTEXT_MENU,
        ptr: &EXT_CONTEXT_MENU as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_NOTE_NAME,
        ptr: &EXT_NOTE_NAME as *const _ as *const c_void,
    },
    HostExtEntry {
        id: CLAP_EXT_AUDIO_PORTS_CONFIG,
        ptr: &EXT_AUDIO_PORTS_CONFIG as *const _ as *const c_void,
    },
];

unsafe extern "C" fn host_get_extension(
    host: *const clap_host_t,
    extension_id: *const c_char,
) -> *const c_void {
    if host.is_null() || extension_id.is_null() {
        return ptr::null();
    }
    let id_bytes = CStr::from_ptr(extension_id).to_bytes_with_nul();
    HOST_EXTENSIONS
        .iter()
        .find(|entry| entry.id == id_bytes)
        .map_or(ptr::null(), |entry| entry.ptr)
}

unsafe extern "C" fn host_request_restart(host: *const clap_host_t) {
    with_host(host, |s| s.restart_requested = true);
}

unsafe extern "C" fn host_request_process(host: *const clap_host_t) {
    with_host(host, |s| s.process_requested = true);
}

unsafe extern "C" fn host_request_callback(host: *const clap_host_t) {
    with_host(host, |s| s.callback_requested = true);
}

unsafe extern "C" fn host_is_main_thread(host: *const clap_host_t) -> bool {
    with_host(host, |s| std::thread::current().id() == s.main_thread_id).unwrap_or(false)
}

unsafe extern "C" fn host_is_audio_thread(host: *const clap_host_t) -> bool {
    with_host(host, |s| {
        s.audio_thread_id == Some(std::thread::current().id())
    })
    .unwrap_or(false)
}

unsafe extern "C" fn host_log(
    host: *const clap_host_t,
    severity: clap_log_severity,
    msg: *const c_char,
) {
    let text = cstr_to_string(msg);
    let level = match severity {
        CLAP_LOG_DEBUG => "DEBUG",
        CLAP_LOG_INFO => "INFO",
        CLAP_LOG_WARNING => "WARN",
        CLAP_LOG_ERROR => "ERROR",
        CLAP_LOG_FATAL => "FATAL",
        CLAP_LOG_HOST_MISBEHAVING => "HOST_MISBEHAVING",
        CLAP_LOG_PLUGIN_MISBEHAVING => "PLUGIN_MISBEHAVING",
        _ => "UNKNOWN",
    };
    with_host(host, |s| s.logs.push(format!("[{level}] {text}")));
}

unsafe extern "C" fn host_params_rescan(host: *const clap_host_t, _flags: u32) {
    with_main_thread(host, "clap_host_params::rescan", |_| {});
}

unsafe extern "C" fn host_params_clear(host: *const clap_host_t, _param_id: clap_id, _flags: u32) {
    with_main_thread(host, "clap_host_params::clear", |_| {});
}

unsafe extern "C" fn host_params_request_flush(host: *const clap_host_t) {
    with_host(host, |s| {
        if s.audio_thread_id == Some(std::thread::current().id()) {
            s.thread_violations.push(
                "[!audio-thread] violation: clap_host_params::request_flush called from audio thread"
                    .to_string(),
            );
        }
        s.flush_requested = true;
    });
}

unsafe extern "C" fn host_audio_ports_is_rescan_supported(
    host: *const clap_host_t,
    _flag: u32,
) -> bool {
    with_main_thread(
        host,
        "clap_host_audio_ports::is_rescan_flag_supported",
        |_| {},
    );
    true
}

unsafe extern "C" fn host_audio_ports_rescan(host: *const clap_host_t, _flags: u32) {
    with_main_thread(host, "clap_host_audio_ports::rescan", |_| {});
}

unsafe extern "C" fn host_note_ports_supported_dialects(host: *const clap_host_t) -> u32 {
    with_main_thread(host, "clap_host_note_ports::supported_dialects", |_| {});
    CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI | CLAP_NOTE_DIALECT_MIDI_MPE
}

unsafe extern "C" fn host_note_ports_rescan(host: *const clap_host_t, _flags: u32) {
    with_main_thread(host, "clap_host_note_ports::rescan", |_| {});
}

unsafe extern "C" fn host_state_mark_dirty(host: *const clap_host_t) {
    with_main_thread(host, "clap_host_state::mark_dirty", |s| {
        s.state_dirty = true
    });
}

unsafe extern "C" fn host_latency_changed(host: *const clap_host_t) {
    with_main_thread(host, "clap_host_latency::changed", |_| {});
}

unsafe extern "C" fn host_tail_changed(_host: *const clap_host_t) {}

unsafe extern "C" fn host_register_timer(
    host: *const clap_host_t,
    period_ms: u32,
    timer_id: *mut clap_id,
) -> bool {
    if timer_id.is_null() {
        return false;
    }
    with_main_thread(host, "clap_host_timer_support::register_timer", |s| {
        let id = s.next_timer_id;
        s.next_timer_id += 1;
        s.timers.push(RegisteredTimer {
            id,
            period_ms: period_ms.max(1),
            accumulated_ms: 0,
        });
        *timer_id = id;
        true
    })
    .unwrap_or(false)
}

unsafe extern "C" fn host_unregister_timer(host: *const clap_host_t, timer_id: clap_id) -> bool {
    with_main_thread(host, "clap_host_timer_support::unregister_timer", |s| {
        let before = s.timers.len();
        s.timers.retain(|t| t.id != timer_id);
        s.timers.len() != before
    })
    .unwrap_or(false)
}

unsafe extern "C" fn host_register_fd(
    host: *const clap_host_t,
    fd: i32,
    flags: clap_posix_fd_flags_t,
) -> bool {
    with_main_thread(host, "clap_host_posix_fd_support::register_fd", |s| {
        if s.posix_fds.iter().any(|item| item.fd == fd) {
            return false;
        }
        s.posix_fds.push(RegisteredPosixFd { fd, flags });
        true
    })
    .unwrap_or(false)
}

unsafe extern "C" fn host_modify_fd(
    host: *const clap_host_t,
    fd: i32,
    flags: clap_posix_fd_flags_t,
) -> bool {
    with_main_thread(host, "clap_host_posix_fd_support::modify_fd", |s| {
        if let Some(item) = s.posix_fds.iter_mut().find(|item| item.fd == fd) {
            item.flags = flags;
            true
        } else {
            false
        }
    })
    .unwrap_or(false)
}

unsafe extern "C" fn host_unregister_fd(host: *const clap_host_t, fd: i32) -> bool {
    with_main_thread(host, "clap_host_posix_fd_support::unregister_fd", |s| {
        let before = s.posix_fds.len();
        s.posix_fds.retain(|item| item.fd != fd);
        s.posix_fds.len() != before
    })
    .unwrap_or(false)
}

unsafe extern "C" fn host_gui_resize_hints_changed(_host: *const clap_host_t) {}

unsafe extern "C" fn host_gui_request_resize(
    host: *const clap_host_t,
    width: u32,
    height: u32,
) -> bool {
    with_host(host, |s| {
        s.requested_resize = Some((width, height));
        true
    })
    .unwrap_or(false)
}

unsafe extern "C" fn host_gui_request_show(_host: *const clap_host_t) -> bool {
    true
}

unsafe extern "C" fn host_gui_request_hide(_host: *const clap_host_t) -> bool {
    true
}

unsafe extern "C" fn host_gui_closed(_host: *const clap_host_t, _was_destroyed: bool) {}

unsafe extern "C" fn host_voice_info_changed(host: *const clap_host_t) {
    with_main_thread(host, "clap_host_voice_info::changed", |_| {});
}

unsafe extern "C" fn host_thread_pool_request_exec(
    host: *const clap_host_t,
    num_tasks: u32,
) -> bool {
    if host.is_null() || (*host).host_data.is_null() {
        return false;
    }
    let ctx = &*((*host).host_data as *const ClapHostContext);
    with_host(host, |s| {
        if s.audio_thread_id != Some(std::thread::current().id()) {
            s.thread_violations.push(
                "[audio-thread] violation: clap_host_thread_pool::request_exec called outside audio thread"
                    .to_string(),
            );
        }
    });
    let plugin = ctx.plugin_ptr.load(std::sync::atomic::Ordering::Acquire);
    if plugin.is_null() {
        return false;
    }
    if let Some(get_ext) = (*plugin).get_extension {
        let ext_ptr = get_ext(
            plugin,
            clap_plugin_thread_pool_t::ID.as_ptr() as *const c_char,
        ) as *const clap_plugin_thread_pool_t;
        if !ext_ptr.is_null() {
            if let Some(exec_fn) = (*ext_ptr).exec {
                for task_idx in 0..num_tasks {
                    exec_fn(plugin, task_idx);
                }
                return true;
            }
        }
    }
    false
}

unsafe extern "C" fn host_preset_load_on_error(
    host: *const clap_host_t,
    _location_kind: u32,
    location: *const c_char,
    _load_key: *const c_char,
    os_error: i32,
    msg: *const c_char,
) {
    let loc_str = cstr_to_string(location);
    let msg_str = cstr_to_string(msg);
    with_main_thread(host, "clap_host_preset_load::on_error", |s| {
        s.logs.push(format!(
            "[PRESET_ERROR] location='{loc_str}' os_err={os_error}: {msg_str}"
        ));
    });
}

unsafe extern "C" fn host_preset_load_loaded(
    host: *const clap_host_t,
    _location_kind: u32,
    _location: *const c_char,
    _load_key: *const c_char,
) {
    with_main_thread(host, "clap_host_preset_load::loaded", |_| {});
}

unsafe extern "C" fn host_remote_controls_changed(host: *const clap_host_t) {
    with_main_thread(host, "clap_host_remote_controls::changed", |_| {});
}

unsafe extern "C" fn host_remote_controls_suggest_page(
    host: *const clap_host_t,
    _page_id: clap_id,
) {
    with_main_thread(host, "clap_host_remote_controls::suggest_page", |_| {});
}

unsafe extern "C" fn host_track_info_get(
    host: *const clap_host_t,
    info: *mut clap_track_info_t,
) -> bool {
    if info.is_null() {
        return false;
    }
    with_main_thread(host, "clap_host_track_info::get", |_| {});
    let out = &mut *info;
    *out = std::mem::zeroed();
    out.flags = CLAP_TRACK_INFO_HAS_TRACK_NAME
        | CLAP_TRACK_INFO_HAS_TRACK_COLOR
        | CLAP_TRACK_INFO_HAS_AUDIO_CHANNEL;
    let track_name = b"HPH Validation Track";
    for (dst, &src) in out
        .name
        .iter_mut()
        .take(CLAP_NAME_SIZE - 1)
        .zip(track_name.iter())
    {
        *dst = src as c_char;
    }
    out.color = clap_color_t {
        alpha: 255,
        red: 64,
        green: 160,
        blue: 255,
    };
    out.audio_channel_count = 2;
    out.audio_port_type = CLAP_PORT_STEREO.as_ptr() as *const c_char;
    true
}

unsafe extern "C" fn host_ambisonic_changed(host: *const clap_host_t) {
    with_main_thread(host, "clap_host_ambisonic::changed", |_| {});
}

unsafe extern "C" fn host_surround_changed(host: *const clap_host_t) {
    with_main_thread(host, "clap_host_surround::changed", |_| {});
}

unsafe extern "C" fn host_context_menu_populate(
    host: *const clap_host_t,
    _target: *const clap_context_menu_target_t,
    _builder: *const clap_context_menu_builder_t,
) -> bool {
    with_main_thread(host, "clap_host_context_menu::populate", |_| {});
    true
}

unsafe extern "C" fn host_context_menu_perform(
    host: *const clap_host_t,
    _target: *const clap_context_menu_target_t,
    _action_id: clap_id,
) -> bool {
    with_main_thread(host, "clap_host_context_menu::perform", |_| {});
    true
}

unsafe extern "C" fn host_context_menu_can_popup(host: *const clap_host_t) -> bool {
    with_main_thread(host, "clap_host_context_menu::can_popup", |_| {});
    false
}

unsafe extern "C" fn host_context_menu_popup(
    host: *const clap_host_t,
    _target: *const clap_context_menu_target_t,
    _screen_index: i32,
    _x: i32,
    _y: i32,
) -> bool {
    with_main_thread(host, "clap_host_context_menu::popup", |_| {});
    false
}

unsafe extern "C" fn host_note_name_changed(host: *const clap_host_t) {
    with_main_thread(host, "clap_host_note_name::changed", |_| {});
}

unsafe extern "C" fn host_audio_ports_config_rescan(host: *const clap_host_t) {
    with_main_thread(host, "clap_host_audio_ports_config::rescan", |_| {});
}

const HOST_NAME: &[u8] = b"headless-plugin-host\0";
const HOST_VENDOR: &[u8] = b"hph\0";
const HOST_URL: &[u8] = b"https://github.com/cyrillsemenov/headless-plugin-host\0";
const HOST_VERSION: &[u8] = b"0.1.0\0";

impl ClapHostContext {
    pub fn new_boxed() -> Box<Self> {
        let mut boxed = Box::new(Self {
            inner: Mutex::new(HostStateInner::new()),
            plugin_ptr: std::sync::atomic::AtomicPtr::new(ptr::null_mut()),
            host_struct: clap_host_t {
                clap_version: CLAP_VERSION,
                host_data: ptr::null_mut(),
                name: HOST_NAME.as_ptr() as *const c_char,
                vendor: HOST_VENDOR.as_ptr() as *const c_char,
                url: HOST_URL.as_ptr() as *const c_char,
                version: HOST_VERSION.as_ptr() as *const c_char,
                get_extension: Some(host_get_extension),
                request_restart: Some(host_request_restart),
                request_process: Some(host_request_process),
                request_callback: Some(host_request_callback),
            },
        });
        let self_ptr = &mut *boxed as *mut ClapHostContext as *mut c_void;
        boxed.host_struct.host_data = self_ptr;
        boxed
    }
}
