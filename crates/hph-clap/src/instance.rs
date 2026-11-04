use crate::bundle::{cstr_to_string, fixed_cstr_to_string, ClapBundleInner};
use crate::events::{
    input_events_get, input_events_size, istream_read, ostream_write, output_events_try_push,
    ClapInputEventQueue, SliceReader,
};
use crate::host::ClapHostContext;
use hph_clap_sys::*;
use hph_core::*;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CString};
use std::ptr;
use std::sync::Arc;

pub struct ClapPluginInstance {
    pub(crate) _bundle: Arc<ClapBundleInner>,
    pub(crate) host_ctx: Box<ClapHostContext>,
    pub(crate) plugin: *const clap_plugin_t,
    pub(crate) descriptor: PluginDescriptorInfo,
    pub(crate) is_active: bool,
    pub(crate) is_processing: bool,
    pub(crate) gui_created: bool,
    pub(crate) param_cookies: HashMap<u32, *mut c_void>,
    pub(crate) scratch_in_ptrs: Vec<*mut f32>,
    pub(crate) scratch_out_ptrs: Vec<*mut f32>,
    pub(crate) scratch_in_events: ClapInputEventQueue,
}

unsafe impl Send for ClapPluginInstance {}

impl Drop for ClapPluginInstance {
    fn drop(&mut self) {
        if self.gui_created {
            self.gui_destroy();
        }
        if self.is_processing {
            self.stop_processing();
        }
        if self.is_active {
            self.deactivate();
        }
        unsafe {
            if !self.plugin.is_null() {
                if let Some(destroy) = (*self.plugin).destroy {
                    destroy(self.plugin);
                }
            }
        }
    }
}

impl ClapPluginInstance {
    pub(crate) unsafe fn get_ext<T: ClapExtension>(&self) -> Option<&T> {
        let get_ext_fn = (*self.plugin).get_extension?;
        let ptr = get_ext_fn(self.plugin, T::ID.as_ptr() as *const c_char);
        if ptr.is_null() {
            None
        } else {
            Some(&*(ptr as *const T))
        }
    }

    pub(crate) unsafe fn probe_ext<T: ClapExtension>(
        &self,
        name: &str,
        inspect: impl FnOnce(&T) -> (bool, String),
    ) -> ExtensionProbeResult {
        match self.get_ext::<T>() {
            Some(ext) => {
                let (valid_vtable, notes) = inspect(ext);
                ExtensionProbeResult {
                    name: name.to_string(),
                    supported: true,
                    valid_vtable,
                    notes,
                }
            }
            None => ExtensionProbeResult {
                name: name.to_string(),
                supported: false,
                valid_vtable: false,
                notes: String::new(),
            },
        }
    }

    pub(crate) fn refresh_param_cookies(&mut self) {
        self.param_cookies.clear();
        unsafe {
            if let Some(ext) = self.get_ext::<clap_plugin_params_t>() {
                if let (Some(count_fn), Some(info_fn)) = (ext.count, ext.get_info) {
                    let n = count_fn(self.plugin);
                    for i in 0..n {
                        let mut info: clap_param_info_t = std::mem::zeroed();
                        if info_fn(self.plugin, i, &mut info) {
                            self.param_cookies.insert(info.id, info.cookie);
                        }
                    }
                }
            }
        }
    }
}

impl PluginInstance for ClapPluginInstance {
    fn descriptor(&self) -> &PluginDescriptorInfo {
        &self.descriptor
    }

    fn audio_ports(&self) -> Vec<AudioPortInfo> {
        let mut ports = Vec::new();
        unsafe {
            if let Some(ext) = self.get_ext::<clap_plugin_audio_ports_t>() {
                if let (Some(count_fn), Some(get_fn)) = (ext.count, ext.get) {
                    for is_input in [true, false] {
                        let n = count_fn(self.plugin, is_input);
                        for idx in 0..n {
                            let mut raw: clap_audio_port_info_t = std::mem::zeroed();
                            if get_fn(self.plugin, idx, is_input, &mut raw) {
                                ports.push(AudioPortInfo {
                                    id: raw.id,
                                    index: idx,
                                    name: fixed_cstr_to_string(&raw.name),
                                    is_input,
                                    channel_count: raw.channel_count,
                                    is_main: (raw.flags & CLAP_AUDIO_PORT_IS_MAIN) != 0,
                                    port_type: cstr_to_string(raw.port_type),
                                    in_place_pair_id: if raw.in_place_pair == CLAP_INVALID_ID {
                                        None
                                    } else {
                                        Some(raw.in_place_pair)
                                    },
                                    prefers_64bit: (raw.flags & CLAP_AUDIO_PORT_PREFERS_64BITS)
                                        != 0,
                                });
                            }
                        }
                    }
                }
            }
        }
        ports
    }

