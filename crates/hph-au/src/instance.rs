use crate::bundle::{AuBundleInner, AuComponentEntry};
use crate::render_context::{
    au_render_input_callback, host_get_beat_and_tempo, host_get_musical_time_location,
    host_get_transport_state, AuRenderContext,
};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::data::{CFData, CFDataRef};
use core_foundation::string::CFString;
use hph_au_sys::*;
use hph_core::*;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use std::ffi::{c_char, c_void, CStr};
use std::ptr;
use std::sync::Arc;

pub struct AuPluginInstance {
    pub(crate) _bundle: Arc<AuBundleInner>,
    pub(crate) entry: AuComponentEntry,
    pub(crate) unit: AudioUnit,
    pub(crate) is_initialized: bool,
    pub(crate) sample_rate: f64,
    pub(crate) max_frames: u32,
    pub(crate) output_channels: u32,
    pub(crate) in_bus_channels: Vec<u32>,
    pub(crate) out_bus_channels: Vec<u32>,
    pub(crate) cached_params: Vec<ParamInfo>,
    pub(crate) render_ctx: Box<AuRenderContext>,
    /// Pre-allocated raw byte buffer aligned for `AudioBufferList` with up to 32 channels
    pub(crate) out_abl_storage: Vec<u64>,
    /// Pre-allocated scratch buffer for scheduled parameter events
    pub(crate) param_event_scratch: Vec<AudioUnitParameterEvent>,
    /// Retained Cocoa `NSView` created via `kAudioUnitProperty_CocoaUI`
    pub(crate) cocoa_view: Option<Retained<NSView>>,
    pub(crate) gui_size: Option<(u32, u32)>,
}

unsafe impl Send for AuPluginInstance {}

impl AuPluginInstance {
    pub(crate) fn new(
        bundle: Arc<AuBundleInner>,
        entry: AuComponentEntry,
        unit: AudioUnit,
    ) -> HostResult<Self> {
        let (in_bus_channels, out_bus_channels) =
            Self::probe_bus_channels(unit, entry.desc.componentType);
        let in_ch = in_bus_channels.first().copied().unwrap_or(0);
        let out_ch = out_bus_channels.first().copied().unwrap_or(2).max(1);
        let total_in_ch: usize = in_bus_channels.iter().map(|&c| c as usize).sum();
        let total_out_ch: usize = out_bus_channels.iter().map(|&c| c as usize).sum();

        let mut in_bus_offsets = Vec::with_capacity(in_bus_channels.len());
        let mut off = 0usize;
        for &c in &in_bus_channels {
            let cnt = c as usize;
            in_bus_offsets.push((off, cnt));
            off += cnt;
        }

        let render_ctx = Box::new(AuRenderContext {
            input_ptrs: vec![ptr::null(); total_in_ch.max(16)],
            in_bus_offsets,
            num_input_channels: in_ch as usize,
            num_frames: 0,
            sample_rate: 48000.0,
            sample_time: 0.0,
        });

        let max_ch = total_out_ch.max(out_ch as usize).max(16);
        let abl_bytes =
            std::mem::size_of::<AudioBufferList>() + max_ch * std::mem::size_of::<AudioBuffer>();
        let abl_words = abl_bytes.div_ceil(8);

        let mut instance = Self {
            _bundle: bundle,
            entry,
            unit,
            is_initialized: false,
            sample_rate: 48000.0,
            max_frames: 1024,
            output_channels: out_ch,
            in_bus_channels,
            out_bus_channels,
            cached_params: Vec::new(),
            render_ctx,
            out_abl_storage: vec![0u64; abl_words],
            param_event_scratch: Vec::with_capacity(128),
            cocoa_view: None,
            gui_size: None,
        };

        instance.setup_host_callbacks();
        instance.cached_params = instance.enumerate_parameters();
        Ok(instance)
    }

