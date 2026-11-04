#[cfg(target_os = "macos")]
use std::ffi::c_void;

#[cfg(target_os = "macos")]
use crate::params::AudioUnitParameterEvent;
#[cfg(target_os = "macos")]
use crate::stream_format::{AudioBufferList, AudioTimeStamp, AudioUnitRenderActionFlags};
#[cfg(target_os = "macos")]
use crate::types::{
    AudioComponent, AudioComponentDescription, AudioComponentFactoryFunction,
    AudioComponentInstance, AudioUnit, AudioUnitElement, AudioUnitParameterID,
    AudioUnitParameterValue, AudioUnitPropertyID, AudioUnitScope, Boolean, CFStringRef, CFTypeRef,
    OSStatus, UInt32,
};

#[cfg(target_os = "macos")]
extern "C" {
    pub fn AudioComponentFindNext(
        inComponent: AudioComponent,
        inDesc: *const AudioComponentDescription,
    ) -> AudioComponent;

    pub fn AudioComponentRegister(
        inDesc: *const AudioComponentDescription,
        inName: CFStringRef,
        inVersion: UInt32,
        inFactory: AudioComponentFactoryFunction,
    ) -> AudioComponent;

    pub fn AudioComponentCopyName(
        inComponent: AudioComponent,
        outName: *mut CFStringRef,
    ) -> OSStatus;

    pub fn AudioComponentGetDescription(
        inComponent: AudioComponent,
        outDesc: *mut AudioComponentDescription,
    ) -> OSStatus;

    pub fn AudioComponentGetVersion(
        inComponent: AudioComponent,
        outVersion: *mut UInt32,
    ) -> OSStatus;

    pub fn AudioComponentInstanceNew(
        inComponent: AudioComponent,
        outInstance: *mut AudioComponentInstance,
    ) -> OSStatus;

    pub fn AudioComponentInstanceDispose(inInstance: AudioComponentInstance) -> OSStatus;

    pub fn AudioUnitInitialize(inUnit: AudioUnit) -> OSStatus;
    pub fn AudioUnitUninitialize(inUnit: AudioUnit) -> OSStatus;

    pub fn AudioUnitGetPropertyInfo(
        inUnit: AudioUnit,
        inID: AudioUnitPropertyID,
        inScope: AudioUnitScope,
        inElement: AudioUnitElement,
        outDataSize: *mut UInt32,
        outWritable: *mut Boolean,
    ) -> OSStatus;

    pub fn AudioUnitGetProperty(
        inUnit: AudioUnit,
        inID: AudioUnitPropertyID,
        inScope: AudioUnitScope,
        inElement: AudioUnitElement,
        outData: *mut c_void,
        ioDataSize: *mut UInt32,
    ) -> OSStatus;

    pub fn AudioUnitSetProperty(
        inUnit: AudioUnit,
        inID: AudioUnitPropertyID,
        inScope: AudioUnitScope,
        inElement: AudioUnitElement,
        inData: *const c_void,
        inDataSize: UInt32,
    ) -> OSStatus;

    pub fn AudioUnitGetParameter(
        inUnit: AudioUnit,
        inID: AudioUnitParameterID,
        inScope: AudioUnitScope,
        inElement: AudioUnitElement,
        outValue: *mut AudioUnitParameterValue,
    ) -> OSStatus;

    pub fn AudioUnitSetParameter(
        inUnit: AudioUnit,
        inID: AudioUnitParameterID,
        inScope: AudioUnitScope,
        inElement: AudioUnitElement,
        inValue: AudioUnitParameterValue,
        inBufferOffsetInFrames: UInt32,
    ) -> OSStatus;

    pub fn AudioUnitRender(
        inUnit: AudioUnit,
        ioActionFlags: *mut AudioUnitRenderActionFlags,
        inTimeStamp: *const AudioTimeStamp,
        inOutputBusNumber: UInt32,
        inNumberFrames: UInt32,
        ioData: *mut AudioBufferList,
    ) -> OSStatus;

    pub fn AudioUnitReset(
        inUnit: AudioUnit,
        inScope: AudioUnitScope,
        inElement: AudioUnitElement,
    ) -> OSStatus;

    pub fn AudioUnitScheduleParameters(
        inUnit: AudioUnit,
        inParameterEvent: *const AudioUnitParameterEvent,
        inNumParamEvents: UInt32,
    ) -> OSStatus;

    pub fn MusicDeviceMIDIEvent(
        inUnit: AudioUnit,
        inStatus: UInt32,
        inData1: UInt32,
        inData2: UInt32,
        inOffsetSampleFrame: UInt32,
    ) -> OSStatus;

    // CoreFoundation PropertyList serialization helpers
    pub fn CFPropertyListCreateData(
        allocator: CFTypeRef,
        propertyList: CFTypeRef,
        format: isize,
        options: usize,
        error: *mut CFTypeRef,
    ) -> CFTypeRef;

    pub fn CFPropertyListCreateWithData(
        allocator: CFTypeRef,
        data: CFTypeRef,
        options: usize,
        format: *mut isize,
        error: *mut CFTypeRef,
    ) -> CFTypeRef;

    pub fn CFArrayGetCount(theArray: CFTypeRef) -> isize;
    pub fn CFArrayGetValueAtIndex(theArray: CFTypeRef, idx: isize) -> *const c_void;
}

pub const kCFPropertyListXMLFormat_v1_0: isize = 100;
pub const kCFPropertyListBinaryFormat_v1_0: isize = 200;