    fn note_ports(&self) -> Vec<NotePortInfo> {
        let mut ports = Vec::new();
        unsafe {
            if let Some(ext) = self.get_ext::<clap_plugin_note_ports_t>() {
                if let (Some(count_fn), Some(get_fn)) = (ext.count, ext.get) {
                    for is_input in [true, false] {
                        let n = count_fn(self.plugin, is_input);
                        for idx in 0..n {
                            let mut raw: clap_note_port_info_t = std::mem::zeroed();
                            if get_fn(self.plugin, idx, is_input, &mut raw) {
                                ports.push(NotePortInfo {
                                    id: raw.id,
                                    index: idx,
                                    name: fixed_cstr_to_string(&raw.name),
                                    is_input,
                                    supported_dialects: raw.supported_dialects,
                                    preferred_dialect: raw.preferred_dialect,
                                });
                            }
                        }
                    }
                }
            }
        }
        ports
    }

    fn params(&self) -> Vec<ParamInfo> {
        let mut list = Vec::new();
        unsafe {
            if let Some(ext) = self.get_ext::<clap_plugin_params_t>() {
                if let (Some(count_fn), Some(info_fn)) = (ext.count, ext.get_info) {
                    let n = count_fn(self.plugin);
                    for idx in 0..n {
                        let mut raw: clap_param_info_t = std::mem::zeroed();
                        if info_fn(self.plugin, idx, &mut raw) {
                            let f = raw.flags;
                            list.push(ParamInfo {
                                id: raw.id,
                                index: idx,
                                name: fixed_cstr_to_string(&raw.name),
                                module: fixed_cstr_to_string(&raw.module),
                                min_value: raw.min_value,
                                max_value: raw.max_value,
                                default_value: raw.default_value,
                                flags: ParamFlags {
                                    is_stepped: (f & CLAP_PARAM_IS_STEPPED) != 0,
                                    is_periodic: (f & CLAP_PARAM_IS_PERIODIC) != 0,
                                    is_hidden: (f & CLAP_PARAM_IS_HIDDEN) != 0,
                                    is_readonly: (f & CLAP_PARAM_IS_READONLY) != 0,
                                    is_bypass: (f & CLAP_PARAM_IS_BYPASS) != 0,
                                    is_automatable: (f & CLAP_PARAM_IS_AUTOMATABLE) != 0,
                                    is_automatable_per_note_id: (f
                                        & CLAP_PARAM_IS_AUTOMATABLE_PER_NOTE_ID)
                                        != 0,
                                    is_automatable_per_key: (f & CLAP_PARAM_IS_AUTOMATABLE_PER_KEY)
                                        != 0,
                                    is_automatable_per_channel: (f
                                        & CLAP_PARAM_IS_AUTOMATABLE_PER_CHANNEL)
                                        != 0,
                                    is_automatable_per_port: (f
                                        & CLAP_PARAM_IS_AUTOMATABLE_PER_PORT)
                                        != 0,
                                    is_modulatable: (f & CLAP_PARAM_IS_MODULATABLE) != 0,
                                    is_modulatable_per_note_id: (f
                                        & CLAP_PARAM_IS_MODULATABLE_PER_NOTE_ID)
                                        != 0,
                                    is_modulatable_per_key: (f & CLAP_PARAM_IS_MODULATABLE_PER_KEY)
                                        != 0,
                                    is_modulatable_per_channel: (f
                                        & CLAP_PARAM_IS_MODULATABLE_PER_CHANNEL)
                                        != 0,
                                    is_modulatable_per_port: (f
                                        & CLAP_PARAM_IS_MODULATABLE_PER_PORT)
                                        != 0,
                                    requires_process: (f & CLAP_PARAM_REQUIRES_PROCESS) != 0,
                                    raw_bits: f,
                                },
                            });
                        }
                    }
                }
            }
        }
        list
    }