    fn setup_host_callbacks(&mut self) {
        let ctx_ptr = (&mut *self.render_ctx) as *mut AuRenderContext as *mut c_void;

        let host_cb = HostCallbackInfo {
            hostUserData: ctx_ptr,
            beatAndTempoProc: Some(host_get_beat_and_tempo),
            musicalTimeLocationProc: Some(host_get_musical_time_location),
            transportStateProc: Some(host_get_transport_state),
            transportStateProc2: None,
        };
        unsafe {
            let _ = AudioUnitSetProperty(
                self.unit,
                kAudioUnitProperty_HostCallbacks,
                kAudioUnitScope_Global,
                0,
                &host_cb as *const _ as *const c_void,
                std::mem::size_of::<HostCallbackInfo>() as u32,
            );
        }

        if !self.in_bus_channels.is_empty() {
            let render_cb = AURenderCallbackStruct {
                inputProc: Some(au_render_input_callback),
                inputProcRefCon: ctx_ptr,
            };
            for bus_idx in 0..self.in_bus_channels.len() {
                unsafe {
                    let _ = AudioUnitSetProperty(
                        self.unit,
                        kAudioUnitProperty_SetRenderCallback,
                        kAudioUnitScope_Input,
                        bus_idx as u32,
                        &render_cb as *const _ as *const c_void,
                        std::mem::size_of::<AURenderCallbackStruct>() as u32,
                    );
                }
            }
        }
    }

    fn probe_bus_channels(unit: AudioUnit, comp_type: OSType) -> (Vec<u32>, Vec<u32>) {
        unsafe {
            let mut in_buses: u32 = 0;
            let mut size = std::mem::size_of::<u32>() as u32;
            if AudioUnitGetProperty(
                unit,
                kAudioUnitProperty_ElementCount,
                kAudioUnitScope_Input,
                0,
                &mut in_buses as *mut _ as *mut c_void,
                &mut size,
            ) != noErr
            {
                in_buses = if comp_type == kAudioUnitType_MusicDevice
                    || comp_type == kAudioUnitType_Generator
                {
                    0
                } else {
                    1
                };
            }

            let mut out_buses: u32 = 0;
            size = std::mem::size_of::<u32>() as u32;
            if AudioUnitGetProperty(
                unit,
                kAudioUnitProperty_ElementCount,
                kAudioUnitScope_Output,
                0,
                &mut out_buses as *mut _ as *mut c_void,
                &mut size,
            ) != noErr
            {
                out_buses = 1;
            }

            let mut in_vec = Vec::with_capacity(in_buses as usize);
            for idx in 0..in_buses {
                let mut asbd = AudioStreamBasicDescription::default();
                let mut asbd_size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
                let ch = if AudioUnitGetProperty(
                    unit,
                    kAudioUnitProperty_StreamFormat,
                    kAudioUnitScope_Input,
                    idx,
                    &mut asbd as *mut _ as *mut c_void,
                    &mut asbd_size,
                ) == noErr
                    && asbd.mChannelsPerFrame > 0
                {
                    asbd.mChannelsPerFrame
                } else {
                    2
                };
                in_vec.push(ch);
            }

            let mut out_vec = Vec::with_capacity(out_buses.max(1) as usize);
            for idx in 0..out_buses.max(1) {
                let mut asbd = AudioStreamBasicDescription::default();
                let mut asbd_size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
                let ch = if AudioUnitGetProperty(
                    unit,
                    kAudioUnitProperty_StreamFormat,
                    kAudioUnitScope_Output,
                    idx,
                    &mut asbd as *mut _ as *mut c_void,
                    &mut asbd_size,
                ) == noErr
                    && asbd.mChannelsPerFrame > 0
                {
                    asbd.mChannelsPerFrame
                } else {
                    2
                };
                out_vec.push(ch);
            }

            (in_vec, out_vec)
        }
    }

