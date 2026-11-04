use std::ffi::{c_char, c_void};

use crate::types::{
    AudioUnitElement, AudioUnitParameterID, AudioUnitParameterValue, AudioUnitScope, Boolean,
    CFStringRef, CFURLRef, Float32, Float64, OSStatus, SInt16, SInt32, UInt32,
};

// --- Parameters & Events ---
pub type AudioUnitParameterUnit = UInt32;
pub const kAudioUnitParameterUnit_Generic: AudioUnitParameterUnit = 0;
pub const kAudioUnitParameterUnit_Indexed: AudioUnitParameterUnit = 1;
pub const kAudioUnitParameterUnit_Boolean: AudioUnitParameterUnit = 2;
pub const kAudioUnitParameterUnit_Percent: AudioUnitParameterUnit = 3;
pub const kAudioUnitParameterUnit_Seconds: AudioUnitParameterUnit = 4;
pub const kAudioUnitParameterUnit_SampleFrames: AudioUnitParameterUnit = 5;
pub const kAudioUnitParameterUnit_Phase: AudioUnitParameterUnit = 6;
pub const kAudioUnitParameterUnit_Rate: AudioUnitParameterUnit = 7;
pub const kAudioUnitParameterUnit_Hertz: AudioUnitParameterUnit = 8;
pub const kAudioUnitParameterUnit_Cents: AudioUnitParameterUnit = 9;
pub const kAudioUnitParameterUnit_RelativeSemiTones: AudioUnitParameterUnit = 10;
pub const kAudioUnitParameterUnit_MIDINoteNumber: AudioUnitParameterUnit = 11;
pub const kAudioUnitParameterUnit_MIDIController: AudioUnitParameterUnit = 12;
pub const kAudioUnitParameterUnit_Decibels: AudioUnitParameterUnit = 13;
pub const kAudioUnitParameterUnit_LinearGain: AudioUnitParameterUnit = 14;
pub const kAudioUnitParameterUnit_Degrees: AudioUnitParameterUnit = 15;
pub const kAudioUnitParameterUnit_EqualPowerCrossfade: AudioUnitParameterUnit = 16;
pub const kAudioUnitParameterUnit_MixerFaderCurve1: AudioUnitParameterUnit = 17;
pub const kAudioUnitParameterUnit_Pan: AudioUnitParameterUnit = 18;
pub const kAudioUnitParameterUnit_Meters: AudioUnitParameterUnit = 19;
pub const kAudioUnitParameterUnit_AbsoluteCents: AudioUnitParameterUnit = 20;
pub const kAudioUnitParameterUnit_Octaves: AudioUnitParameterUnit = 21;
pub const kAudioUnitParameterUnit_BPM: AudioUnitParameterUnit = 22;
pub const kAudioUnitParameterUnit_Beats: AudioUnitParameterUnit = 23;
pub const kAudioUnitParameterUnit_Milliseconds: AudioUnitParameterUnit = 24;
pub const kAudioUnitParameterUnit_Ratio: AudioUnitParameterUnit = 25;
pub const kAudioUnitParameterUnit_CustomUnit: AudioUnitParameterUnit = 26;