    fn get_param_value(&self, param_id: u32) -> Option<f64> {
        unsafe {
            let ext = self.get_ext::<clap_plugin_params_t>()?;
            let get_val = ext.get_value?;
            let mut val = 0.0;
            if get_val(self.plugin, param_id, &mut val) {
                Some(val)
            } else {
                None
            }
        }
    }

    fn param_value_to_text(&self, param_id: u32, value: f64) -> Option<String> {
        unsafe {
            let ext = self.get_ext::<clap_plugin_params_t>()?;
            let v2t = ext.value_to_text?;
            let mut buf = [0 as c_char; CLAP_NAME_SIZE];
            if v2t(
                self.plugin,
                param_id,
                value,
                buf.as_mut_ptr(),
                buf.len() as u32,
            ) {
                Some(fixed_cstr_to_string(&buf))
            } else {
                None
            }
        }
    }

    fn param_text_to_value(&self, param_id: u32, text: &str) -> Option<f64> {
        unsafe {
            let ext = self.get_ext::<clap_plugin_params_t>()?;
            let t2v = ext.text_to_value?;
            let c_text = CString::new(text).ok()?;
            let mut out = 0.0;
            if t2v(self.plugin, param_id, c_text.as_ptr(), &mut out) {
                Some(out)
            } else {
                None
            }
        }
    }

    fn flush_params(
        &mut self,
        events: &[ProcessEvent],
        out_events: &mut Vec<ProcessEvent>,
    ) -> HostResult<()> {
        unsafe {
            let ext = self
                .get_ext::<clap_plugin_params_t>()
                .ok_or_else(|| HostError::Unsupported("clap.params not supported".into()))?;
            let flush_fn = ext
                .flush
                .ok_or_else(|| HostError::Unsupported("clap.params.flush is null".into()))?;

            let mut in_q = ClapInputEventQueue::from_events(events, &self.param_cookies);
            let clap_in = clap_input_events_t {
                ctx: &mut in_q as *mut _ as *mut c_void,
                size: Some(input_events_size),
                get: Some(input_events_get),
            };
            let clap_out = clap_output_events_t {
                ctx: out_events as *mut _ as *mut c_void,
                try_push: Some(output_events_try_push),
            };

            if self.is_active {
                if let Ok(mut guard) = self.host_ctx.inner.lock() {
                    guard.audio_thread_id = Some(std::thread::current().id());
                }
            }
            flush_fn(self.plugin, &clap_in, &clap_out);
            if self.is_active {
                if let Ok(mut guard) = self.host_ctx.inner.lock() {
                    guard.audio_thread_id = None;
                }
            }
            Ok(())
        }
    }

    fn activate(&mut self, sample_rate: f64, min_frames: u32, max_frames: u32) -> HostResult<()> {
        if self.is_active {
            return Ok(());
        }
        unsafe {
            let act_fn = (*self.plugin).activate.ok_or_else(|| {
                HostError::ActivationError("`plugin.activate` pointer is null".into())
            })?;
            if !act_fn(self.plugin, sample_rate, min_frames, max_frames) {
                return Err(HostError::ActivationError(
                    "`plugin.activate()` returned false".into(),
                ));
            }
            self.is_active = true;
            Ok(())
        }
    }

    fn deactivate(&mut self) {
        if self.is_processing {
            self.stop_processing();
        }
        if !self.is_active {
            return;
        }
        unsafe {
            if let Some(deact_fn) = (*self.plugin).deactivate {
                deact_fn(self.plugin);
            }
        }
        self.is_active = false;
    }

    fn start_processing(&mut self) -> HostResult<()> {
        if !self.is_active {
            return Err(HostError::ProcessError(
                "Cannot call start_processing while plugin is deactivated".into(),
            ));
        }
        if self.is_processing {
            return Ok(());
        }
        unsafe {
            if let Ok(mut guard) = self.host_ctx.inner.lock() {
                guard.audio_thread_id = Some(std::thread::current().id());
            }
            let res = if let Some(start_fn) = (*self.plugin).start_processing {
                start_fn(self.plugin)
            } else {
                true
            };
            if let Ok(mut guard) = self.host_ctx.inner.lock() {
                guard.audio_thread_id = None;
            }
            if !res {
                return Err(HostError::ProcessError(
                    "`plugin.start_processing()` returned false".into(),
                ));
            }
            self.is_processing = true;
            Ok(())
        }
    }

