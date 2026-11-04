use crate::events::{istream_read, ostream_write, SliceReader};
use crate::instance::ClapPluginInstance;
use hph_clap_sys::*;
use hph_core::*;
use std::ffi::{c_char, c_void};
use std::ptr;

impl PluginContractChecks for ClapPluginInstance {
    fn probe_extensions(&self) -> Vec<ExtensionProbeResult> {
        unsafe {
            vec![
                self.probe_ext::<clap_plugin_params_t>("clap.params", |e| {
                    let valid = e.count.is_some()
                        && e.get_info.is_some()
                        && e.get_value.is_some()
                        && e.value_to_text.is_some()
                        && e.text_to_value.is_some()
                        && e.flush.is_some();
                    let notes = e
                        .count
                        .map(|c| format!("{} params", c(self.plugin)))
                        .unwrap_or_default();
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_audio_ports_t>("clap.audio-ports", |e| {
                    let valid = e.count.is_some() && e.get.is_some();
                    let notes = e
                        .count
                        .map(|c| {
                            format!("{} in, {} out", c(self.plugin, true), c(self.plugin, false))
                        })
                        .unwrap_or_default();
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_note_ports_t>("clap.note-ports", |e| {
                    let valid = e.count.is_some() && e.get.is_some();
                    let notes = e
                        .count
                        .map(|c| {
                            format!("{} in, {} out", c(self.plugin, true), c(self.plugin, false))
                        })
                        .unwrap_or_default();
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_state_t>("clap.state", |e| {
                    (e.save.is_some() && e.load.is_some(), String::new())
                }),
                self.probe_ext::<clap_plugin_state_context_t>("clap.state-context", |e| {
                    (
                        e.save.is_some() && e.load.is_some(),
                        "Preset/Duplicate/Project contexts".into(),
                    )
                }),
                self.probe_ext::<clap_plugin_gui_t>("clap.gui", |e| {
                    let valid = e.is_api_supported.is_some()
                        && e.create.is_some()
                        && e.destroy.is_some()
                        && e.set_parent.is_some()
                        && e.show.is_some();
                    let notes = if let Some(is_sup) = e.is_api_supported {
                        #[cfg(target_os = "windows")]
                        let (native_api, label) = (CLAP_WINDOW_API_WIN32, "win32");
                        #[cfg(target_os = "linux")]
                        let (native_api, label) = (CLAP_WINDOW_API_X11, "x11");
                        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
                        let (native_api, label) = (CLAP_WINDOW_API_COCOA, "cocoa");
                        let embedded =
                            is_sup(self.plugin, native_api.as_ptr() as *const c_char, false);
                        format!("{label} embedded: {embedded}")
                    } else {
                        String::new()
                    };
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_latency_t>("clap.latency", |e| {
                    (e.get.is_some(), String::new())
                }),
                self.probe_ext::<clap_plugin_tail_t>("clap.tail", |e| {
                    (e.get.is_some(), String::new())
                }),
                self.probe_ext::<clap_plugin_render_t>("clap.render", |e| {
                    let valid = e.has_hard_realtime_requirement.is_some() && e.set.is_some();
                    let notes = e
                        .has_hard_realtime_requirement
                        .map(|hr| format!("hard_rt: {}", hr(self.plugin)))
                        .unwrap_or_default();
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_voice_info_t>("clap.voice-info", |e| {
                    let mut valid = e.get.is_some();
                    let mut notes = String::new();
                    if let Some(get_fn) = e.get {
                        let mut info: clap_voice_info_t = std::mem::zeroed();
                        if get_fn(self.plugin, &mut info) {
                            valid = info.voice_count <= info.voice_capacity;
                            notes = format!(
                                "voices: {}/{} (overlap={})",
                                info.voice_count,
                                info.voice_capacity,
                                (info.flags & CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES) != 0
                            );
                        }
                    }
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_timer_support_t>("clap.timer-support", |e| {
                    (e.on_timer.is_some(), String::new())
                }),
                self.probe_ext::<clap_plugin_posix_fd_support_t>("clap.posix-fd-support", |e| {
                    (e.on_fd.is_some(), String::new())
                }),
                self.probe_ext::<clap_plugin_thread_pool_t>("clap.thread-pool", |e| {
                    (e.exec.is_some(), "Host synchronous worker pool".into())
                }),
                self.probe_ext::<clap_plugin_preset_load_t>("clap.preset-load", |e| {
                    (e.from_location.is_some(), "Location preset loader".into())
                }),
                self.probe_ext::<clap_plugin_remote_controls_t>("clap.remote-controls", |e| {
                    let valid = e.count.is_some() && e.get.is_some();
                    let notes = e
                        .count
                        .map(|cnt_fn| format!("{} pages", cnt_fn(self.plugin)))
                        .unwrap_or_default();
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_track_info_t>("clap.track-info", |e| {
                    (e.changed.is_some(), "Host track metadata listener".into())
                }),
                self.probe_ext::<clap_plugin_ambisonic_t>("clap.ambisonic", |e| {
                    let valid = e.is_config_supported.is_some() && e.get_config.is_some();
                    let notes = e
                        .is_config_supported
                        .map(|is_sup| {
                            let cfg = clap_ambisonic_config_t {
                                ordering: CLAP_AMBISONIC_ORDERING_ACN,
                                normalization: CLAP_AMBISONIC_NORMALIZATION_SN3D,
                            };
                            format!("ACN/SN3D supported: {}", is_sup(self.plugin, &cfg))
                        })
                        .unwrap_or_default();
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_surround_t>("clap.surround", |e| {
                    let valid =
                        e.is_channel_mask_supported.is_some() && e.get_channel_map.is_some();
                    let notes = e
                        .is_channel_mask_supported
                        .map(|is_sup| format!("5.1 supported: {}", is_sup(self.plugin, 0x3F)))
                        .unwrap_or_default();
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_context_menu_t>("clap.context-menu", |e| {
                    (
                        e.populate.is_some() && e.perform.is_some(),
                        "Global & parameter context menus".into(),
                    )
                }),
                self.probe_ext::<clap_plugin_param_indication_t>("clap.param-indication", |e| {
                    (
                        e.set_mapping.is_some() && e.set_automation.is_some(),
                        "Physical mapping & automation LEDs".into(),
                    )
                }),
                self.probe_ext::<clap_plugin_note_name_t>("clap.note-name", |e| {
                    let valid = e.count.is_some() && e.get.is_some();
                    let notes = e
                        .count
                        .map(|cnt_fn| format!("{} custom note names", cnt_fn(self.plugin)))
                        .unwrap_or_default();
                    (valid, notes)
                }),
                self.probe_ext::<clap_plugin_audio_ports_config_t>(
                    "clap.audio-ports-config",
                    |e| {
                        let valid = e.count.is_some() && e.get.is_some() && e.select.is_some();
                        let notes = e
                            .count
                            .map(|cnt_fn| format!("{} port configs", cnt_fn(self.plugin)))
                            .unwrap_or_default();
                        (valid, notes)
                    },
                ),
                self.probe_ext::<clap_plugin_audio_ports_activation_t>(
                    "clap.audio-ports-activation",
                    |e| {
                        let valid =
                            e.can_activate_while_processing.is_some() && e.set_active.is_some();
                        let notes = e
                            .can_activate_while_processing
                            .map(|can_fn| format!("while_processing: {}", can_fn(self.plugin)))
                            .unwrap_or_default();
                        (valid, notes)
                    },
                ),
                self.probe_ext::<clap_plugin_configurable_audio_ports_t>(
                    "clap.configurable-audio-ports",
                    |e| {
                        (
                            e.can_apply_configuration.is_some() && e.apply_configuration.is_some(),
                            "Dynamic port reconfiguration".into(),
                        )
                    },
                ),
            ]
        }
    }

    fn run_extended_contract_checks(&mut self) -> HostResult<Vec<String>> {
        let mut verified = Vec::new();
        let was_active = self.is_active;
        if was_active {
            self.deactivate();
        }

        unsafe {
            // 1. clap.state-context (Preset, Duplicate, Project roundtrips)
            if let Some(ext) = self.get_ext::<clap_plugin_state_context_t>() {
                if let (Some(save_fn), Some(load_fn)) = (ext.save, ext.load) {
                    let mut ok_contexts = 0usize;
                    for (ctx_name, ctx_type) in [
                        ("PRESET", CLAP_STATE_CONTEXT_FOR_PRESET),
                        ("DUPLICATE", CLAP_STATE_CONTEXT_FOR_DUPLICATE),
                        ("PROJECT", CLAP_STATE_CONTEXT_FOR_PROJECT),
                    ] {
                        let mut data = Vec::new();
                        let ostream = clap_ostream_t {
                            ctx: &mut data as *mut _ as *mut c_void,
                            write: Some(ostream_write),
                        };
                        if !save_fn(self.plugin, &ostream, ctx_type) {
                            return Err(HostError::StateError(format!(
                                "clap.state-context.save({ctx_name}) returned false"
                            )));
                        }
                        let mut reader = SliceReader {
                            data: &data,
                            pos: 0,
                        };
                        let istream = clap_istream_t {
                            ctx: &mut reader as *mut _ as *mut c_void,
                            read: Some(istream_read),
                        };
                        if !load_fn(self.plugin, &istream, ctx_type) {
                            return Err(HostError::StateError(format!(
                                "clap.state-context.load({ctx_name}) returned false"
                            )));
                        }
                        ok_contexts += 1;
                    }
                    verified.push(format!("clap.state-context ({ok_contexts} contexts)"));
                }
            }

            // 2. clap.render (Realtime <-> Offline mode switching)
            if let Some(ext) = self.get_ext::<clap_plugin_render_t>() {
                if let Some(set_fn) = ext.set {
                    let _ = set_fn(self.plugin, CLAP_RENDER_OFFLINE);
                    let _ = set_fn(self.plugin, CLAP_RENDER_REALTIME);
                    verified.push("clap.render (offline/realtime switch)".into());
                }
            }

            // 3. clap.track-info
            if let Some(ext) = self.get_ext::<clap_plugin_track_info_t>() {
                if let Some(changed_fn) = ext.changed {
                    changed_fn(self.plugin);
                    verified.push("clap.track-info (changed -> host.get)".into());
                }
            }

            // 4. clap.voice-info
            if let Some(ext) = self.get_ext::<clap_plugin_voice_info_t>() {
                if let Some(get_fn) = ext.get {
                    let mut info: clap_voice_info_t = std::mem::zeroed();
                    if get_fn(self.plugin, &mut info) {
                        if info.voice_count > info.voice_capacity {
                            return Err(HostError::ProcessError(format!(
                                "clap.voice-info invalid: voice_count ({}) > voice_capacity ({})",
                                info.voice_count, info.voice_capacity
                            )));
                        }
                        verified.push(format!(
                            "clap.voice-info ({}/{})",
                            info.voice_count, info.voice_capacity
                        ));
                    }
                }
            }

            // 5. clap.remote-controls
            if let Some(ext) = self.get_ext::<clap_plugin_remote_controls_t>() {
                if let (Some(count_fn), Some(get_fn)) = (ext.count, ext.get) {
                    let n = count_fn(self.plugin);
                    for i in 0..n {
                        let mut page: clap_remote_controls_page_t = std::mem::zeroed();
                        if !get_fn(self.plugin, i, &mut page) {
                            return Err(HostError::InitError(format!(
                                "clap.remote-controls.get({i}) returned false"
                            )));
                        }
                    }
                    verified.push(format!("clap.remote-controls ({n} pages)"));
                }
            }

            // 6. clap.note-name
            if let Some(ext) = self.get_ext::<clap_plugin_note_name_t>() {
                if let (Some(count_fn), Some(get_fn)) = (ext.count, ext.get) {
                    let n = count_fn(self.plugin);
                    for i in 0..n {
                        let mut nn: clap_note_name_t = std::mem::zeroed();
                        if !get_fn(self.plugin, i, &mut nn) {
                            return Err(HostError::InitError(format!(
                                "clap.note-name.get({i}) returned false"
                            )));
                        }
                        if nn.key < -1 || nn.key > 127 || nn.channel < -1 || nn.channel > 15 {
                            return Err(HostError::InitError(format!(
                                "clap.note-name[{i}] out-of-range key={} channel={}",
                                nn.key, nn.channel
                            )));
                        }
                    }
                    verified.push(format!("clap.note-name ({n} names)"));
                }
            }

            // 7. clap.audio-ports-config
            if let Some(ext) = self.get_ext::<clap_plugin_audio_ports_config_t>() {
                if let (Some(count_fn), Some(get_fn), Some(select_fn)) =
                    (ext.count, ext.get, ext.select)
                {
                    let n = count_fn(self.plugin);
                    let mut first_id = None;
                    for i in 0..n {
                        let mut cfg: clap_audio_ports_config_t = std::mem::zeroed();
                        if !get_fn(self.plugin, i, &mut cfg) {
                            return Err(HostError::InitError(format!(
                                "clap.audio-ports-config.get({i}) returned false"
                            )));
                        }
                        if first_id.is_none() {
                            first_id = Some(cfg.id);
                        }
                        let _ = select_fn(self.plugin, cfg.id);
                    }
                    if let Some(fid) = first_id {
                        let _ = select_fn(self.plugin, fid);
                    }
                    verified.push(format!("clap.audio-ports-config ({n} configs)"));
                }
            }

            // 8. clap.param-indication
            if let Some(ext) = self.get_ext::<clap_plugin_param_indication_t>() {
                let first_param_id = self.params().first().map(|p| p.id);
                if let Some(pid) = first_param_id {
                    let color = clap_color_t {
                        alpha: 255,
                        red: 0,
                        green: 200,
                        blue: 120,
                    };
                    if let Some(set_map) = ext.set_mapping {
                        set_map(
                            self.plugin,
                            pid,
                            true,
                            &color,
                            c"Knob 1".as_ptr(),
                            c"HPH Controller Mapping".as_ptr(),
                        );
                        set_map(
                            self.plugin,
                            pid,
                            false,
                            ptr::null(),
                            ptr::null(),
                            ptr::null(),
                        );
                    }
                    if let Some(set_auto) = ext.set_automation {
                        set_auto(
                            self.plugin,
                            pid,
                            CLAP_PARAM_INDICATION_AUTOMATION_PRESENT,
                            &color,
                        );
                        set_auto(
                            self.plugin,
                            pid,
                            CLAP_PARAM_INDICATION_AUTOMATION_NONE,
                            ptr::null(),
                        );
                    }
                    verified.push("clap.param-indication (mapping & automation)".into());
                }
            }

            // 9. clap.context-menu
            if let Some(ext) = self.get_ext::<clap_plugin_context_menu_t>() {
                if let Some(populate_fn) = ext.populate {
                    unsafe extern "C" fn menu_add_item(
                        builder: *const clap_context_menu_builder_t,
                        _item_kind: clap_context_menu_item_kind_t,
                        _item_data: *const c_void,
                    ) -> bool {
                        if !builder.is_null() && !(*builder).ctx.is_null() {
                            let count = &mut *((*builder).ctx as *mut usize);
                            *count += 1;
                        }
                        true
                    }
                    unsafe extern "C" fn menu_supports(
                        _builder: *const clap_context_menu_builder_t,
                        _item_kind: clap_context_menu_item_kind_t,
                    ) -> bool {
                        true
                    }

                    let mut item_count = 0usize;
                    let builder = clap_context_menu_builder_t {
                        ctx: &mut item_count as *mut _ as *mut c_void,
                        add_item: Some(menu_add_item),
                        supports: Some(menu_supports),
                    };
                    let global_target = clap_context_menu_target_t {
                        kind: CLAP_CONTEXT_MENU_TARGET_KIND_GLOBAL,
                        id: 0,
                    };
                    let _ = populate_fn(self.plugin, &global_target, &builder);
                    verified.push(format!("clap.context-menu ({item_count} items)"));
                }
            }
        }

        Ok(verified)
    }
}
