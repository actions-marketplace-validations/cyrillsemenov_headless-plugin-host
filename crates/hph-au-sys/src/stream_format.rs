use std::ffi::c_void;

use crate::types::{fourcc, Float64, OSStatus, SInt16, UInt32, UInt64};

// --- CoreAudio Stream Format ---
pub type AudioFormatID = UInt32;
pub type AudioFormatFlags = UInt32;

pub const kAudioFormatLinearPCM: AudioFormatID = fourcc(*b"lpcm");
pub const kAudioFormatFlagIsFloat: AudioFormatFlags = 1 << 0;
pub const kAudioFormatFlagIsBigEndian: AudioFormatFlags = 1 << 1;
pub const kAudioFormatFlagIsSignedInteger: AudioFormatFlags = 1 << 2;
pub const kAudioFormatFlagIsPacked: AudioFormatFlags = 1 << 3;
pub const kAudioFormatFlagIsNonInterleaved: AudioFormatFlags = 1 << 5;
pub const kAudioFormatFlagsNativeFloatPacked: AudioFormatFlags =
    kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioStreamBasicDescription {
    pub mSampleRate: Float64,
    pub mFormatID: AudioFormatID,
    pub mFormatFlags: AudioFormatFlags,
    pub mBytesPerPacket: UInt32,
    pub mFramesPerPacket: UInt32,
    pub mBytesPerFrame: UInt32,
    pub mChannelsPerFrame: UInt32,
    pub mBitsPerChannel: UInt32,
    pub mReserved: UInt32,
}

impl AudioStreamBasicDescription {
    pub fn canonical_non_interleaved_f32(sample_rate: f64, channels: u32) -> Self {
        Self {
            mSampleRate: sample_rate,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagsNativeFloatPacked | kAudioFormatFlagIsNonInterleaved,
            mBytesPerPacket: 4,
            mFramesPerPacket: 1,
            mBytesPerFrame: 4,
            mChannelsPerFrame: channels,
            mBitsPerChannel: 32,
            mReserved: 0,
        }
    }
}

// --- AudioTimeStamp & SMPTETime ---
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SMPTETime {
    pub mSubframes: SInt16,
    pub mSubframeDivisor: SInt16,
    pub mCounter: UInt32,
    pub mType: UInt32,
    pub mFlags: UInt32,
    pub mHours: SInt16,
    pub mMinutes: SInt16,
    pub mSeconds: SInt16,
    pub mFrames: SInt16,
}

pub type AudioTimeStampFlags = UInt32;
pub const kAudioTimeStampSampleTimeValid: AudioTimeStampFlags = 1 << 0;
pub const kAudioTimeStampHostTimeValid: AudioTimeStampFlags = 1 << 1;
pub const kAudioTimeStampRateScalarValid: AudioTimeStampFlags = 1 << 2;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioTimeStamp {
    pub mSampleTime: Float64,
    pub mHostTime: UInt64,
    pub mRateScalar: Float64,
    pub mWordClockTime: UInt64,
    pub mSMPTETime: SMPTETime,
    pub mFlags: AudioTimeStampFlags,
    pub mReserved: UInt32,
}

// --- AudioBuffer & AudioBufferList ---
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioBuffer {
    pub mNumberChannels: UInt32,
    pub mDataByteSize: UInt32,
    pub mData: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioBufferList {
    pub mNumberBuffers: UInt32,
    pub mBuffers: [AudioBuffer; 1],
}

// --- Render Flags & Callback ---
pub type AudioUnitRenderActionFlags = UInt32;
pub const kAudioUnitRenderAction_PreRender: AudioUnitRenderActionFlags = 1 << 2;
pub const kAudioUnitRenderAction_PostRender: AudioUnitRenderActionFlags = 1 << 3;
pub const kAudioUnitRenderAction_OutputIsSilence: AudioUnitRenderActionFlags = 1 << 4;

pub type AURenderCallback = unsafe extern "C" fn(
    inRefCon: *mut c_void,
    ioActionFlags: *mut AudioUnitRenderActionFlags,
    inTimeStamp: *const AudioTimeStamp,
    inBusNumber: UInt32,
    inNumberFrames: UInt32,
    ioData: *mut AudioBufferList,
) -> OSStatus;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AURenderCallbackStruct {
    pub inputProc: Option<AURenderCallback>,
    pub inputProcRefCon: *mut c_void,
}
