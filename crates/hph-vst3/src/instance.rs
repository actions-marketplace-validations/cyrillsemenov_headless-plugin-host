use crate::bundle::Vst3BundleInner;
use crate::component_handler::{SharedHostState, Vst3ComponentHandler};
use crate::events_params::{Vst3EventList, Vst3ParameterChanges};
use crate::host_app::Vst3HostApplication;
use crate::plug_frame::Vst3PlugFrame;
use crate::stream::Vst3MemoryStream;
use hph_core::*;
use hph_vst3_sys::*;
use std::ptr;
use std::sync::{Arc, Mutex};

pub struct Vst3PluginInstance {
    pub(crate) descriptor: PluginDescriptorInfo,
    pub(crate) plug_view: Option<ComPtr<Steinberg_IPlugView>>,
    pub(crate) comp_cp: Option<ComPtr<Steinberg_Vst_IConnectionPoint>>,
    pub(crate) ctrl_cp: Option<ComPtr<Steinberg_Vst_IConnectionPoint>>,
    pub(crate) controller: Option<ComPtr<Steinberg_Vst_IEditController>>,
    pub(crate) processor: ComPtr<Steinberg_Vst_IAudioProcessor>,
    pub(crate) component: ComPtr<Steinberg_Vst_IComponent>,
    pub(crate) is_single_component: bool,
    pub(crate) _host_app: Box<Vst3HostApplication>,
    pub(crate) _comp_handler: Box<Vst3ComponentHandler>,
    pub(crate) plug_frame: Box<Vst3PlugFrame>,
    pub(crate) shared_state: Arc<Mutex<SharedHostState>>,
    pub(crate) gui_attached: bool,
    pub(crate) is_active: bool,
    pub(crate) is_processing: bool,
    pub(crate) sample_rate: f64,
    pub(crate) max_frames: usize,
    pub(crate) param_values: Vec<(u32, f64)>,
    pub(crate) in_bus_channel_counts: Vec<usize>,
    pub(crate) out_bus_channel_counts: Vec<usize>,
    pub(crate) scratch_in_ptrs: Vec<*mut f32>,
    pub(crate) scratch_out_ptrs: Vec<*mut f32>,
    pub(crate) scratch_dummy_in: Vec<Vec<f32>>,
    pub(crate) scratch_dummy_out: Vec<Vec<f32>>,
    pub(crate) scratch_in_buses: Vec<Steinberg_Vst_AudioBusBuffers>,
    pub(crate) scratch_out_buses: Vec<Steinberg_Vst_AudioBusBuffers>,
    pub(crate) in_param_changes: Box<Vst3ParameterChanges>,
    pub(crate) out_param_changes: Box<Vst3ParameterChanges>,
    pub(crate) in_event_list: Box<Vst3EventList>,
    pub(crate) out_event_list: Box<Vst3EventList>,
    pub(crate) process_context: Steinberg_Vst_ProcessContext,
    pub(crate) _bundle: Arc<Vst3BundleInner>,
}

unsafe impl Send for Vst3PluginInstance {}