    fn enumerate_parameters(&self) -> Vec<ParamInfo> {
        unsafe {
            let mut data_size: u32 = 0;
            let mut writable: Boolean = 0;
            let status = AudioUnitGetPropertyInfo(
                self.unit,
                kAudioUnitProperty_ParameterList,
                kAudioUnitScope_Global,
                0,
                &mut data_size,
                &mut writable,
            );
            if status != noErr || data_size == 0 {
                return Vec::new();
            }

            let count = (data_size as usize) / std::mem::size_of::<AudioUnitParameterID>();
            let mut param_ids = vec![0u32; count];
            let status = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_ParameterList,
                kAudioUnitScope_Global,
                0,
                param_ids.as_mut_ptr() as *mut c_void,
                &mut data_size,
            );
            if status != noErr {
                return Vec::new();
            }

            let mut params = Vec::with_capacity(count);
            for (idx, &pid) in param_ids.iter().enumerate() {
                let mut info: AudioUnitParameterInfo = std::mem::zeroed();
                let mut info_size = std::mem::size_of::<AudioUnitParameterInfo>() as u32;
                if AudioUnitGetProperty(
                    self.unit,
                    kAudioUnitProperty_ParameterInfo,
                    kAudioUnitScope_Global,
                    pid,
                    &mut info as *mut _ as *mut c_void,
                    &mut info_size,
                ) != noErr
                {
                    continue;
                }

                let name = if (info.flags & kAudioUnitParameterFlag_HasCFNameString) != 0
                    && !info.cfNameString.is_null()
                {
                    let cf_str = CFString::wrap_under_get_rule(info.cfNameString);
                    let s = cf_str.to_string();
                    if (info.flags & kAudioUnitParameterFlag_CFNameRelease) != 0 {
                        CFRelease(info.cfNameString as CFTypeRef);
                    }
                    s
                } else {
                    CStr::from_ptr(info.name.as_ptr() as *const c_char)
                        .to_string_lossy()
                        .into_owned()
                };

                if (info.flags & kAudioUnitParameterFlag_CFNameRelease) != 0
                    && info.unit == kAudioUnitParameterUnit_CustomUnit
                    && !info.unitName.is_null()
                {
                    CFRelease(info.unitName as CFTypeRef);
                }

                let module = if (info.flags & kAudioUnitParameterFlag_HasClump) != 0 {
                    format!("Clump {}", info.clumpID)
                } else {
                    String::new()
                };

                #[allow(non_upper_case_globals)]
                let is_stepped = matches!(
                    info.unit,
                    kAudioUnitParameterUnit_Indexed | kAudioUnitParameterUnit_Boolean
                );
                let is_readonly = (info.flags & kAudioUnitParameterFlag_IsWritable) == 0
                    || (info.flags & kAudioUnitParameterFlag_MeterReadOnly) != 0;
                let is_bypass = name.eq_ignore_ascii_case("bypass");

                params.push(ParamInfo {
                    id: pid,
                    index: idx as u32,
                    name,
                    module,
                    min_value: info.minValue as f64,
                    max_value: info.maxValue as f64,
                    default_value: info.defaultValue as f64,
                    flags: ParamFlags {
                        is_stepped,
                        is_periodic: false,
                        is_hidden: false,
                        is_readonly,
                        is_bypass,
                        is_automatable: !is_readonly,
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
                        raw_bits: info.flags,
                    },
                });
            }

            params
        }
    }
}

impl Drop for AuPluginInstance {
    fn drop(&mut self) {
        self.gui_destroy();
        self.deactivate();
        if !self.unit.is_null() {
            unsafe {
                let _ = AudioComponentInstanceDispose(self.unit);
            }
            self.unit = ptr::null_mut();
        }
    }
}

impl PluginInstance for AuPluginInstance {
    fn descriptor(&self) -> &PluginDescriptorInfo {
        &self.entry.descriptor
    }

    fn audio_ports(&self) -> Vec<AudioPortInfo> {
        let mut ports = Vec::new();
        let in_buses = self.in_bus_channels.len() as u32;
        let out_buses = self.out_bus_channels.len() as u32;

        for (idx_usize, &ch_raw) in self.in_bus_channels.iter().enumerate() {
            let idx = idx_usize as u32;
            let ch = ch_raw.max(1);
            ports.push(AudioPortInfo {
                id: idx,
                index: idx,
                name: if idx == 0 {
                    "Input".into()
                } else {
                    format!("Sidechain / Aux In {idx}")
                },
                is_input: true,
                channel_count: ch,
                is_main: idx == 0,
                port_type: match ch {
                    1 => "mono".into(),
                    2 => "stereo".into(),
                    n => format!("{n}ch"),
                },
                in_place_pair_id: if idx == 0 && out_buses > 0 {
                    Some(0)
                } else {
                    None
                },
                prefers_64bit: false,
            });
        }

        for (idx_usize, &ch_raw) in self.out_bus_channels.iter().enumerate() {
            let idx = idx_usize as u32;
            let ch = ch_raw.max(1);
            ports.push(AudioPortInfo {
                id: idx,
                index: idx,
                name: if idx == 0 {
                    "Output".into()
                } else {
                    format!("Aux Out {idx}")
                },
                is_input: false,
                channel_count: ch,
                is_main: idx == 0,
                port_type: match ch {
                    1 => "mono".into(),
                    2 => "stereo".into(),
                    n => format!("{n}ch"),
                },
                in_place_pair_id: if idx == 0 && in_buses > 0 {
                    Some(0)
                } else {
                    None
                },
                prefers_64bit: false,
            });
        }
        ports
    }

