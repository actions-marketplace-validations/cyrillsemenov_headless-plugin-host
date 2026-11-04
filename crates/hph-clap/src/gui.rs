use crate::instance::ClapPluginInstance;
use hph_clap_sys::*;
use hph_core::*;
use std::ffi::{c_char, c_void};

impl PluginGui for ClapPluginInstance {
    fn supports_gui(&self) -> bool {
        unsafe {
            if let Some(ext) = self.get_ext::<clap_plugin_gui_t>() {
                if let Some(is_sup) = ext.is_api_supported {
                    #[cfg(target_os = "windows")]
                    let api = CLAP_WINDOW_API_WIN32;
                    #[cfg(target_os = "linux")]
                    let api = CLAP_WINDOW_API_X11;
                    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
                    let api = CLAP_WINDOW_API_COCOA;
                    return is_sup(self.plugin, api.as_ptr() as *const c_char, false);
                }
            }
            false
        }
    }

    fn gui_create(&mut self) -> HostResult<()> {
        if self.gui_created {
            return Ok(());
        }
        unsafe {
            let ext = self
                .get_ext::<clap_plugin_gui_t>()
                .ok_or_else(|| HostError::GuiError("clap.gui extension not supported".into()))?;
            let create_fn = ext
                .create
                .ok_or_else(|| HostError::GuiError("clap.gui.create is null".into()))?;
            #[cfg(target_os = "windows")]
            let api = CLAP_WINDOW_API_WIN32;
            #[cfg(target_os = "linux")]
            let api = CLAP_WINDOW_API_X11;
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            let api = CLAP_WINDOW_API_COCOA;
            if !create_fn(self.plugin, api.as_ptr() as *const c_char, false) {
                return Err(HostError::GuiError(
                    "`clap_plugin_gui.create()` returned false".into(),
                ));
            }
            self.gui_created = true;
            Ok(())
        }
    }

    fn gui_destroy(&mut self) {
        if !self.gui_created {
            return;
        }
        unsafe {
            if let Some(ext) = self.get_ext::<clap_plugin_gui_t>() {
                if let Some(destroy_fn) = ext.destroy {
                    destroy_fn(self.plugin);
                }
            }
        }
        self.gui_created = false;
    }

    fn gui_can_resize(&self) -> bool {
        unsafe {
            if let Some(ext) = self.get_ext::<clap_plugin_gui_t>() {
                if let Some(can_fn) = ext.can_resize {
                    return can_fn(self.plugin);
                }
            }
            false
        }
    }