pub type AudioUnitParameterOptions = UInt32;
pub const kAudioUnitParameterFlag_CFNameRelease: AudioUnitParameterOptions = 1 << 4;
pub const kAudioUnitParameterFlag_OmitFromPresets: AudioUnitParameterOptions = 1 << 13;
pub const kAudioUnitParameterFlag_PlotHistory: AudioUnitParameterOptions = 1 << 14;
pub const kAudioUnitParameterFlag_MeterReadOnly: AudioUnitParameterOptions = 1 << 15;
pub const kAudioUnitParameterFlag_DisplayMask: AudioUnitParameterOptions = (7 << 16) | (1 << 22);
pub const kAudioUnitParameterFlag_HasClump: AudioUnitParameterOptions = 1 << 20;
pub const kAudioUnitParameterFlag_ValuesHaveStrings: AudioUnitParameterOptions = 1 << 21;
pub const kAudioUnitParameterFlag_CanRamp: AudioUnitParameterOptions = 1 << 25;
pub const kAudioUnitParameterFlag_NonRealTime: AudioUnitParameterOptions = 1 << 26;
pub const kAudioUnitParameterFlag_HasCFNameString: AudioUnitParameterOptions = 1 << 27;
pub const kAudioUnitParameterFlag_IsGlobalMeta: AudioUnitParameterOptions = 1 << 28;
pub const kAudioUnitParameterFlag_IsElementMeta: AudioUnitParameterOptions = 1 << 29;
pub const kAudioUnitParameterFlag_IsReadable: AudioUnitParameterOptions = 1 << 30;
pub const kAudioUnitParameterFlag_IsWritable: AudioUnitParameterOptions = 1 << 31;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AudioUnitParameterInfo {
    pub name: [c_char; 52],
    pub unitName: CFStringRef,
    pub clumpID: UInt32,
    pub cfNameString: CFStringRef,
    pub unit: AudioUnitParameterUnit,
    pub minValue: AudioUnitParameterValue,
    pub maxValue: AudioUnitParameterValue,
    pub defaultValue: AudioUnitParameterValue,
    pub flags: AudioUnitParameterOptions,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioUnitParameterStringFromValue {
    pub inParamID: AudioUnitParameterID,
    pub inValue: *const AudioUnitParameterValue,
    pub outString: CFStringRef,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioUnitParameterValueFromString {
    pub inParamID: AudioUnitParameterID,
    pub inString: CFStringRef,
    pub outValue: AudioUnitParameterValue,
}

pub type AUParameterEventType = UInt32;
pub const kParameterEvent_Immediate: AUParameterEventType = 1;
pub const kParameterEvent_Ramped: AUParameterEventType = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AUParameterEventImmediate {
    pub bufferOffset: UInt32,
    pub value: AudioUnitParameterValue,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AUParameterEventRamp {
    pub startBufferOffset: SInt32,
    pub durationInFrames: UInt32,
    pub startValue: AudioUnitParameterValue,
    pub endValue: AudioUnitParameterValue,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union AUParameterEventValues {
    pub ramp: AUParameterEventRamp,
    pub immediate: AUParameterEventImmediate,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AudioUnitParameterEvent {
    pub scope: AudioUnitScope,
    pub element: AudioUnitElement,
    pub parameter: AudioUnitParameterID,
    pub eventType: AUParameterEventType,
    pub eventValues: AUParameterEventValues,
}

// --- Channel Info & Host Callbacks ---
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AUChannelInfo {
    pub inChannels: SInt16,
    pub outChannels: SInt16,
}

pub type HostCallback_GetBeatAndTempo = Option<
    unsafe extern "C" fn(
        inHostUserData: *mut c_void,
        outCurrentBeat: *mut Float64,
        outCurrentTempo: *mut Float64,
    ) -> OSStatus,
>;

pub type HostCallback_GetMusicalTimeLocation = Option<
    unsafe extern "C" fn(
        inHostUserData: *mut c_void,
        outDeltaSampleOffsetToNextBeat: *mut UInt32,
        outTimeSig_Numerator: *mut Float32,
        outTimeSig_Denominator: *mut UInt32,
        outCurrentMeasureDownBeat: *mut Float64,
    ) -> OSStatus,
>;

pub type HostCallback_GetTransportState = Option<
    unsafe extern "C" fn(
        inHostUserData: *mut c_void,
        outIsPlaying: *mut Boolean,
        outTransportStateChanged: *mut Boolean,
        outCurrentSampleInTimeLine: *mut Float64,
        outIsCycling: *mut Boolean,
        outCycleStartBeat: *mut Float64,
        outCycleEndBeat: *mut Float64,
    ) -> OSStatus,
>;

pub type HostCallback_GetTransportState2 = Option<
    unsafe extern "C" fn(
        inHostUserData: *mut c_void,
        outIsPlaying: *mut Boolean,
        outIsRecording: *mut Boolean,
        outTransportStateChanged: *mut Boolean,
        outCurrentSampleInTimeLine: *mut Float64,
        outIsCycling: *mut Boolean,
        outCycleStartBeat: *mut Float64,
        outCycleEndBeat: *mut Float64,
    ) -> OSStatus,
>;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HostCallbackInfo {
    pub hostUserData: *mut c_void,
    pub beatAndTempoProc: HostCallback_GetBeatAndTempo,
    pub musicalTimeLocationProc: HostCallback_GetMusicalTimeLocation,
    pub transportStateProc: HostCallback_GetTransportState,
    pub transportStateProc2: HostCallback_GetTransportState2,
}

// --- Cocoa UI & Factory Presets ---
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioUnitCocoaViewInfo {
    pub mCocoaAUViewBundleLocation: CFURLRef,
    pub mCocoaAUViewClass: [CFStringRef; 1],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AUPreset {
    pub presetNumber: SInt32,
    pub presetName: CFStringRef,
}
