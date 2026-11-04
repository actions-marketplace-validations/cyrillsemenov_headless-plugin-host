#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

pub mod core;
pub mod events;
pub mod ext;

pub use core::*;
pub use events::*;
pub use ext::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn verify_clap_abi_sizes() {
        assert_eq!(size_of::<clap_version_t>(), 12);
        assert_eq!(size_of::<clap_event_header_t>(), 16);
        assert_eq!(size_of::<clap_event_note_t>(), 40);
        assert_eq!(size_of::<clap_event_param_value_t>(), 56);
        assert_eq!(size_of::<clap_event_param_mod_t>(), 56);
        assert_eq!(size_of::<clap_event_midi_t>(), 24);
        assert_eq!(size_of::<clap_param_info_t>(), 1320);
        assert_eq!(size_of::<clap_color_t>(), 4);
        assert_eq!(size_of::<clap_note_name_t>(), 262);
        assert_eq!(size_of::<clap_remote_controls_page_t>(), 552);
    }

    #[test]
    fn verify_clap_extension_trait_ids() {
        fn assert_valid_ext<T: ClapExtension>(expected: &[u8]) {
            assert_eq!(T::ID, expected);
            assert_eq!(
                T::ID.last().copied(),
                Some(0),
                "Extension ID must be NUL-terminated"
            );
            assert!(!T::ID[..T::ID.len() - 1].contains(&0));
        }

        assert_valid_ext::<clap_plugin_audio_ports_t>(CLAP_EXT_AUDIO_PORTS);
        assert_valid_ext::<clap_plugin_note_ports_t>(CLAP_EXT_NOTE_PORTS);
        assert_valid_ext::<clap_plugin_params_t>(CLAP_EXT_PARAMS);
        assert_valid_ext::<clap_plugin_state_t>(CLAP_EXT_STATE);
        assert_valid_ext::<clap_plugin_latency_t>(CLAP_EXT_LATENCY);
        assert_valid_ext::<clap_plugin_tail_t>(CLAP_EXT_TAIL);
        assert_valid_ext::<clap_plugin_render_t>(CLAP_EXT_RENDER);
        assert_valid_ext::<clap_plugin_gui_t>(CLAP_EXT_GUI);
        assert_valid_ext::<clap_plugin_timer_support_t>(CLAP_EXT_TIMER_SUPPORT);
        assert_valid_ext::<clap_plugin_posix_fd_support_t>(CLAP_EXT_POSIX_FD_SUPPORT);
        assert_valid_ext::<clap_plugin_voice_info_t>(CLAP_EXT_VOICE_INFO);
        assert_valid_ext::<clap_plugin_state_context_t>(CLAP_EXT_STATE_CONTEXT);
        assert_valid_ext::<clap_plugin_preset_load_t>(CLAP_EXT_PRESET_LOAD);
        assert_valid_ext::<clap_plugin_track_info_t>(CLAP_EXT_TRACK_INFO);
        assert_valid_ext::<clap_plugin_audio_ports_config_t>(CLAP_EXT_AUDIO_PORTS_CONFIG);
        assert_valid_ext::<clap_plugin_thread_pool_t>(CLAP_EXT_THREAD_POOL);
        assert_valid_ext::<clap_plugin_audio_ports_activation_t>(CLAP_EXT_AUDIO_PORTS_ACTIVATION);
        assert_valid_ext::<clap_plugin_configurable_audio_ports_t>(
            CLAP_EXT_CONFIGURABLE_AUDIO_PORTS,
        );
        assert_valid_ext::<clap_plugin_surround_t>(CLAP_EXT_SURROUND);
        assert_valid_ext::<clap_plugin_ambisonic_t>(CLAP_EXT_AMBISONIC);
        assert_valid_ext::<clap_plugin_note_name_t>(CLAP_EXT_NOTE_NAME);
        assert_valid_ext::<clap_plugin_remote_controls_t>(CLAP_EXT_REMOTE_CONTROLS);
        assert_valid_ext::<clap_plugin_param_indication_t>(CLAP_EXT_PARAM_INDICATION);
        assert_valid_ext::<clap_plugin_context_menu_t>(CLAP_EXT_CONTEXT_MENU);
    }
}