impl Vst3PluginInstance {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        bundle: Arc<Vst3BundleInner>,
        descriptor: PluginDescriptorInfo,
        component: ComPtr<Steinberg_Vst_IComponent>,
        processor: ComPtr<Steinberg_Vst_IAudioProcessor>,
        controller: Option<ComPtr<Steinberg_Vst_IEditController>>,
        is_single_component: bool,
        comp_cp: Option<ComPtr<Steinberg_Vst_IConnectionPoint>>,
        ctrl_cp: Option<ComPtr<Steinberg_Vst_IConnectionPoint>>,
        host_app: Box<Vst3HostApplication>,
        comp_handler: Box<Vst3ComponentHandler>,
        plug_frame: Box<Vst3PlugFrame>,
        shared_state: Arc<Mutex<SharedHostState>>,
    ) -> Self {
        let param_count = controller.as_ref().map_or(0, |ctrl| unsafe {
            (ctrl.getParameterCount)(ctrl.as_void()).max(0) as usize
        });

        Self {
            descriptor,
            plug_view: None,
            comp_cp,
            ctrl_cp,
            controller,
            processor,
            component,
            is_single_component,
            _host_app: host_app,
            _comp_handler: comp_handler,
            plug_frame,
            shared_state,
            gui_attached: false,
            is_active: false,
            is_processing: false,
            sample_rate: 48_000.0,
            max_frames: 4096,
            param_values: Vec::with_capacity(param_count.max(64)),
            in_bus_channel_counts: Vec::new(),
            out_bus_channel_counts: Vec::new(),
            scratch_in_ptrs: Vec::with_capacity(32),
            scratch_out_ptrs: Vec::with_capacity(32),
            scratch_dummy_in: Vec::new(),
            scratch_dummy_out: Vec::new(),
            scratch_in_buses: Vec::with_capacity(8),
            scratch_out_buses: Vec::with_capacity(8),
            in_param_changes: Vst3ParameterChanges::new_preallocated(param_count + 16),
            out_param_changes: Vst3ParameterChanges::new_preallocated(param_count + 16),
            in_event_list: Vst3EventList::new_preallocated(1024),
            out_event_list: Vst3EventList::new_preallocated(1024),
            process_context: Steinberg_Vst_ProcessContext {
                state: Steinberg_Vst_ProcessContext_StatesAndFlags_kPlaying
                    | Steinberg_Vst_ProcessContext_StatesAndFlags_kTempoValid
                    | Steinberg_Vst_ProcessContext_StatesAndFlags_kTimeSigValid
                    | Steinberg_Vst_ProcessContext_StatesAndFlags_kProjectTimeMusicValid
                    | Steinberg_Vst_ProcessContext_StatesAndFlags_kBarPositionValid,
                sampleRate: 48_000.0,
                tempo: 120.0,
                timeSigNumerator: 4,
                timeSigDenominator: 4,
                ..Default::default()
            },
            _bundle: bundle,
        }
    }

    pub(crate) fn refresh_param_cache(&mut self) {
        self.param_values.clear();
        let Some(ref ctrl) = self.controller else {
            return;
        };
        unsafe {
            let count = (ctrl.getParameterCount)(ctrl.as_void()).min(64);
            for i in 0..count {
                let mut info: Steinberg_Vst_ParameterInfo = std::mem::zeroed();
                if (ctrl.getParameterInfo)(ctrl.as_void(), i, &mut info) == kResultOk {
                    let val = (ctrl.getParamNormalized)(ctrl.as_void(), info.id);
                    self.param_values.push((info.id, val));
                }
            }
        }
    }

    fn update_cached_param_rt(&mut self, param_id: u32, norm_val: f64) {
        for (id, v) in &mut self.param_values {
            if *id == param_id {
                *v = norm_val;
                return;
            }
        }
    }

    fn prepare_bus_buffers(&mut self, max_frames: usize) {
        self.max_frames = max_frames.max(256);
        self.in_bus_channel_counts.clear();
        self.out_bus_channel_counts.clear();
        self.scratch_in_buses.clear();
        self.scratch_out_buses.clear();

        unsafe {
            let in_buses = (self.component.getBusCount)(
                self.component.as_void(),
                Steinberg_Vst_MediaTypes_kAudio,
                Steinberg_Vst_BusDirections_kInput,
            );
            let mut total_in_ch = 0usize;
            for i in 0..in_buses {
                let mut info: Steinberg_Vst_BusInfo = std::mem::zeroed();
                if (self.component.getBusInfo)(
                    self.component.as_void(),
                    Steinberg_Vst_MediaTypes_kAudio,
                    Steinberg_Vst_BusDirections_kInput,
                    i,
                    &mut info,
                ) == kResultOk
                {
                    let ch = info.channelCount.max(0) as usize;
                    self.in_bus_channel_counts.push(ch);
                    total_in_ch += ch;
                    self.scratch_in_buses.push(Steinberg_Vst_AudioBusBuffers {
                        numChannels: ch as i32,
                        silenceFlags: 0,
                        buffers: Steinberg_Vst_AudioBusBuffers_Buffers {
                            channelBuffers32: ptr::null_mut(),
                        },
                    });
                }
            }

            let out_buses = (self.component.getBusCount)(
                self.component.as_void(),
                Steinberg_Vst_MediaTypes_kAudio,
                Steinberg_Vst_BusDirections_kOutput,
            );
            let mut total_out_ch = 0usize;
            for i in 0..out_buses {
                let mut info: Steinberg_Vst_BusInfo = std::mem::zeroed();
                if (self.component.getBusInfo)(
                    self.component.as_void(),
                    Steinberg_Vst_MediaTypes_kAudio,
                    Steinberg_Vst_BusDirections_kOutput,
                    i,
                    &mut info,
                ) == kResultOk
                {
                    let ch = info.channelCount.max(0) as usize;
                    self.out_bus_channel_counts.push(ch);
                    total_out_ch += ch;
                    self.scratch_out_buses.push(Steinberg_Vst_AudioBusBuffers {
                        numChannels: ch as i32,
                        silenceFlags: 0,
                        buffers: Steinberg_Vst_AudioBusBuffers_Buffers {
                            channelBuffers32: ptr::null_mut(),
                        },
                    });
                }
            }

            self.scratch_in_ptrs = vec![ptr::null_mut(); total_in_ch.max(8)];
            self.scratch_out_ptrs = vec![ptr::null_mut(); total_out_ch.max(8)];
            self.scratch_dummy_in = (0..total_in_ch.max(8))
                .map(|_| vec![0.0f32; self.max_frames])
                .collect();
            self.scratch_dummy_out = (0..total_out_ch.max(8))
                .map(|_| vec![0.0f32; self.max_frames])
                .collect();
        }
    }
}