    fn stop_processing(&mut self) {
        if !self.is_processing {
            return;
        }
        unsafe {
            if let Ok(mut guard) = self.host_ctx.inner.lock() {
                guard.audio_thread_id = Some(std::thread::current().id());
            }
            if let Some(stop_fn) = (*self.plugin).stop_processing {
                stop_fn(self.plugin);
            }
            if let Ok(mut guard) = self.host_ctx.inner.lock() {
                guard.audio_thread_id = None;
            }
        }
        self.is_processing = false;
    }

    fn reset(&mut self) {
        if !self.is_active {
            return;
        }
        unsafe {
            if let Ok(mut guard) = self.host_ctx.inner.lock() {
                guard.audio_thread_id = Some(std::thread::current().id());
            }
            if let Some(reset_fn) = (*self.plugin).reset {
                reset_fn(self.plugin);
            }
            if let Ok(mut guard) = self.host_ctx.inner.lock() {
                guard.audio_thread_id = None;
            }
        }
    }

    fn process(
        &mut self,
        buffers: &mut AudioProcessBuffers<'_>,
        in_events: &[ProcessEvent],
        out_events: &mut Vec<ProcessEvent>,
    ) -> HostResult<ProcessStatus> {
        if !self.is_active || !self.is_processing {
            return Err(HostError::ProcessError(
                "Plugin must be active and processing before calling process()".into(),
            ));
        }

        unsafe {
            let proc_fn = (*self.plugin).process.ok_or_else(|| {
                HostError::ProcessError("`plugin.process` pointer is null".into())
            })?;

            self.scratch_in_ptrs.clear();
            for ch in buffers.inputs {
                self.scratch_in_ptrs.push(ch.as_ptr() as *mut f32);
            }
            self.scratch_out_ptrs.clear();
            for ch in buffers.outputs.iter_mut() {
                self.scratch_out_ptrs.push(ch.as_mut_ptr());
            }

            let in_audio_buf = clap_audio_buffer_t {
                data32: if self.scratch_in_ptrs.is_empty() {
                    ptr::null_mut()
                } else {
                    self.scratch_in_ptrs.as_mut_ptr()
                },
                data64: ptr::null_mut(),
                channel_count: self.scratch_in_ptrs.len() as u32,
                latency: 0,
                constant_mask: 0,
            };

            let mut out_audio_buf = clap_audio_buffer_t {
                data32: if self.scratch_out_ptrs.is_empty() {
                    ptr::null_mut()
                } else {
                    self.scratch_out_ptrs.as_mut_ptr()
                },
                data64: ptr::null_mut(),
                channel_count: self.scratch_out_ptrs.len() as u32,
                latency: 0,
                constant_mask: 0,
            };

            self.scratch_in_events
                .populate(in_events, &self.param_cookies);
            let clap_in = clap_input_events_t {
                ctx: &mut self.scratch_in_events as *mut _ as *mut c_void,
                size: Some(input_events_size),
                get: Some(input_events_get),
            };
            let clap_out = clap_output_events_t {
                ctx: out_events as *mut _ as *mut c_void,
                try_push: Some(output_events_try_push),
            };

            let transport = clap_event_transport_t {
                header: clap_event_header_t {
                    size: std::mem::size_of::<clap_event_transport_t>() as u32,
                    time: 0,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_TRANSPORT,
                    flags: 0,
                },
                flags: CLAP_TRANSPORT_HAS_TEMPO
                    | CLAP_TRANSPORT_HAS_TIME_SIGNATURE
                    | CLAP_TRANSPORT_IS_PLAYING,
                song_pos_beats: 0,
                song_pos_seconds: 0,
                tempo: 120.0,
                tempo_inc: 0.0,
                loop_start_beats: 0,
                loop_end_beats: 0,
                loop_start_seconds: 0,
                loop_end_seconds: 0,
                bar_start: 0,
                bar_number: 0,
                tsig_num: 4,
                tsig_denom: 4,
            };

            let clap_proc = clap_process_t {
                steady_time: buffers.steady_time,
                frames_count: buffers.num_frames as u32,
                transport: &transport,
                audio_inputs: if self.scratch_in_ptrs.is_empty() {
                    ptr::null()
                } else {
                    &in_audio_buf
                },
                audio_outputs: if self.scratch_out_ptrs.is_empty() {
                    ptr::null_mut()
                } else {
                    &mut out_audio_buf
                },
                audio_inputs_count: if self.scratch_in_ptrs.is_empty() {
                    0
                } else {
                    1
                },
                audio_outputs_count: if self.scratch_out_ptrs.is_empty() {
                    0
                } else {
                    1
                },
                in_events: &clap_in,
                out_events: &clap_out,
            };

            if let Ok(mut guard) = self.host_ctx.inner.lock() {
                guard.audio_thread_id = Some(std::thread::current().id());
            }

            let status = proc_fn(self.plugin, &clap_proc);

            if let Ok(mut guard) = self.host_ctx.inner.lock() {
                guard.audio_thread_id = None;
            }

            let mapped = match status {
                CLAP_PROCESS_CONTINUE => ProcessStatus::Continue,
                CLAP_PROCESS_CONTINUE_IF_NOT_QUIET => ProcessStatus::ContinueIfNotQuiet,
                CLAP_PROCESS_TAIL => ProcessStatus::Tail,
                CLAP_PROCESS_SLEEP => ProcessStatus::Sleep,
                _ => ProcessStatus::Error,
            };
            Ok(mapped)
        }
    }

