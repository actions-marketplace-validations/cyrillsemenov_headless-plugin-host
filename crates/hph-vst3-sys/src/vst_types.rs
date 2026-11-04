use crate::base::{
    Steinberg_Vst_BusDirection, Steinberg_Vst_BusType, Steinberg_Vst_IoMode,
    Steinberg_Vst_MediaType, Steinberg_Vst_ParamID, Steinberg_Vst_ParamValue,
    Steinberg_Vst_ProgramListID, Steinberg_Vst_Sample32, Steinberg_Vst_Sample64,
    Steinberg_Vst_SpeakerArrangement, Steinberg_Vst_String128, Steinberg_Vst_TChar,
    Steinberg_Vst_TQuarterNotes, Steinberg_Vst_TSamples, Steinberg_Vst_UnitID, Steinberg_int16,
    Steinberg_int32, Steinberg_int64, Steinberg_int8, Steinberg_uint16, Steinberg_uint32,
    Steinberg_uint64, Steinberg_uint8,
};
use crate::interfaces::{Steinberg_Vst_IEventList, Steinberg_Vst_IParameterChanges};

// MediaTypes
pub const Steinberg_Vst_MediaTypes_kAudio: Steinberg_Vst_MediaType = 0;
pub const Steinberg_Vst_MediaTypes_kEvent: Steinberg_Vst_MediaType = 1;

// BusDirections
pub const Steinberg_Vst_BusDirections_kInput: Steinberg_Vst_BusDirection = 0;
pub const Steinberg_Vst_BusDirections_kOutput: Steinberg_Vst_BusDirection = 1;

// BusTypes
pub const Steinberg_Vst_BusTypes_kMain: Steinberg_Vst_BusType = 0;
pub const Steinberg_Vst_BusTypes_kAux: Steinberg_Vst_BusType = 1;

// BusFlags
pub const Steinberg_Vst_BusInfo_BusFlags_kDefaultActive: u32 = 1 << 0;
pub const Steinberg_Vst_BusInfo_BusFlags_kIsControlVoltage: u32 = 1 << 1;

// IoModes
pub const Steinberg_Vst_IoModes_kSimple: Steinberg_Vst_IoMode = 0;
pub const Steinberg_Vst_IoModes_kAdvanced: Steinberg_Vst_IoMode = 1;
pub const Steinberg_Vst_IoModes_kOfflineProcessing: Steinberg_Vst_IoMode = 2;

// ParameterFlags
pub const Steinberg_Vst_ParameterInfo_ParameterFlags_kNoFlags: i32 = 0;
pub const Steinberg_Vst_ParameterInfo_ParameterFlags_kCanAutomate: i32 = 1 << 0;
pub const Steinberg_Vst_ParameterInfo_ParameterFlags_kIsReadOnly: i32 = 1 << 1;
pub const Steinberg_Vst_ParameterInfo_ParameterFlags_kIsWrapAround: i32 = 1 << 2;
pub const Steinberg_Vst_ParameterInfo_ParameterFlags_kIsList: i32 = 1 << 3;
pub const Steinberg_Vst_ParameterInfo_ParameterFlags_kIsHidden: i32 = 1 << 4;
pub const Steinberg_Vst_ParameterInfo_ParameterFlags_kIsProgramChange: i32 = 1 << 15;
pub const Steinberg_Vst_ParameterInfo_ParameterFlags_kIsBypass: i32 = 1 << 16;

// ProcessModes
pub const Steinberg_Vst_ProcessModes_kRealtime: i32 = 0;
pub const Steinberg_Vst_ProcessModes_kPrefetch: i32 = 1;
pub const Steinberg_Vst_ProcessModes_kOffline: i32 = 2;

// SymbolicSampleSizes
pub const Steinberg_Vst_SymbolicSampleSizes_kSample32: i32 = 0;
pub const Steinberg_Vst_SymbolicSampleSizes_kSample64: i32 = 1;

// Event types
pub const Steinberg_Vst_Event_EventTypes_kNoteOnEvent: u16 = 0;
pub const Steinberg_Vst_Event_EventTypes_kNoteOffEvent: u16 = 1;
pub const Steinberg_Vst_Event_EventTypes_kDataEvent: u16 = 2;
pub const Steinberg_Vst_Event_EventTypes_kPolyPressureEvent: u16 = 3;
pub const Steinberg_Vst_Event_EventTypes_kNoteExpressionValueEvent: u16 = 4;
pub const Steinberg_Vst_Event_EventTypes_kNoteExpressionTextEvent: u16 = 5;
pub const Steinberg_Vst_Event_EventTypes_kChordEvent: u16 = 6;
pub const Steinberg_Vst_Event_EventTypes_kScaleEvent: u16 = 7;
pub const Steinberg_Vst_Event_EventTypes_kLegacyMIDICCOutEvent: u16 = 65535;