impl PluginInstance for Vst3PluginInstance {
    fn descriptor(&self) -> &PluginDescriptorInfo {
        &self.descriptor
    }

    fn audio_ports(&self) -> Vec<AudioPortInfo> {
        let mut ports = Vec::new();
        unsafe {
            let supports_f64 = (self.processor.canProcessSampleSize)(
                self.processor.as_void(),
                Steinberg_Vst_SymbolicSampleSizes_kSample64,
            ) == kResultOk;

            for &(dir, is_input) in &[
                (Steinberg_Vst_BusDirections_kInput, true),
                (Steinberg_Vst_BusDirections_kOutput, false),
            ] {
                let count = (self.component.getBusCount)(
                    self.component.as_void(),
                    Steinberg_Vst_MediaTypes_kAudio,
                    dir,
                );
                for idx in 0..count {
                    let mut info: Steinberg_Vst_BusInfo = std::mem::zeroed();
                    if (self.component.getBusInfo)(
                        self.component.as_void(),
                        Steinberg_Vst_MediaTypes_kAudio,
                        dir,
                        idx,
                        &mut info,
                    ) == kResultOk
                    {
                        let ch = info.channelCount.max(0) as u32;
                        let port_type = match ch {
                            1 => "mono".to_string(),
                            2 => "stereo".to_string(),
                            n => format!("{}ch", n),
                        };
                        ports.push(AudioPortInfo {
                            id: idx as u32,
                            index: idx as u32,
                            name: utf16_to_string(&info.name),
                            is_input,
                            channel_count: ch,
                            is_main: info.busType == Steinberg_Vst_BusTypes_kMain,
                            port_type,
                            in_place_pair_id: None,
                            prefers_64bit: supports_f64,
                        });
                    }
                }
            }
        }
        ports
    }

    fn note_ports(&self) -> Vec<NotePortInfo> {
        let mut ports = Vec::new();
        unsafe {
            for &(dir, is_input) in &[
                (Steinberg_Vst_BusDirections_kInput, true),
                (Steinberg_Vst_BusDirections_kOutput, false),
            ] {
                let count = (self.component.getBusCount)(
                    self.component.as_void(),
                    Steinberg_Vst_MediaTypes_kEvent,
                    dir,
                );
                for idx in 0..count {
                    let mut info: Steinberg_Vst_BusInfo = std::mem::zeroed();
                    if (self.component.getBusInfo)(
                        self.component.as_void(),
                        Steinberg_Vst_MediaTypes_kEvent,
                        dir,
                        idx,
                        &mut info,
                    ) == kResultOk
                    {
                        ports.push(NotePortInfo {
                            id: idx as u32,
                            index: idx as u32,
                            name: utf16_to_string(&info.name),
                            is_input,
                            supported_dialects: 1,
                            preferred_dialect: 1,
                        });
                    }
                }
            }
        }
        ports
    }

