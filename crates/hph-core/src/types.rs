use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginFormat {
    Clap,
    Vst3,
    AuV2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescriptorInfo {
    pub format: PluginFormat,
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub url: String,
    pub manual_url: String,
    pub support_url: String,
    pub description: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionProbeResult {
    pub name: String,
    pub supported: bool,
    pub valid_vtable: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPortInfo {
    pub id: u32,
    pub index: u32,
    pub name: String,
    pub is_input: bool,
    pub channel_count: u32,
    pub is_main: bool,
    pub port_type: String,
    pub in_place_pair_id: Option<u32>,
    pub prefers_64bit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotePortInfo {
    pub id: u32,
    pub index: u32,
    pub name: String,
    pub is_input: bool,
    pub supported_dialects: u32,
    pub preferred_dialect: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamFlags {
    pub is_stepped: bool,
    pub is_periodic: bool,
    pub is_hidden: bool,
    pub is_readonly: bool,
    pub is_bypass: bool,
    pub is_automatable: bool,
    pub is_automatable_per_note_id: bool,
    pub is_automatable_per_key: bool,
    pub is_automatable_per_channel: bool,
    pub is_automatable_per_port: bool,
    pub is_modulatable: bool,
    pub is_modulatable_per_note_id: bool,
    pub is_modulatable_per_key: bool,
    pub is_modulatable_per_channel: bool,
    pub is_modulatable_per_port: bool,
    pub requires_process: bool,
    pub raw_bits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamInfo {
    pub id: u32,
    pub index: u32,
    pub name: String,
    pub module: String,
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
    pub flags: ParamFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessEvent {
    NoteOn {
        sample_offset: u32,
        port_index: u16,
        channel: i16,
        key: i16,
        note_id: i32,
        velocity: f64,
    },
    NoteOff {
        sample_offset: u32,
        port_index: u16,
        channel: i16,
        key: i16,
        note_id: i32,
        velocity: f64,
    },
    ParamValue {
        sample_offset: u32,
        param_id: u32,
        note_id: i32,
        port_index: i16,
        channel: i16,
        key: i16,
        value: f64,
    },
    ParamMod {
        sample_offset: u32,
        param_id: u32,
        note_id: i32,
        port_index: i16,
        channel: i16,
        key: i16,
        amount: f64,
    },
    Midi1 {
        sample_offset: u32,
        port_index: u16,
        data: [u8; 3],
    },
}

impl ProcessEvent {
    pub fn sample_offset(&self) -> u32 {
        match self {
            ProcessEvent::NoteOn { sample_offset, .. } => *sample_offset,
            ProcessEvent::NoteOff { sample_offset, .. } => *sample_offset,
            ProcessEvent::ParamValue { sample_offset, .. } => *sample_offset,
            ProcessEvent::ParamMod { sample_offset, .. } => *sample_offset,
            ProcessEvent::Midi1 { sample_offset, .. } => *sample_offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessStatus {
    Continue,
    ContinueIfNotQuiet,
    Tail,
    Sleep,
    Error,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostDiagnostics {
    pub logs: Vec<String>,
    pub thread_violations: Vec<String>,
    pub restart_requested: bool,
    pub process_requested: bool,
    pub callback_requested: bool,
    pub flush_requested: bool,
}

/// Planar audio buffer for processing a single block of frames.
pub struct AudioProcessBuffers<'a> {
    pub inputs: &'a [&'a [f32]],
    pub outputs: &'a mut [&'a mut [f32]],
    pub num_frames: usize,
    pub steady_time: i64,
}