    fn gui_get_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let ext = self.get_ext::<clap_plugin_gui_t>()?;
            let get_size_fn = ext.get_size?;
            let mut w = 0u32;
            let mut h = 0u32;
            if get_size_fn(self.plugin, &mut w, &mut h) {
                Some((w, h))
            } else {
                None
            }
        }
    }

    fn gui_adjust_size(&self, width: u32, height: u32) -> Option<(u32, u32)> {
        unsafe {
            let ext = self.get_ext::<clap_plugin_gui_t>()?;
            let adj_fn = ext.adjust_size?;
            let mut w = width;
            let mut h = height;
            if adj_fn(self.plugin, &mut w, &mut h) {
                Some((w, h))
            } else {
                None
            }
        }
    }

    fn gui_set_size(&mut self, width: u32, height: u32) -> HostResult<()> {
        unsafe {
            let ext = self
                .get_ext::<clap_plugin_gui_t>()
                .ok_or_else(|| HostError::GuiError("clap.gui not supported".into()))?;
            let set_fn = ext
                .set_size
                .ok_or_else(|| HostError::GuiError("clap.gui.set_size is null".into()))?;
            if !set_fn(self.plugin, width, height) {
                return Err(HostError::GuiError(format!(
                    "`clap_plugin_gui.set_size({width}, {height})` returned false"
                )));
            }
            Ok(())
        }
    }

    unsafe fn gui_set_parent(&mut self, ns_view: *mut c_void) -> HostResult<()> {
        let ext = self
            .get_ext::<clap_plugin_gui_t>()
            .ok_or_else(|| HostError::GuiError("clap.gui not supported".into()))?;
        let set_parent_fn = ext
            .set_parent
            .ok_or_else(|| HostError::GuiError("clap.gui.set_parent is null".into()))?;
        #[cfg(target_os = "windows")]
        let wnd = clap_window_t {
            api: CLAP_WINDOW_API_WIN32.as_ptr() as *const c_char,
            handle: clap_window_handle_t { win32: ns_view },
        };
        #[cfg(target_os = "linux")]
        let wnd = clap_window_t {
            api: CLAP_WINDOW_API_X11.as_ptr() as *const c_char,
            handle: clap_window_handle_t {
                x11: ns_view as usize as libc::c_ulong,
            },
        };
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        let wnd = clap_window_t {
            api: CLAP_WINDOW_API_COCOA.as_ptr() as *const c_char,
            handle: clap_window_handle_t { cocoa: ns_view },
        };
        if !set_parent_fn(self.plugin, &wnd) {
            return Err(HostError::GuiError(
                "`clap_plugin_gui.set_parent()` returned false".into(),
            ));
        }
        Ok(())
    }

    fn gui_show(&mut self) -> HostResult<()> {
        unsafe {
            let ext = self
                .get_ext::<clap_plugin_gui_t>()
                .ok_or_else(|| HostError::GuiError("clap.gui not supported".into()))?;
            let show_fn = ext
                .show
                .ok_or_else(|| HostError::GuiError("clap.gui.show is null".into()))?;
            if !show_fn(self.plugin) {
                return Err(HostError::GuiError(
                    "`clap_plugin_gui.show()` returned false".into(),
                ));
            }
            Ok(())
        }
    }

    fn gui_hide(&mut self) -> HostResult<()> {
        unsafe {
            let ext = self
                .get_ext::<clap_plugin_gui_t>()
                .ok_or_else(|| HostError::GuiError("clap.gui not supported".into()))?;
            let hide_fn = ext
                .hide
                .ok_or_else(|| HostError::GuiError("clap.gui.hide is null".into()))?;
            if !hide_fn(self.plugin) {
                return Err(HostError::GuiError(
                    "`clap_plugin_gui.hide()` returned false".into(),
                ));
            }
            Ok(())
        }
    }

    fn tick_main_thread(&mut self, elapsed_ms: u64) {
        let mut do_callback = false;
        let mut fired_timers = Vec::new();
        let mut fds_to_poll = Vec::new();

        if let Ok(mut guard) = self.host_ctx.inner.lock() {
            if guard.callback_requested {
                guard.callback_requested = false;
                do_callback = true;
            }
            for timer in &mut guard.timers {
                timer.accumulated_ms += elapsed_ms;
                while timer.accumulated_ms >= timer.period_ms as u64 {
                    timer.accumulated_ms -= timer.period_ms as u64;
                    fired_timers.push(timer.id);
                }
            }
            if !guard.posix_fds.is_empty() {
                fds_to_poll.extend_from_slice(&guard.posix_fds);
            }
        }

        unsafe {
            if do_callback {
                if let Some(cb) = (*self.plugin).on_main_thread {
                    cb(self.plugin);
                }
            }
            if !fired_timers.is_empty() {
                if let Some(ext) = self.get_ext::<clap_plugin_timer_support_t>() {
                    if let Some(on_timer) = ext.on_timer {
                        for id in fired_timers {
                            on_timer(self.plugin, id);
                        }
                    }
                }
            }
            #[cfg(unix)]
            if !fds_to_poll.is_empty() {
                if let Some(ext) = self.get_ext::<clap_plugin_posix_fd_support_t>() {
                    if let Some(on_fd) = ext.on_fd {
                        let mut pollfds: Vec<libc::pollfd> = fds_to_poll
                            .iter()
                            .map(|item| {
                                let mut events: i16 = 0;
                                if (item.flags & CLAP_POSIX_FD_READ) != 0 {
                                    events |= libc::POLLIN;
                                }
                                if (item.flags & CLAP_POSIX_FD_WRITE) != 0 {
                                    events |= libc::POLLOUT;
                                }
                                libc::pollfd {
                                    fd: item.fd,
                                    events,
                                    revents: 0,
                                }
                            })
                            .collect();
                        let ready = libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, 0);
                        if ready > 0 {
                            for pfd in &pollfds {
                                if pfd.revents != 0 {
                                    let mut out_flags: clap_posix_fd_flags_t = 0;
                                    if (pfd.revents & libc::POLLIN) != 0 {
                                        out_flags |= CLAP_POSIX_FD_READ;
                                    }
                                    if (pfd.revents & libc::POLLOUT) != 0 {
                                        out_flags |= CLAP_POSIX_FD_WRITE;
                                    }
                                    if (pfd.revents
                                        & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL))
                                        != 0
                                    {
                                        out_flags |= CLAP_POSIX_FD_ERROR;
                                    }
                                    if out_flags != 0 {
                                        on_fd(self.plugin, pfd.fd, out_flags);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