    fn params(&self) -> Vec<ParamInfo> {
        let mut list = Vec::new();
        let Some(ref ctrl) = self.controller else {
            return list;
        };
        unsafe {
            let count = (ctrl.getParameterCount)(ctrl.as_void());
            for idx in 0..count {
                let mut info: Steinberg_Vst_ParameterInfo = std::mem::zeroed();
                if (ctrl.getParameterInfo)(ctrl.as_void(), idx, &mut info) == kResultOk {
                    let flags = info.flags;
                    let units = utf16_to_string(&info.units);
                    let module = if units.is_empty() {
                        format!("Unit {}", info.unitId)
                    } else {
                        format!("Unit {} ({})", info.unitId, units)
                    };
                    list.push(ParamInfo {
                        id: info.id,
                        index: idx as u32,
                        name: utf16_to_string(&info.title),
                        module,
                        min_value: 0.0,
                        max_value: 1.0,
                        default_value: info.defaultNormalizedValue.clamp(0.0, 1.0),
                        flags: ParamFlags {
                            is_stepped: info.stepCount > 0,
                            is_periodic: (flags
                                & Steinberg_Vst_ParameterInfo_ParameterFlags_kIsWrapAround)
                                != 0,
                            is_hidden: (flags
                                & Steinberg_Vst_ParameterInfo_ParameterFlags_kIsHidden)
                                != 0,
                            is_readonly: (flags
                                & Steinberg_Vst_ParameterInfo_ParameterFlags_kIsReadOnly)
                                != 0,
                            is_bypass: (flags
                                & Steinberg_Vst_ParameterInfo_ParameterFlags_kIsBypass)
                                != 0,
                            is_automatable: (flags
                                & Steinberg_Vst_ParameterInfo_ParameterFlags_kCanAutomate)
                                != 0,
                            is_automatable_per_note_id: false,
                            is_automatable_per_key: false,
                            is_automatable_per_channel: false,
                            is_automatable_per_port: false,
                            is_modulatable: false,
                            is_modulatable_per_note_id: false,
                            is_modulatable_per_key: false,
                            is_modulatable_per_channel: false,
                            is_modulatable_per_port: false,
                            requires_process: false,
                            raw_bits: flags as u32,
                        },
                    });
                }
            }
        }
        list
    }

    fn get_param_value(&self, param_id: u32) -> Option<f64> {
        for &(id, val) in &self.param_values {
            if id == param_id {
                return Some(val);
            }
        }
        let ctrl = self.controller.as_ref()?;
        Some(unsafe { (ctrl.getParamNormalized)(ctrl.as_void(), param_id) })
    }

    fn param_value_to_text(&self, param_id: u32, value: f64) -> Option<String> {
        let ctrl = self.controller.as_ref()?;
        unsafe {
            let mut buf: Steinberg_Vst_String128 = [0u16; 128];
            if (ctrl.getParamStringByValue)(
                ctrl.as_void(),
                param_id,
                value.clamp(0.0, 1.0),
                buf.as_mut_ptr(),
            ) == kResultOk
            {
                let text = utf16_to_string(&buf);
                if !text.is_empty() {
                    return Some(text);
                }
            }
            let plain =
                (ctrl.normalizedParamToPlain)(ctrl.as_void(), param_id, value.clamp(0.0, 1.0));
            Some(format!("{:.3}", plain))
        }
    }

    fn param_text_to_value(&self, param_id: u32, text: &str) -> Option<f64> {
        let ctrl = self.controller.as_ref()?;
        unsafe {
            let mut utf16 = [0u16; 128];
            string_to_utf16_buf(text, &mut utf16);
            let mut out_val: Steinberg_Vst_ParamValue = 0.0;
            if (ctrl.getParamValueByString)(ctrl.as_void(), param_id, utf16.as_ptr(), &mut out_val)
                == kResultOk
            {
                return Some(out_val);
            }
        }
        None
    }

    fn flush_params(
        &mut self,
        events: &[ProcessEvent],
        out_events: &mut Vec<ProcessEvent>,
    ) -> HostResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        self.in_param_changes.clear_rt();
        self.out_param_changes.clear_rt();
        self.in_event_list.events.clear();
        self.out_event_list.events.clear();

        for ev in events {
            if let ProcessEvent::ParamValue {
                sample_offset,
                param_id,
                value,
                ..
            } = *ev
            {
                let norm = value.clamp(0.0, 1.0);
                self.update_cached_param_rt(param_id, norm);
                if let Some(ref ctrl) = self.controller {
                    unsafe {
                        let _ = (ctrl.setParamNormalized)(ctrl.as_void(), param_id, norm);
                    }
                }
                self.in_param_changes
                    .push_point_rt(param_id, sample_offset as i32, norm);
            }
        }

        let was_active = self.is_active;
        let was_processing = self.is_processing;
        if !was_active {
            self.activate(self.sample_rate, 1, 512)?;
        }
        if !was_processing {
            let _ = self.start_processing();
        }

        let mut ptr_offset = 0usize;
        for (bus_idx, &ch_count) in self.in_bus_channel_counts.iter().enumerate() {
            for ch in 0..ch_count {
                let global_ch = ptr_offset + ch;
                self.scratch_in_ptrs[global_ch] = self.scratch_dummy_in[global_ch].as_mut_ptr();
            }
            self.scratch_in_buses[bus_idx].numChannels = ch_count as i32;
            self.scratch_in_buses[bus_idx].silenceFlags = 0;
            self.scratch_in_buses[bus_idx].buffers.channelBuffers32 =
                unsafe { self.scratch_in_ptrs.as_mut_ptr().add(ptr_offset) };
            ptr_offset += ch_count;
        }

