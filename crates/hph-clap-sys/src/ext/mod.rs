pub mod gui;
pub mod host_services;
pub mod params_ports;
pub mod state_streams;

pub use gui::*;
pub use host_services::*;
pub use params_ports::*;
pub use state_streams::*;

pub const CLAP_EXT_PARAMS: &[u8] = b"clap.params\0";
pub const CLAP_EXT_AUDIO_PORTS: &[u8] = b"clap.audio-ports\0";
pub const CLAP_EXT_NOTE_PORTS: &[u8] = b"clap.note-ports\0";
pub const CLAP_EXT_STATE: &[u8] = b"clap.state\0";
pub const CLAP_EXT_STATE_CONTEXT: &[u8] = b"clap.state-context\0";
pub const CLAP_EXT_GUI: &[u8] = b"clap.gui\0";
pub const CLAP_EXT_LATENCY: &[u8] = b"clap.latency\0";
pub const CLAP_EXT_TAIL: &[u8] = b"clap.tail\0";
pub const CLAP_EXT_RENDER: &[u8] = b"clap.render\0";
pub const CLAP_EXT_THREAD_CHECK: &[u8] = b"clap.thread-check\0";
pub const CLAP_EXT_THREAD_POOL: &[u8] = b"clap.thread-pool\0";
pub const CLAP_EXT_TIMER_SUPPORT: &[u8] = b"clap.timer-support\0";
pub const CLAP_EXT_POSIX_FD_SUPPORT: &[u8] = b"clap.posix-fd-support\0";
pub const CLAP_EXT_VOICE_INFO: &[u8] = b"clap.voice-info\0";
pub const CLAP_EXT_LOG: &[u8] = b"clap.log\0";
pub const CLAP_EXT_PRESET_LOAD: &[u8] = b"clap.preset-load\0";
pub const CLAP_EXT_REMOTE_CONTROLS: &[u8] = b"clap.remote-controls\0";
pub const CLAP_EXT_TRACK_INFO: &[u8] = b"clap.track-info\0";
pub const CLAP_EXT_AMBISONIC: &[u8] = b"clap.ambisonic\0";
pub const CLAP_EXT_SURROUND: &[u8] = b"clap.surround\0";
pub const CLAP_EXT_CONTEXT_MENU: &[u8] = b"clap.context-menu\0";
pub const CLAP_EXT_PARAM_INDICATION: &[u8] = b"clap.param-indication\0";
pub const CLAP_EXT_NOTE_NAME: &[u8] = b"clap.note-name\0";
pub const CLAP_EXT_AUDIO_PORTS_CONFIG: &[u8] = b"clap.audio-ports-config\0";
pub const CLAP_EXT_AUDIO_PORTS_ACTIVATION: &[u8] = b"clap.audio-ports-activation\0";
pub const CLAP_EXT_CONFIGURABLE_AUDIO_PORTS: &[u8] = b"clap.configurable-audio-ports\0";

/// Type-level binding between a CLAP plugin extension C struct and its null-terminated extension ID.
///
/// # Safety
/// Implementors must ensure `Self` is the exact `#[repr(C)]` vtable struct corresponding to `Self::ID`.
pub unsafe trait ClapExtension: Sized {
    const ID: &'static [u8];
}

macro_rules! impl_clap_extension {
    ($($ty:ty => $id:ident),* $(,)?) => {
        $(
            unsafe impl ClapExtension for $ty {
                const ID: &'static [u8] = $id;
            }
        )*
    };
}

impl_clap_extension! {
    clap_plugin_params_t => CLAP_EXT_PARAMS,
    clap_plugin_audio_ports_t => CLAP_EXT_AUDIO_PORTS,
    clap_plugin_note_ports_t => CLAP_EXT_NOTE_PORTS,
    clap_plugin_state_t => CLAP_EXT_STATE,
    clap_plugin_state_context_t => CLAP_EXT_STATE_CONTEXT,
    clap_plugin_latency_t => CLAP_EXT_LATENCY,
    clap_plugin_tail_t => CLAP_EXT_TAIL,
    clap_plugin_render_t => CLAP_EXT_RENDER,
    clap_plugin_gui_t => CLAP_EXT_GUI,
    clap_plugin_timer_support_t => CLAP_EXT_TIMER_SUPPORT,
    clap_plugin_posix_fd_support_t => CLAP_EXT_POSIX_FD_SUPPORT,
    clap_plugin_voice_info_t => CLAP_EXT_VOICE_INFO,
    clap_plugin_thread_pool_t => CLAP_EXT_THREAD_POOL,
    clap_plugin_preset_load_t => CLAP_EXT_PRESET_LOAD,
    clap_plugin_remote_controls_t => CLAP_EXT_REMOTE_CONTROLS,
    clap_plugin_track_info_t => CLAP_EXT_TRACK_INFO,
    clap_plugin_ambisonic_t => CLAP_EXT_AMBISONIC,
    clap_plugin_surround_t => CLAP_EXT_SURROUND,
    clap_plugin_param_indication_t => CLAP_EXT_PARAM_INDICATION,
    clap_plugin_context_menu_t => CLAP_EXT_CONTEXT_MENU,
    clap_plugin_note_name_t => CLAP_EXT_NOTE_NAME,
    clap_plugin_audio_ports_config_t => CLAP_EXT_AUDIO_PORTS_CONFIG,
    clap_plugin_configurable_audio_ports_t => CLAP_EXT_CONFIGURABLE_AUDIO_PORTS,
    clap_plugin_audio_ports_activation_t => CLAP_EXT_AUDIO_PORTS_ACTIVATION,
}