    fn note_ports(&self) -> Vec<NotePortInfo> {
        let ct = self.entry.desc.componentType;
        if ct == kAudioUnitType_MusicDevice
            || ct == kAudioUnitType_MusicEffect
            || ct == kAudioUnitType_MIDIProcessor
        {
            vec![NotePortInfo {
                id: 0,
                index: 0,
                name: "MIDI In".into(),
                is_input: true,
                supported_dialects: 1,
                preferred_dialect: 1,
            }]
        } else {
            Vec::new()
        }
    }

    fn params(&self) -> Vec<ParamInfo> {
        self.cached_params.clone()
    }

    fn get_param_value(&self, param_id: u32) -> Option<f64> {
        let mut val: AudioUnitParameterValue = 0.0;
        let status = unsafe {
            AudioUnitGetParameter(self.unit, param_id, kAudioUnitScope_Global, 0, &mut val)
        };
        if status == noErr {
            Some(val as f64)
        } else {
            None
        }
    }

    fn param_value_to_text(&self, param_id: u32, value: f64) -> Option<String> {
        let fval = value as AudioUnitParameterValue;
        let mut req = AudioUnitParameterStringFromValue {
            inParamID: param_id,
            inValue: &fval,
            outString: ptr::null(),
        };
        let mut size = std::mem::size_of::<AudioUnitParameterStringFromValue>() as u32;
        let status = unsafe {
            AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_ParameterStringFromValue,
                kAudioUnitScope_Global,
                0,
                &mut req as *mut _ as *mut c_void,
                &mut size,
            )
        };
        if status == noErr && !req.outString.is_null() {
            let s = unsafe { CFString::wrap_under_create_rule(req.outString).to_string() };
            if !s.is_empty() {
                return Some(s);
            }
        }
        Some(format!("{value:.2}"))
    }

    fn param_text_to_value(&self, param_id: u32, text: &str) -> Option<f64> {
        let cf_str = CFString::new(text);
        let mut req = AudioUnitParameterValueFromString {
            inParamID: param_id,
            inString: cf_str.as_concrete_TypeRef(),
            outValue: 0.0,
        };
        let mut size = std::mem::size_of::<AudioUnitParameterValueFromString>() as u32;
        let status = unsafe {
            AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_ParameterValueFromString,
                kAudioUnitScope_Global,
                0,
                &mut req as *mut _ as *mut c_void,
                &mut size,
            )
        };
        if status == noErr {
            return Some(req.outValue as f64);
        }
        let cleaned: String = text
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
            .collect();
        cleaned.parse::<f64>().ok()
    }

    fn flush_params(
        &mut self,
        events: &[ProcessEvent],
        _out_events: &mut Vec<ProcessEvent>,
    ) -> HostResult<()> {
        for ev in events {
            if let ProcessEvent::ParamValue {
                param_id, value, ..
            } = ev
            {
                unsafe {
                    let _ = AudioUnitSetParameter(
                        self.unit,
                        *param_id,
                        kAudioUnitScope_Global,
                        0,
                        *value as f32,
                        0,
                    );
                }
            }
        }
        Ok(())
    }

    fn activate(&mut self, sample_rate: f64, _min_frames: u32, max_frames: u32) -> HostResult<()> {
        if self.is_initialized {
            self.deactivate();
        }
        self.sample_rate = sample_rate;
        self.max_frames = max_frames.max(64);
        self.render_ctx.sample_rate = sample_rate;
        self.render_ctx.sample_time = 0.0;

        unsafe {
            let _ = AudioUnitSetProperty(
                self.unit,
                kAudioUnitProperty_MaximumFramesPerSlice,
                kAudioUnitScope_Global,
                0,
                &self.max_frames as *const _ as *const c_void,
                std::mem::size_of::<u32>() as u32,
            );

            for (bus_idx, &ch_count) in self.in_bus_channels.iter().enumerate() {
                if ch_count > 0 {
                    let _ = AudioUnitSetProperty(
                        self.unit,
                        kAudioUnitProperty_SampleRate,
                        kAudioUnitScope_Input,
                        bus_idx as u32,
                        &sample_rate as *const _ as *const c_void,
                        std::mem::size_of::<f64>() as u32,
                    );
                    let in_asbd = AudioStreamBasicDescription::canonical_non_interleaved_f32(
                        sample_rate,
                        ch_count,
                    );
                    let _ = AudioUnitSetProperty(
                        self.unit,
                        kAudioUnitProperty_StreamFormat,
                        kAudioUnitScope_Input,
                        bus_idx as u32,
                        &in_asbd as *const _ as *const c_void,
                        std::mem::size_of::<AudioStreamBasicDescription>() as u32,
                    );
                }
            }

            for (bus_idx, &ch_count) in self.out_bus_channels.iter().enumerate() {
                let _ = AudioUnitSetProperty(
                    self.unit,
                    kAudioUnitProperty_SampleRate,
                    kAudioUnitScope_Output,
                    bus_idx as u32,
                    &sample_rate as *const _ as *const c_void,
                    std::mem::size_of::<f64>() as u32,
                );
                let out_asbd = AudioStreamBasicDescription::canonical_non_interleaved_f32(
                    sample_rate,
                    ch_count.max(1),
                );
                let _ = AudioUnitSetProperty(
                    self.unit,
                    kAudioUnitProperty_StreamFormat,
                    kAudioUnitScope_Output,
                    bus_idx as u32,
                    &out_asbd as *const _ as *const c_void,
                    std::mem::size_of::<AudioStreamBasicDescription>() as u32,
                );
            }

            self.setup_host_callbacks();

            let status = AudioUnitInitialize(self.unit);
            if status != noErr {
                return Err(HostError::ActivationError(format!(
                    "AudioUnitInitialize failed with OSStatus {status}"
                )));
            }
        }

        self.is_initialized = true;
        Ok(())
    }

    fn deactivate(&mut self) {
        if self.is_initialized && !self.unit.is_null() {
            unsafe {
                let _ = AudioUnitUninitialize(self.unit);
            }
            self.is_initialized = false;
        }
    }

    fn reset(&mut self) {
        self.render_ctx.sample_time = 0.0;
        if !self.unit.is_null() {
            unsafe {
                let _ = AudioUnitReset(self.unit, kAudioUnitScope_Global, 0);
            }
        }
    }

    fn process(
        &mut self,
        buffers: &mut AudioProcessBuffers<'_>,
        in_events: &[ProcessEvent],
        _out_events: &mut Vec<ProcessEvent>,
    ) -> HostResult<ProcessStatus> {
        let num_frames = buffers.num_frames;
        if num_frames == 0 {
            return Ok(ProcessStatus::Continue);
        }

        let in_ch_count = buffers.inputs.len().min(self.render_ctx.input_ptrs.len());
        for (i, ch) in buffers.inputs.iter().take(in_ch_count).enumerate() {
            self.render_ctx.input_ptrs[i] = ch.as_ptr();
        }
        self.render_ctx.num_input_channels = in_ch_count;
        self.render_ctx.num_frames = num_frames;

        self.param_event_scratch.clear();
        for ev in in_events {
            match *ev {
                ProcessEvent::ParamValue {
                    sample_offset,
                    param_id,
                    value,
                    ..
                } => unsafe {
                    let _ = AudioUnitSetParameter(
                        self.unit,
                        param_id,
                        kAudioUnitScope_Global,
                        0,
                        value as f32,
                        sample_offset,
                    );
                },
                ProcessEvent::NoteOn {
                    sample_offset,
                    channel,
                    key,
                    velocity,
                    ..
                } => {
                    let status = 0x90 | ((channel.max(0) as u32) & 0x0F);
                    let data1 = (key.clamp(0, 127)) as u32;
                    let data2 = ((velocity.clamp(0.0, 1.0) * 127.0).round() as u32).clamp(1, 127);
                    unsafe {
                        let _ =
                            MusicDeviceMIDIEvent(self.unit, status, data1, data2, sample_offset);
                    }
                }
                ProcessEvent::NoteOff {
                    sample_offset,
                    channel,
                    key,
                    velocity,
                    ..
                } => {
                    let status = 0x80 | ((channel.max(0) as u32) & 0x0F);
                    let data1 = (key.clamp(0, 127)) as u32;
                    let data2 = ((velocity.clamp(0.0, 1.0) * 127.0).round() as u32).min(127);
                    unsafe {
                        let _ =
                            MusicDeviceMIDIEvent(self.unit, status, data1, data2, sample_offset);
                    }
                }
                ProcessEvent::Midi1 {
                    sample_offset,
                    data,
                    ..
                } => unsafe {
                    let _ = MusicDeviceMIDIEvent(
                        self.unit,
                        data[0] as u32,
                        data[1] as u32,
                        data[2] as u32,
                        sample_offset,
                    );
                },
                ProcessEvent::ParamMod { .. } => {}
            }
        }

        if !self.param_event_scratch.is_empty() {
            unsafe {
                let _ = AudioUnitScheduleParameters(
                    self.unit,
                    self.param_event_scratch.as_ptr(),
                    self.param_event_scratch.len() as u32,
                );
            }
        }

        let total_out_slices = buffers.outputs.len().max(1);
        let byte_size = (num_frames * std::mem::size_of::<f32>()) as u32;

        unsafe {
            let abl_ptr = self.out_abl_storage.as_mut_ptr() as *mut AudioBufferList;
            let bufs_ptr = (*abl_ptr).mBuffers.as_mut_ptr();
            let time_stamp = AudioTimeStamp {
                mSampleTime: self.render_ctx.sample_time,
                mFlags: kAudioTimeStampSampleTimeValid,
                ..Default::default()
            };

            let main_out_ch = if self.out_bus_channels.len() > 1
                && total_out_slices > (self.output_channels as usize)
            {
                (self.output_channels as usize).min(total_out_slices).max(1)
            } else {
                total_out_slices
            };

            (*abl_ptr).mNumberBuffers = main_out_ch as u32;
            for (ch_idx, out_slice) in buffers.outputs.iter_mut().take(main_out_ch).enumerate() {
                (*bufs_ptr.add(ch_idx)) = AudioBuffer {
                    mNumberChannels: 1,
                    mDataByteSize: byte_size,
                    mData: out_slice.as_mut_ptr() as *mut c_void,
                };
            }

            let mut action_flags: AudioUnitRenderActionFlags = 0;
            let status = AudioUnitRender(
                self.unit,
                &mut action_flags,
                &time_stamp,
                0,
                num_frames as u32,
                abl_ptr,
            );

            if status == noErr && self.out_bus_channels.len() > 1 && total_out_slices > main_out_ch
            {
                let mut ch_offset = main_out_ch;
                for (bus_idx, &bus_ch_u32) in self.out_bus_channels.iter().enumerate().skip(1) {
                    let bus_ch = (bus_ch_u32 as usize).max(1);
                    if ch_offset + bus_ch > total_out_slices {
                        break;
                    }
                    (*abl_ptr).mNumberBuffers = bus_ch as u32;
                    for (local_ch, out_slice) in buffers.outputs[ch_offset..ch_offset + bus_ch]
                        .iter_mut()
                        .enumerate()
                    {
                        (*bufs_ptr.add(local_ch)) = AudioBuffer {
                            mNumberChannels: 1,
                            mDataByteSize: byte_size,
                            mData: out_slice.as_mut_ptr() as *mut c_void,
                        };
                    }
                    let mut aux_flags: AudioUnitRenderActionFlags = 0;
                    let _ = AudioUnitRender(
                        self.unit,
                        &mut aux_flags,
                        &time_stamp,
                        bus_idx as u32,
                        num_frames as u32,
                        abl_ptr,
                    );
                    ch_offset += bus_ch;
                }
            }

            self.render_ctx.sample_time += num_frames as f64;

            if status != noErr {
                return Err(HostError::ProcessError(format!(
                    "AudioUnitRender failed with OSStatus {status}"
                )));
            }
        }

        Ok(ProcessStatus::Continue)
    }

    fn supports_state(&self) -> bool {
        let mut size: u32 = 0;
        let mut writable: Boolean = 0;
        unsafe {
            AudioUnitGetPropertyInfo(
                self.unit,
                kAudioUnitProperty_ClassInfo,
                kAudioUnitScope_Global,
                0,
                &mut size,
                &mut writable,
            ) == noErr
                && size > 0
        }
    }

    fn save_state(&self) -> HostResult<Vec<u8>> {
        unsafe {
            let mut plist: CFTypeRef = ptr::null();
            let mut size = std::mem::size_of::<CFTypeRef>() as u32;
            let status = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_ClassInfo,
                kAudioUnitScope_Global,
                0,
                &mut plist as *mut _ as *mut c_void,
                &mut size,
            );
            if status != noErr || plist.is_null() {
                return Err(HostError::StateError(format!(
                    "AudioUnitGetProperty(kAudioUnitProperty_ClassInfo) failed with OSStatus {status}"
                )));
            }

            let mut error: CFTypeRef = ptr::null();
            let cf_data_ref = CFPropertyListCreateData(
                ptr::null(),
                plist,
                kCFPropertyListBinaryFormat_v1_0,
                0,
                &mut error,
            );
            CFRelease(plist);

            if cf_data_ref.is_null() {
                return Err(HostError::StateError(
                    "CFPropertyListCreateData returned null".into(),
                ));
            }

            let cf_data = CFData::wrap_under_create_rule(cf_data_ref as CFDataRef);
            Ok(cf_data.bytes().to_vec())
        }
    }

    fn load_state(&mut self, data: &[u8]) -> HostResult<()> {
        unsafe {
            let cf_data = CFData::from_buffer(data);
            let mut format: isize = 0;
            let mut error: CFTypeRef = ptr::null();
            let plist = CFPropertyListCreateWithData(
                ptr::null(),
                cf_data.as_concrete_TypeRef() as CFTypeRef,
                0,
                &mut format,
                &mut error,
            );
            if plist.is_null() {
                return Err(HostError::StateError(
                    "CFPropertyListCreateWithData failed to parse state bytes".into(),
                ));
            }

            let status = AudioUnitSetProperty(
                self.unit,
                kAudioUnitProperty_ClassInfo,
                kAudioUnitScope_Global,
                0,
                &plist as *const _ as *const c_void,
                std::mem::size_of::<CFTypeRef>() as u32,
            );
            CFRelease(plist);

            if status != noErr {
                return Err(HostError::StateError(format!(
                    "AudioUnitSetProperty(kAudioUnitProperty_ClassInfo) failed with OSStatus {status}"
                )));
            }
            Ok(())
        }
    }

    fn latency_samples(&self) -> Option<u32> {
        unsafe {
            let mut latency_sec: Float64 = 0.0;
            let mut size = std::mem::size_of::<Float64>() as u32;
            let status = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_Latency,
                kAudioUnitScope_Global,
                0,
                &mut latency_sec as *mut _ as *mut c_void,
                &mut size,
            );
            if status == noErr {
                Some((latency_sec * self.sample_rate).round().max(0.0) as u32)
            } else {
                None
            }
        }
    }

    fn tail_samples(&self) -> Option<u32> {
        unsafe {
            let mut tail_sec: Float64 = 0.0;
            let mut size = std::mem::size_of::<Float64>() as u32;
            let status = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_TailTime,
                kAudioUnitScope_Global,
                0,
                &mut tail_sec as *mut _ as *mut c_void,
                &mut size,
            );
            if status == noErr {
                Some((tail_sec * self.sample_rate).round().max(0.0) as u32)
            } else {
                None
            }
        }
    }
}