        let mut out_ptr_offset = 0usize;
        for (bus_idx, &ch_count) in self.out_bus_channel_counts.iter().enumerate() {
            for ch in 0..ch_count {
                let global_ch = out_ptr_offset + ch;
                self.scratch_out_ptrs[global_ch] = self.scratch_dummy_out[global_ch].as_mut_ptr();
            }
            self.scratch_out_buses[bus_idx].numChannels = ch_count as i32;
            self.scratch_out_buses[bus_idx].silenceFlags = 0;
            self.scratch_out_buses[bus_idx].buffers.channelBuffers32 =
                unsafe { self.scratch_out_ptrs.as_mut_ptr().add(out_ptr_offset) };
            out_ptr_offset += ch_count;
        }

        unsafe {
            let mut data = Steinberg_Vst_ProcessData {
                processMode: Steinberg_Vst_ProcessModes_kRealtime,
                symbolicSampleSize: Steinberg_Vst_SymbolicSampleSizes_kSample32,
                numSamples: 0,
                numInputs: self.scratch_in_buses.len() as i32,
                numOutputs: self.scratch_out_buses.len() as i32,
                inputs: if self.scratch_in_buses.is_empty() {
                    ptr::null_mut()
                } else {
                    self.scratch_in_buses.as_mut_ptr()
                },
                outputs: if self.scratch_out_buses.is_empty() {
                    ptr::null_mut()
                } else {
                    self.scratch_out_buses.as_mut_ptr()
                },
                inputParameterChanges: self.in_param_changes.as_ptr(),
                outputParameterChanges: self.out_param_changes.as_ptr(),
                inputEvents: self.in_event_list.as_ptr(),
                outputEvents: self.out_event_list.as_ptr(),
                processContext: &mut self.process_context,
            };
            let _ = (self.processor.process)(self.processor.as_void(), &mut data);
        }

        for i in 0..self.out_param_changes.active_count {
            let q = &self.out_param_changes.queues[i];
            for &(offset, val) in &q.points {
                out_events.push(ProcessEvent::ParamValue {
                    sample_offset: offset.max(0) as u32,
                    param_id: q.param_id,
                    note_id: -1,
                    port_index: 0,
                    channel: 0,
                    key: 0,
                    value: val,
                });
            }
        }

        if !was_processing {
            self.stop_processing();
        }
        if !was_active {
            self.deactivate();
        }