// ProcessContext StatesAndFlags
pub const Steinberg_Vst_ProcessContext_StatesAndFlags_kPlaying: u32 = 1 << 1;
pub const Steinberg_Vst_ProcessContext_StatesAndFlags_kProjectTimeMusicValid: u32 = 1 << 9;
pub const Steinberg_Vst_ProcessContext_StatesAndFlags_kBarPositionValid: u32 = 1 << 11;
pub const Steinberg_Vst_ProcessContext_StatesAndFlags_kTempoValid: u32 = 1 << 10;
pub const Steinberg_Vst_ProcessContext_StatesAndFlags_kTimeSigValid: u32 = 1 << 13;

// SpeakerArrangements
pub const kSpeakerArrEmpty: Steinberg_Vst_SpeakerArrangement = 0;
pub const kSpeakerArrMono: Steinberg_Vst_SpeakerArrangement = 1 << 2; // kSpeakerM
pub const kSpeakerArrStereo: Steinberg_Vst_SpeakerArrangement = (1 << 0) | (1 << 1); // kSpeakerL | kSpeakerR

// Data structures
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_ViewRect {
    pub left: Steinberg_int32,
    pub top: Steinberg_int32,
    pub right: Steinberg_int32,
    pub bottom: Steinberg_int32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_Vst_BusInfo {
    pub mediaType: Steinberg_Vst_MediaType,
    pub direction: Steinberg_Vst_BusDirection,
    pub channelCount: Steinberg_int32,
    pub name: Steinberg_Vst_String128,
    pub busType: Steinberg_Vst_BusType,
    pub flags: Steinberg_uint32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_RoutingInfo {
    pub mediaType: Steinberg_Vst_MediaType,
    pub busIndex: Steinberg_int32,
    pub channel: Steinberg_int32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_Vst_ParameterInfo {
    pub id: Steinberg_Vst_ParamID,
    pub title: Steinberg_Vst_String128,
    pub shortTitle: Steinberg_Vst_String128,
    pub units: Steinberg_Vst_String128,
    pub stepCount: Steinberg_int32,
    pub defaultNormalizedValue: Steinberg_Vst_ParamValue,
    pub unitId: Steinberg_Vst_UnitID,
    pub flags: Steinberg_int32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_NoteOnEvent {
    pub channel: Steinberg_int16,
    pub pitch: Steinberg_int16,
    pub tuning: f32,
    pub velocity: f32,
    pub length: Steinberg_int32,
    pub noteId: Steinberg_int32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_NoteOffEvent {
    pub channel: Steinberg_int16,
    pub pitch: Steinberg_int16,
    pub velocity: f32,
    pub noteId: Steinberg_int32,
    pub tuning: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Steinberg_Vst_DataEvent {
    pub size: Steinberg_uint32,
    pub r#type: Steinberg_uint32,
    pub bytes: *const Steinberg_uint8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_PolyPressureEvent {
    pub channel: Steinberg_int16,
    pub pitch: Steinberg_int16,
    pub pressure: f32,
    pub noteId: Steinberg_int32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_NoteExpressionValueEvent {
    pub typeId: Steinberg_uint32,
    pub noteId: Steinberg_int32,
    pub value: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Steinberg_Vst_NoteExpressionTextEvent {
    pub typeId: Steinberg_uint32,
    pub noteId: Steinberg_int32,
    pub textLen: Steinberg_uint32,
    pub text: *const Steinberg_Vst_TChar,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Steinberg_Vst_ChordEvent {
    pub root: Steinberg_int16,
    pub bassNote: Steinberg_int16,
    pub mask: Steinberg_int16,
    pub textLen: Steinberg_uint16,
    pub text: *const Steinberg_Vst_TChar,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Steinberg_Vst_ScaleEvent {
    pub root: Steinberg_int16,
    pub mask: Steinberg_int16,
    pub textLen: Steinberg_uint16,
    pub text: *const Steinberg_Vst_TChar,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_LegacyMIDICCOutEvent {
    pub controlNumber: Steinberg_uint8,
    pub channel: Steinberg_int8,
    pub value: Steinberg_int8,
    pub value2: Steinberg_int8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Steinberg_Vst_Event_Data {
    pub noteOn: Steinberg_Vst_NoteOnEvent,
    pub noteOff: Steinberg_Vst_NoteOffEvent,
    pub data: Steinberg_Vst_DataEvent,
    pub polyPressure: Steinberg_Vst_PolyPressureEvent,
    pub noteExpressionValue: Steinberg_Vst_NoteExpressionValueEvent,
    pub noteExpressionText: Steinberg_Vst_NoteExpressionTextEvent,
    pub chord: Steinberg_Vst_ChordEvent,
    pub scale: Steinberg_Vst_ScaleEvent,
    pub midiCCOut: Steinberg_Vst_LegacyMIDICCOutEvent,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_Vst_Event {
    pub busIndex: Steinberg_int32,
    pub sampleOffset: Steinberg_int32,
    pub ppqPosition: Steinberg_Vst_TQuarterNotes,
    pub flags: Steinberg_uint16,
    pub r#type: Steinberg_uint16,
    pub event: Steinberg_Vst_Event_Data,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_ProcessSetup {
    pub processMode: Steinberg_int32,
    pub symbolicSampleSize: Steinberg_int32,
    pub maxSamplesPerBlock: Steinberg_int32,
    pub sampleRate: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Steinberg_Vst_AudioBusBuffers_Buffers {
    pub channelBuffers32: *mut *mut Steinberg_Vst_Sample32,
    pub channelBuffers64: *mut *mut Steinberg_Vst_Sample64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_Vst_AudioBusBuffers {
    pub numChannels: Steinberg_int32,
    pub silenceFlags: Steinberg_uint64,
    pub buffers: Steinberg_Vst_AudioBusBuffers_Buffers,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_FrameRate {
    pub framesPerSecond: Steinberg_uint32,
    pub flags: Steinberg_uint32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_Chord {
    pub keyNote: Steinberg_uint8,
    pub rootNote: Steinberg_uint8,
    pub chordMask: Steinberg_int16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Steinberg_Vst_ProcessContext {
    pub state: Steinberg_uint32,
    pub sampleRate: f64,
    pub projectTimeSamples: Steinberg_Vst_TSamples,
    pub systemTime: Steinberg_int64,
    pub continousTimeSamples: Steinberg_Vst_TSamples,
    pub projectTimeMusic: Steinberg_Vst_TQuarterNotes,
    pub barPositionMusic: Steinberg_Vst_TQuarterNotes,
    pub cycleStartMusic: Steinberg_Vst_TQuarterNotes,
    pub cycleEndMusic: Steinberg_Vst_TQuarterNotes,
    pub tempo: f64,
    pub timeSigNumerator: Steinberg_int32,
    pub timeSigDenominator: Steinberg_int32,
    pub chord: Steinberg_Vst_Chord,
    pub smpteOffsetSubframes: Steinberg_int32,
    pub frameRate: Steinberg_Vst_FrameRate,
    pub samplesToNextClock: Steinberg_int32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_Vst_ProcessData {
    pub processMode: Steinberg_int32,
    pub symbolicSampleSize: Steinberg_int32,
    pub numSamples: Steinberg_int32,
    pub numInputs: Steinberg_int32,
    pub numOutputs: Steinberg_int32,
    pub inputs: *mut Steinberg_Vst_AudioBusBuffers,
    pub outputs: *mut Steinberg_Vst_AudioBusBuffers,
    pub inputParameterChanges: *mut Steinberg_Vst_IParameterChanges,
    pub outputParameterChanges: *mut Steinberg_Vst_IParameterChanges,
    pub inputEvents: *mut Steinberg_Vst_IEventList,
    pub outputEvents: *mut Steinberg_Vst_IEventList,
    pub processContext: *mut Steinberg_Vst_ProcessContext,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_Vst_UnitInfo {
    pub id: Steinberg_Vst_UnitID,
    pub parentUnitId: Steinberg_Vst_UnitID,
    pub name: Steinberg_Vst_String128,
    pub programListId: Steinberg_Vst_ProgramListID,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_Vst_ProgramListInfo {
    pub id: Steinberg_Vst_ProgramListID,
    pub name: Steinberg_Vst_String128,
    pub programCount: Steinberg_int32,
}