    fn supports_state(&self) -> bool {
        unsafe { self.get_ext::<clap_plugin_state_t>().is_some() }
    }

    fn save_state(&self) -> HostResult<Vec<u8>> {
        unsafe {
            let ext = self
                .get_ext::<clap_plugin_state_t>()
                .ok_or_else(|| HostError::StateError("clap.state not supported".into()))?;
            let save_fn = ext
                .save
                .ok_or_else(|| HostError::StateError("clap.state.save is null".into()))?;

            let mut data = Vec::new();
            let stream = clap_ostream_t {
                ctx: &mut data as *mut _ as *mut c_void,
                write: Some(ostream_write),
            };

            if !save_fn(self.plugin, &stream) {
                return Err(HostError::StateError(
                    "`clap_plugin_state.save()` returned false".into(),
                ));
            }
            Ok(data)
        }
    }

    fn load_state(&mut self, data: &[u8]) -> HostResult<()> {
        unsafe {
            let ext = self
                .get_ext::<clap_plugin_state_t>()
                .ok_or_else(|| HostError::StateError("clap.state not supported".into()))?;
            let load_fn = ext
                .load
                .ok_or_else(|| HostError::StateError("clap.state.load is null".into()))?;

            let mut reader = SliceReader { data, pos: 0 };
            let stream = clap_istream_t {
                ctx: &mut reader as *mut _ as *mut c_void,
                read: Some(istream_read),
            };

            if !load_fn(self.plugin, &stream) {
                return Err(HostError::StateError(
                    "`clap_plugin_state.load()` returned false".into(),
                ));
            }
            Ok(())
        }
    }

    fn latency_samples(&self) -> Option<u32> {
        unsafe {
            let ext = self.get_ext::<clap_plugin_latency_t>()?;
            let get_fn = ext.get?;
            Some(get_fn(self.plugin))
        }
    }

    fn tail_samples(&self) -> Option<u32> {
        unsafe {
            let ext = self.get_ext::<clap_plugin_tail_t>()?;
            let get_fn = ext.get?;
            Some(get_fn(self.plugin))
        }
    }

    fn take_diagnostics(&mut self) -> HostDiagnostics {
        if let Ok(mut guard) = self.host_ctx.inner.lock() {
            HostDiagnostics {
                logs: std::mem::take(&mut guard.logs),
                thread_violations: std::mem::take(&mut guard.thread_violations),
                restart_requested: std::mem::replace(&mut guard.restart_requested, false),
                process_requested: std::mem::replace(&mut guard.process_requested, false),
                callback_requested: guard.callback_requested,
                flush_requested: std::mem::replace(&mut guard.flush_requested, false),
            }
        } else {
            HostDiagnostics::default()
        }
    }
}