        Ok(())
    }

    fn activate(&mut self, sample_rate: f64, _min_frames: u32, max_frames: u32) -> HostResult<()> {
        if self.is_active {
            self.deactivate();
        }
        self.sample_rate = sample_rate;
        self.process_context.sampleRate = sample_rate;
        self.prepare_bus_buffers(max_frames as usize);

        unsafe {
            let mut setup = Steinberg_Vst_ProcessSetup {
                processMode: Steinberg_Vst_ProcessModes_kRealtime,
                symbolicSampleSize: Steinberg_Vst_SymbolicSampleSizes_kSample32,
                maxSamplesPerBlock: max_frames as i32,
                sampleRate: sample_rate,
            };
            let res = (self.processor.setupProcessing)(self.processor.as_void(), &mut setup);
            if res != kResultOk {
                return Err(HostError::ActivationError(format!(
                    "IAudioProcessor::setupProcessing failed with tresult {}",
                    res
                )));
            }

            let act_res = (self.component.setActive)(self.component.as_void(), 1);
            if act_res != kResultOk && act_res != kResultFalse {
                return Err(HostError::ActivationError(format!(
                    "IComponent::setActive(true) failed with tresult {}",
                    act_res
                )));
            }
        }

        self.is_active = true;
        Ok(())
    }

    fn deactivate(&mut self) {
        if self.is_processing {
            self.stop_processing();
        }
        if self.is_active {
            unsafe {
                let _ = (self.component.setActive)(self.component.as_void(), 0);
            }
            self.is_active = false;
        }
    }

    fn start_processing(&mut self) -> HostResult<()> {
        if !self.is_active {
            return Err(HostError::ProcessError(
                "Cannot start processing before plugin is activated".into(),
            ));
        }
        if !self.is_processing {
            unsafe {
                let res = (self.processor.setProcessing)(self.processor.as_void(), 1);
                if res != kResultOk && res != kNotImplemented && res != kResultFalse {
                    return Err(HostError::ProcessError(format!(
                        "IAudioProcessor::setProcessing(true) failed with tresult {}",
                        res
                    )));
                }
            }
            self.is_processing = true;
        }
        Ok(())
    }

    fn stop_processing(&mut self) {
        if self.is_processing {
            unsafe {
                let _ = (self.processor.setProcessing)(self.processor.as_void(), 0);
            }
            self.is_processing = false;
        }
    }

    fn reset(&mut self) {
        if self.is_active {
            let was_processing = self.is_processing;
            if was_processing {
                self.stop_processing();
            }
            unsafe {
                let _ = (self.component.setActive)(self.component.as_void(), 0);
                let _ = (self.component.setActive)(self.component.as_void(), 1);
            }
            if was_processing {
                let _ = self.start_processing();
            }
        }
    }

    fn process(
        &mut self,
        buffers: &mut AudioProcessBuffers<'_>,
        in_events: &[ProcessEvent],
        out_events: &mut Vec<ProcessEvent>,
    ) -> HostResult<ProcessStatus> {
        let num_frames = buffers.num_frames;
        self.in_param_changes.clear_rt();
        self.out_param_changes.clear_rt();
        self.in_event_list.events.clear();
        self.out_event_list.events.clear();

        for ev in in_events {
            match *ev {
                ProcessEvent::NoteOn {
                    sample_offset,
                    port_index,
                    channel,
                    key,
                    note_id,
                    velocity,
                } => {
                    if self.in_event_list.events.len() < self.in_event_list.events.capacity() {
                        self.in_event_list.events.push(Steinberg_Vst_Event {
                            busIndex: port_index as i32,
                            sampleOffset: sample_offset as i32,
                            ppqPosition: 0.0,
                            flags: 1,
                            r#type: Steinberg_Vst_Event_EventTypes_kNoteOnEvent,
                            event: Steinberg_Vst_Event_Data {
                                noteOn: Steinberg_Vst_NoteOnEvent {
                                    channel,
                                    pitch: key,
                                    tuning: 0.0,
                                    velocity: velocity as f32,
                                    length: 0,
                                    noteId: note_id,
                                },
                            },
                        });
                    }
                }
                ProcessEvent::NoteOff {
                    sample_offset,
                    port_index,
                    channel,
                    key,
                    note_id,
                    velocity,
                } => {
                    if self.in_event_list.events.len() < self.in_event_list.events.capacity() {
                        self.in_event_list.events.push(Steinberg_Vst_Event {
                            busIndex: port_index as i32,
                            sampleOffset: sample_offset as i32,
                            ppqPosition: 0.0,
                            flags: 1,
                            r#type: Steinberg_Vst_Event_EventTypes_kNoteOffEvent,
                            event: Steinberg_Vst_Event_Data {
                                noteOff: Steinberg_Vst_NoteOffEvent {
                                    channel,
                                    pitch: key,
                                    velocity: velocity as f32,
                                    noteId: note_id,
                                    tuning: 0.0,
                                },
                            },
                        });
                    }
                }
                ProcessEvent::ParamValue {
                    sample_offset,
                    param_id,
                    value,
                    ..
                } => {
                    let norm = value.clamp(0.0, 1.0);
                    self.update_cached_param_rt(param_id, norm);
                    self.in_param_changes
                        .push_point_rt(param_id, sample_offset as i32, norm);
                }
                ProcessEvent::ParamMod { .. } => {}
                ProcessEvent::Midi1 {
                    sample_offset,
                    port_index,
                    data,
                } => {
                    let status = data[0] & 0xF0;
                    let channel = (data[0] & 0x0F) as i16;
                    if status == 0x90 && data[2] > 0 {
                        if self.in_event_list.events.len() < self.in_event_list.events.capacity() {
                            self.in_event_list.events.push(Steinberg_Vst_Event {
                                busIndex: port_index as i32,
                                sampleOffset: sample_offset as i32,
                                ppqPosition: 0.0,
                                flags: 1,
                                r#type: Steinberg_Vst_Event_EventTypes_kNoteOnEvent,
                                event: Steinberg_Vst_Event_Data {
                                    noteOn: Steinberg_Vst_NoteOnEvent {
                                        channel,
                                        pitch: data[1] as i16,
                                        tuning: 0.0,
                                        velocity: (data[2] as f32) / 127.0,
                                        length: 0,
                                        noteId: -1,
                                    },
                                },
                            });
                        }
                    } else if (status == 0x80 || (status == 0x90 && data[2] == 0))
                        && self.in_event_list.events.len() < self.in_event_list.events.capacity()
                    {
                        self.in_event_list.events.push(Steinberg_Vst_Event {
                            busIndex: port_index as i32,
                            sampleOffset: sample_offset as i32,
                            ppqPosition: 0.0,
                            flags: 1,
                            r#type: Steinberg_Vst_Event_EventTypes_kNoteOffEvent,
                            event: Steinberg_Vst_Event_Data {
                                noteOff: Steinberg_Vst_NoteOffEvent {
                                    channel,
                                    pitch: data[1] as i16,
                                    velocity: (data[2] as f32) / 127.0,
                                    noteId: -1,
                                    tuning: 0.0,
                                },
                            },
                        });
                    }
                }
            }
        }

        let mut ptr_offset = 0usize;
        for (bus_idx, &ch_count) in self.in_bus_channel_counts.iter().enumerate() {
            for ch in 0..ch_count {
                let global_ch = ptr_offset + ch;
                let ptr = if bus_idx == 0 && ch < buffers.inputs.len() {
                    buffers.inputs[ch].as_ptr() as *mut f32
                } else {
                    self.scratch_dummy_in[global_ch].as_mut_ptr()
                };
                self.scratch_in_ptrs[global_ch] = ptr;
            }
            self.scratch_in_buses[bus_idx].numChannels = ch_count as i32;
            self.scratch_in_buses[bus_idx].silenceFlags = 0;
            self.scratch_in_buses[bus_idx].buffers.channelBuffers32 =
                unsafe { self.scratch_in_ptrs.as_mut_ptr().add(ptr_offset) };
            ptr_offset += ch_count;
        }

        let mut out_ptr_offset = 0usize;
        for (bus_idx, &ch_count) in self.out_bus_channel_counts.iter().enumerate() {
            for ch in 0..ch_count {
                let global_ch = out_ptr_offset + ch;
                let ptr = if bus_idx == 0 && ch < buffers.outputs.len() {
                    buffers.outputs[ch].as_mut_ptr()
                } else {
                    self.scratch_dummy_out[global_ch].as_mut_ptr()
                };
                self.scratch_out_ptrs[global_ch] = ptr;
            }
            self.scratch_out_buses[bus_idx].numChannels = ch_count as i32;
            self.scratch_out_buses[bus_idx].silenceFlags = 0;
            self.scratch_out_buses[bus_idx].buffers.channelBuffers32 =
                unsafe { self.scratch_out_ptrs.as_mut_ptr().add(out_ptr_offset) };
            out_ptr_offset += ch_count;
        }

        self.process_context.projectTimeSamples = buffers.steady_time;
        self.process_context.continousTimeSamples = buffers.steady_time;
        let beats_per_sec = self.process_context.tempo / 60.0;
        self.process_context.projectTimeMusic =
            (buffers.steady_time as f64 / self.sample_rate) * beats_per_sec;

        let res = unsafe {
            let mut data = Steinberg_Vst_ProcessData {
                processMode: Steinberg_Vst_ProcessModes_kRealtime,
                symbolicSampleSize: Steinberg_Vst_SymbolicSampleSizes_kSample32,
                numSamples: num_frames as i32,
                numInputs: self.scratch_in_buses.len() as i32,
                numOutputs: self.scratch_out_buses.len() as i32,
                inputs: if self.scratch_in_buses.is_empty() {
                    ptr::null_mut()
                } else {
                    self.scratch_in_buses.as_mut_ptr()
                },
                outputs: if self.scratch_out_buses.is_empty() {
                    ptr::null_mut()
                } else {
                    self.scratch_out_buses.as_mut_ptr()
                },
                inputParameterChanges: self.in_param_changes.as_ptr(),
                outputParameterChanges: self.out_param_changes.as_ptr(),
                inputEvents: self.in_event_list.as_ptr(),
                outputEvents: self.out_event_list.as_ptr(),
                processContext: &mut self.process_context,
            };
            (self.processor.process)(self.processor.as_void(), &mut data)
        };

        let _ = out_events;
        if res == kResultOk || res == kResultFalse {
            Ok(ProcessStatus::Continue)
        } else {
            Err(HostError::ProcessError(format!(
                "IAudioProcessor::process returned tresult {}",
                res
            )))
        }
    }

    fn supports_state(&self) -> bool {
        true
    }

    fn save_state(&self) -> HostResult<Vec<u8>> {
        unsafe {
            let mut comp_stream = Vst3MemoryStream::new_empty();
            let comp_res =
                (self.component.getState)(self.component.as_void(), comp_stream.as_ibstream_ptr());

            let mut ctrl_stream = Vst3MemoryStream::new_empty();
            if let Some(ref ctrl) = self.controller {
                if !self.is_single_component {
                    let _ = (ctrl.getState)(ctrl.as_void(), ctrl_stream.as_ibstream_ptr());
                }
            }

            if comp_res != kResultOk
                && comp_res != kResultFalse
                && comp_res != kNotImplemented
                && comp_stream.data.is_empty()
                && ctrl_stream.data.is_empty()
            {
                return Err(HostError::StateError(format!(
                    "IComponent::getState failed with tresult {}",
                    comp_res
                )));
            }

            let mut out = Vec::with_capacity(12 + comp_stream.data.len() + ctrl_stream.data.len());
            out.extend_from_slice(b"HPHV");
            out.extend_from_slice(&(comp_stream.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&comp_stream.data);
            out.extend_from_slice(&(ctrl_stream.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&ctrl_stream.data);
            Ok(out)
        }
    }

    fn load_state(&mut self, data: &[u8]) -> HostResult<()> {
        if data.len() < 12 || &data[0..4] != b"HPHV" {
            return Err(HostError::StateError(
                "Invalid VST3 state blob header".into(),
            ));
        }
        let comp_len = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        if 8 + comp_len + 4 > data.len() {
            return Err(HostError::StateError("Truncated VST3 state blob".into()));
        }
        let comp_bytes = &data[8..8 + comp_len];
        let ctrl_offset = 8 + comp_len;
        let ctrl_len = u32::from_le_bytes([
            data[ctrl_offset],
            data[ctrl_offset + 1],
            data[ctrl_offset + 2],
            data[ctrl_offset + 3],
        ]) as usize;
        if ctrl_offset + 4 + ctrl_len > data.len() {
            return Err(HostError::StateError(
                "Truncated VST3 controller state blob".into(),
            ));
        }
        let ctrl_bytes = &data[ctrl_offset + 4..ctrl_offset + 4 + ctrl_len];

        unsafe {
            if !comp_bytes.is_empty() {
                let mut comp_stream = Vst3MemoryStream::from_slice(comp_bytes);
                let res = (self.component.setState)(
                    self.component.as_void(),
                    comp_stream.as_ibstream_ptr(),
                );
                if res != kResultOk && res != kResultFalse {
                    return Err(HostError::StateError(format!(
                        "IComponent::setState failed with tresult {}",
                        res
                    )));
                }

                if let Some(ref ctrl) = self.controller {
                    comp_stream.cursor = 0;
                    let _ = (ctrl.setComponentState)(ctrl.as_void(), comp_stream.as_ibstream_ptr());
                }
            }

            if let Some(ref ctrl) = self.controller {
                if !ctrl_bytes.is_empty() {
                    let mut ctrl_stream = Vst3MemoryStream::from_slice(ctrl_bytes);
                    let _ = (ctrl.setState)(ctrl.as_void(), ctrl_stream.as_ibstream_ptr());
                }
            }
        }

        self.refresh_param_cache();
        Ok(())
    }

    fn latency_samples(&self) -> Option<u32> {
        unsafe { Some((self.processor.getLatencySamples)(self.processor.as_void())) }
    }

    fn tail_samples(&self) -> Option<u32> {
        unsafe { Some((self.processor.getTailSamples)(self.processor.as_void())) }
    }

    fn take_diagnostics(&mut self) -> HostDiagnostics {
        if let Ok(mut st) = self.shared_state.lock() {
            let logs = std::mem::take(&mut st.logs);
            let restart = st.restart_requested;
            st.restart_requested = false;
            HostDiagnostics {
                logs,
                restart_requested: restart,
                ..HostDiagnostics::default()
            }
        } else {
            HostDiagnostics::default()
        }
    }
}

impl Drop for Vst3PluginInstance {
    fn drop(&mut self) {
        self.gui_destroy();
        self.deactivate();

        unsafe {
            if let (Some(comp_cp), Some(ctrl_cp)) = (self.comp_cp.take(), self.ctrl_cp.take()) {
                let _ = (comp_cp.disconnect)(comp_cp.as_void(), ctrl_cp.as_ptr());
                let _ = (ctrl_cp.disconnect)(ctrl_cp.as_void(), comp_cp.as_ptr());
            }
            self.comp_cp = None;
            self.ctrl_cp = None;

            if let Some(ctrl) = self.controller.take() {
                let _ = (ctrl.setComponentHandler)(ctrl.as_void(), ptr::null_mut());
                if !self.is_single_component {
                    let _ = (ctrl.terminate)(ctrl.as_void());
                }
            }

            let _ = (self.component.terminate)(self.component.as_void());
        }
    }
}
