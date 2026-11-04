use crate::instance::Vst3PluginInstance;
use crate::plug_frame::Vst3PlugFrame;
use hph_core::*;
use hph_vst3_sys::*;
use std::ffi::c_void;
use std::ptr;

impl PluginGui for Vst3PluginInstance {
    fn supports_gui(&self) -> bool {
        let Some(ref ctrl) = self.controller else {
            return false;
        };
        unsafe {
            let Some(view) = ComPtr::<Steinberg_IPlugView>::from_raw((ctrl.createView)(
                ctrl.as_void(),
                kEditorViewType.as_ptr() as Steinberg_FIDString,
            )) else {
                return false;
            };
            #[cfg(target_os = "windows")]
            let platform_type = kPlatformTypeHWND;
            #[cfg(target_os = "linux")]
            let platform_type = kPlatformTypeX11EmbedWindowID;
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            let platform_type = kPlatformTypeNSView;
            (view.isPlatformTypeSupported)(
                view.as_void(),
                platform_type.as_ptr() as Steinberg_FIDString,
            ) == kResultOk
        }
    }

    fn gui_create(&mut self) -> HostResult<()> {
        if self.plug_view.is_some() {
            self.gui_destroy();
        }
        let ctrl = self
            .controller
            .as_ref()
            .ok_or_else(|| HostError::GuiError("Plugin has no IEditController".into()))?;
        unsafe {
            let view = ComPtr::<Steinberg_IPlugView>::from_raw((ctrl.createView)(
                ctrl.as_void(),
                kEditorViewType.as_ptr() as Steinberg_FIDString,
            ))
            .ok_or_else(|| {
                HostError::GuiError("IEditController::createView(\"editor\") returned null".into())
            })?;
            #[cfg(target_os = "windows")]
            let platform_type = kPlatformTypeHWND;
            #[cfg(target_os = "linux")]
            let platform_type = kPlatformTypeX11EmbedWindowID;
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            let platform_type = kPlatformTypeNSView;
            let res = (view.isPlatformTypeSupported)(
                view.as_void(),
                platform_type.as_ptr() as Steinberg_FIDString,
            );
            if res != kResultOk {
                return Err(HostError::GuiError(
                    "IPlugView does not support native platform window type".into(),
                ));
            }
            let _ = (view.setFrame)(
                view.as_void(),
                &mut *self.plug_frame as *mut Vst3PlugFrame as *mut Steinberg_IPlugFrame,
            );
            self.plug_view = Some(view);
            self.gui_attached = false;
        }
        Ok(())
    }

    fn gui_destroy(&mut self) {
        if let Some(view) = self.plug_view.take() {
            unsafe {
                if self.gui_attached {
                    let _ = (view.removed)(view.as_void());
                    self.gui_attached = false;
                }
                let _ = (view.setFrame)(view.as_void(), ptr::null_mut());
            }
        }
    }

    fn gui_can_resize(&self) -> bool {
        self.plug_view
            .as_ref()
            .is_some_and(|view| unsafe { (view.canResize)(view.as_void()) == kResultTrue })
    }

    fn gui_get_size(&self) -> Option<(u32, u32)> {
        let view = self.plug_view.as_ref()?;
        unsafe {
            let mut rect = Steinberg_ViewRect::default();
            if (view.getSize)(view.as_void(), &mut rect) == kResultOk {
                let w = (rect.right - rect.left).max(1) as u32;
                let h = (rect.bottom - rect.top).max(1) as u32;
                return Some((w, h));
            }
        }
        None
    }

    fn gui_adjust_size(&self, width: u32, height: u32) -> Option<(u32, u32)> {
        let view = self.plug_view.as_ref()?;
        unsafe {
            let mut rect = Steinberg_ViewRect {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            };
            if (view.checkSizeConstraint)(view.as_void(), &mut rect) == kResultOk {
                let w = (rect.right - rect.left).max(1) as u32;
                let h = (rect.bottom - rect.top).max(1) as u32;
                return Some((w, h));
            }
        }
        Some((width, height))
    }

    fn gui_set_size(&mut self, width: u32, height: u32) -> HostResult<()> {
        let view = self
            .plug_view
            .as_ref()
            .ok_or_else(|| HostError::GuiError("No active IPlugView".into()))?;
        unsafe {
            let mut rect = Steinberg_ViewRect {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            };
            let res = (view.onSize)(view.as_void(), &mut rect);
            if res != kResultOk && res != kResultFalse {
                return Err(HostError::GuiError(format!(
                    "IPlugView::onSize failed with tresult {}",
                    res
                )));
            }
        }
        Ok(())
    }

    unsafe fn gui_set_parent(&mut self, ns_view: *mut c_void) -> HostResult<()> {
        let view = self
            .plug_view
            .as_ref()
            .ok_or_else(|| HostError::GuiError("No active IPlugView".into()))?;
        unsafe {
            #[cfg(target_os = "windows")]
            let platform_type = kPlatformTypeHWND;
            #[cfg(target_os = "linux")]
            let platform_type = kPlatformTypeX11EmbedWindowID;
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            let platform_type = kPlatformTypeNSView;
            let res = (view.attached)(
                view.as_void(),
                ns_view,
                platform_type.as_ptr() as Steinberg_FIDString,
            );
            if res != kResultOk {
                return Err(HostError::GuiError(format!(
                    "IPlugView::attached() failed with tresult {}",
                    res
                )));
            }
            self.gui_attached = true;
        }
        Ok(())
    }

    fn tick_main_thread(&mut self, elapsed_ms: u64) {
        if let Ok(mut st) = self.shared_state.lock() {
            for (id, val) in st.edited_params.drain(..) {
                for (cid, cval) in &mut self.param_values {
                    if *cid == id {
                        *cval = val;
                    }
                }
            }
        }

        let mut fired_timers = Vec::new();
        if let Ok(mut timers) = self.plug_frame.run_loop.timer_handlers.lock() {
            for (interval, accum, handler) in timers.iter_mut() {
                *accum += elapsed_ms;
                while *accum >= *interval {
                    *accum -= *interval;
                    fired_timers.push(*handler);
                }
            }
        }
        unsafe {
            for handler in fired_timers {
                if !handler.is_null() {
                    ((*(*handler).lpVtbl).onTimer)(handler as *mut c_void);
                }
            }
        }

        #[cfg(unix)]
        {
            let handlers = if let Ok(guard) = self.plug_frame.run_loop.event_handlers.lock() {
                guard.clone()
            } else {
                Vec::new()
            };
            if !handlers.is_empty() {
                unsafe {
                    let mut pollfds: Vec<libc::pollfd> = handlers
                        .iter()
                        .map(|&(fd, _)| libc::pollfd {
                            fd,
                            events: libc::POLLIN,
                            revents: 0,
                        })
                        .collect();
                    let ready = libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, 0);
                    if ready > 0 {
                        for (pfd, &(fd, handler)) in pollfds.iter().zip(handlers.iter()) {
                            if pfd.revents != 0 && !handler.is_null() {
                                ((*(*handler).lpVtbl).onFDIsSet)(handler as *mut c_void, fd);
                            }
                        }
                    }
                }
            }
        }
    }
}
