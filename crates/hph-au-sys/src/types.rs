#[cfg(target_os = "macos")]
pub use core_foundation::base::CFTypeRef;
#[cfg(target_os = "macos")]
pub use core_foundation::string::CFStringRef;
#[cfg(target_os = "macos")]
pub use core_foundation::url::CFURLRef;
use std::ffi::c_void;

#[cfg(not(target_os = "macos"))]
pub type CFTypeRef = *const c_void;
#[cfg(not(target_os = "macos"))]
pub type CFStringRef = *const c_void;
#[cfg(not(target_os = "macos"))]
pub type CFURLRef = *const c_void;

pub type OSStatus = i32;
pub type OSType = u32;
pub type UInt32 = u32;
pub type SInt32 = i32;
pub type SInt16 = i16;
pub type UInt16 = u16;
pub type UInt8 = u8;
pub type UInt64 = u64;
pub type Float64 = f64;
pub type Float32 = f32;
pub type Boolean = u8;

pub const noErr: OSStatus = 0;

#[repr(C)]
pub struct OpaqueAudioComponent {
    _private: [u8; 0],
}
pub type AudioComponent = *mut OpaqueAudioComponent;

#[repr(C)]
pub struct ComponentInstanceRecord {
    _private: [u8; 0],
}
pub type AudioComponentInstance = *mut ComponentInstanceRecord;
pub type AudioUnit = AudioComponentInstance;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AudioComponentDescription {
    pub componentType: OSType,
    pub componentSubType: OSType,
    pub componentManufacturer: OSType,
    pub componentFlags: UInt32,
    pub componentFlagsMask: UInt32,
}

#[repr(C)]
pub struct AudioComponentPlugInInterface {
    pub Open: Option<
        unsafe extern "C" fn(self_ptr: *mut c_void, mInstance: AudioComponentInstance) -> OSStatus,
    >,
    pub Close: Option<unsafe extern "C" fn(self_ptr: *mut c_void) -> OSStatus>,
    pub Lookup: Option<unsafe extern "C" fn(selector: SInt16) -> *const c_void>,
    pub reserved: *mut c_void,
}

pub type AudioComponentFactoryFunction = unsafe extern "C" fn(
    inDesc: *const AudioComponentDescription,
)
    -> *mut AudioComponentPlugInInterface;

// --- Component Types ---
pub const kAudioUnitType_Output: OSType = fourcc(*b"auou");
pub const kAudioUnitType_MusicDevice: OSType = fourcc(*b"aumu");
pub const kAudioUnitType_MusicEffect: OSType = fourcc(*b"aumf");
pub const kAudioUnitType_FormatConverter: OSType = fourcc(*b"aufc");
pub const kAudioUnitType_Effect: OSType = fourcc(*b"aufx");
pub const kAudioUnitType_Mixer: OSType = fourcc(*b"aumx");
pub const kAudioUnitType_Panner: OSType = fourcc(*b"aupn");
pub const kAudioUnitType_Generator: OSType = fourcc(*b"augn");
pub const kAudioUnitType_OfflineEffect: OSType = fourcc(*b"auol");
pub const kAudioUnitType_MIDIProcessor: OSType = fourcc(*b"aumi");

// --- AudioUnit Scopes & Elements ---
pub type AudioUnitScope = UInt32;
pub type AudioUnitElement = UInt32;
pub type AudioUnitPropertyID = UInt32;
pub type AudioUnitParameterID = UInt32;
pub type AudioUnitParameterValue = Float32;

pub const kAudioUnitScope_Global: AudioUnitScope = 0;
pub const kAudioUnitScope_Input: AudioUnitScope = 1;
pub const kAudioUnitScope_Output: AudioUnitScope = 2;
pub const kAudioUnitScope_Group: AudioUnitScope = 3;
pub const kAudioUnitScope_Part: AudioUnitScope = 4;
pub const kAudioUnitScope_Note: AudioUnitScope = 5;

// --- AudioUnit Property IDs ---
pub const kAudioUnitProperty_ClassInfo: AudioUnitPropertyID = 0;
pub const kAudioUnitProperty_MakeConnection: AudioUnitPropertyID = 1;
pub const kAudioUnitProperty_SampleRate: AudioUnitPropertyID = 2;
pub const kAudioUnitProperty_ParameterList: AudioUnitPropertyID = 3;
pub const kAudioUnitProperty_ParameterInfo: AudioUnitPropertyID = 4;
pub const kAudioUnitProperty_StreamFormat: AudioUnitPropertyID = 8;
pub const kAudioUnitProperty_ElementCount: AudioUnitPropertyID = 11;
pub const kAudioUnitProperty_Latency: AudioUnitPropertyID = 12;
pub const kAudioUnitProperty_SupportedNumChannels: AudioUnitPropertyID = 13;
pub const kAudioUnitProperty_MaximumFramesPerSlice: AudioUnitPropertyID = 14;
pub const kAudioUnitProperty_ParameterValueStrings: AudioUnitPropertyID = 16;
pub const kAudioUnitProperty_AudioChannelLayout: AudioUnitPropertyID = 19;
pub const kAudioUnitProperty_TailTime: AudioUnitPropertyID = 20;
pub const kAudioUnitProperty_BypassEffect: AudioUnitPropertyID = 21;
pub const kAudioUnitProperty_LastRenderError: AudioUnitPropertyID = 22;
pub const kAudioUnitProperty_SetRenderCallback: AudioUnitPropertyID = 23;
pub const kAudioUnitProperty_FactoryPresets: AudioUnitPropertyID = 24;
pub const kAudioUnitProperty_HostCallbacks: AudioUnitPropertyID = 27;
pub const kAudioUnitProperty_InPlaceProcessing: AudioUnitPropertyID = 29;
pub const kAudioUnitProperty_ElementName: AudioUnitPropertyID = 30;
pub const kAudioUnitProperty_CocoaUI: AudioUnitPropertyID = 31;
pub const kAudioUnitProperty_ParameterStringFromValue: AudioUnitPropertyID = 33;
pub const kAudioUnitProperty_ParameterClumpName: AudioUnitPropertyID = 35;
pub const kAudioUnitProperty_PresentPreset: AudioUnitPropertyID = 36;
pub const kAudioUnitProperty_ParameterValueFromString: AudioUnitPropertyID = 38;

// --- FourCC Utilities ---
pub const fn fourcc(bytes: [u8; 4]) -> OSType {
    ((bytes[0] as u32) << 24)
        | ((bytes[1] as u32) << 16)
        | ((bytes[2] as u32) << 8)
        | (bytes[3] as u32)
}

pub fn str_to_fourcc(s: &str) -> Option<OSType> {
    let b = s.as_bytes();
    if b.len() != 4 {
        return None;
    }
    Some(fourcc([b[0], b[1], b[2], b[3]]))
}

pub fn fourcc_to_string(code: OSType) -> String {
    let bytes = [
        ((code >> 24) & 0xFF) as u8,
        ((code >> 16) & 0xFF) as u8,
        ((code >> 8) & 0xFF) as u8,
        (code & 0xFF) as u8,
    ];
    bytes
        .iter()
        .map(|&b| {
            if (0x20..=0x7E).contains(&b) {
                b as char
            } else {
                '?'
            }
        })
        .collect()
}
