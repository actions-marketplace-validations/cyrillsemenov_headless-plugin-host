#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(clippy::missing_safety_doc)]

pub mod ffi;
pub mod params;
pub mod stream_format;
pub mod types;

pub use ffi::*;
pub use params::*;
pub use stream_format::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn verify_audiotoolbox_abi_layouts() {
        assert_eq!(size_of::<AudioComponentDescription>(), 20);
        assert_eq!(size_of::<AudioStreamBasicDescription>(), 40);
        assert_eq!(size_of::<SMPTETime>(), 24);
        assert_eq!(size_of::<AudioTimeStamp>(), 64);
        assert_eq!(size_of::<AudioBuffer>(), 16);
        assert_eq!(size_of::<AudioBufferList>(), 24);
        assert_eq!(offset_of!(AudioBufferList, mBuffers), 8);
        assert_eq!(size_of::<AURenderCallbackStruct>(), 16);
        assert_eq!(size_of::<AudioUnitParameterInfo>(), 104);
        assert_eq!(offset_of!(AudioUnitParameterInfo, unit), 80);
        assert_eq!(offset_of!(AudioUnitParameterInfo, minValue), 84);
        assert_eq!(offset_of!(AudioUnitParameterInfo, maxValue), 88);
        assert_eq!(offset_of!(AudioUnitParameterInfo, defaultValue), 92);
        assert_eq!(offset_of!(AudioUnitParameterInfo, flags), 96);
        assert_eq!(size_of::<AudioUnitParameterStringFromValue>(), 24);
        assert_eq!(size_of::<AudioUnitParameterValueFromString>(), 24);
        assert_eq!(size_of::<AudioUnitParameterEvent>(), 32);
        assert_eq!(offset_of!(AudioUnitParameterEvent, eventValues), 16);
        assert_eq!(size_of::<AudioUnitCocoaViewInfo>(), 16);
        assert_eq!(size_of::<AUChannelInfo>(), 4);
        assert_eq!(size_of::<HostCallbackInfo>(), 40);
        assert_eq!(fourcc_to_string(kAudioUnitType_Effect), "aufx");
        assert_eq!(str_to_fourcc("aufx"), Some(kAudioUnitType_Effect));
    }
}
