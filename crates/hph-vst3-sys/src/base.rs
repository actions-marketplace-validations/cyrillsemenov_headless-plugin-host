use std::ffi::{c_char, c_void};

pub type Steinberg_int8 = i8;
pub type Steinberg_uint8 = u8;
pub type Steinberg_int16 = i16;
pub type Steinberg_uint16 = u16;
pub type Steinberg_int32 = i32;
pub type Steinberg_uint32 = u32;
pub type Steinberg_int64 = i64;
pub type Steinberg_uint64 = u64;
pub type Steinberg_TPtrInt = isize;
pub type Steinberg_TBool = u8;
pub type Steinberg_char8 = c_char;
pub type Steinberg_char16 = u16;
pub type Steinberg_tchar = Steinberg_char16;
pub type Steinberg_CStringA = *const Steinberg_char8;
pub type Steinberg_CStringW = *const Steinberg_char16;
pub type Steinberg_CString = *const Steinberg_tchar;
pub type Steinberg_FIDString = *const Steinberg_char8;
pub type Steinberg_tresult = Steinberg_int32;
pub type Steinberg_TUID = [u8; 16];
pub type Steinberg_TSize = Steinberg_int64;

pub type Steinberg_Vst_TChar = Steinberg_char16;
pub type Steinberg_Vst_String128 = [Steinberg_Vst_TChar; 128];
pub type Steinberg_Vst_CString = *const Steinberg_char8;
pub type Steinberg_Vst_ParamID = Steinberg_uint32;
pub type Steinberg_Vst_ParamValue = f64;
pub type Steinberg_Vst_UnitID = Steinberg_int32;
pub type Steinberg_Vst_ProgramListID = Steinberg_int32;
pub type Steinberg_Vst_CtrlNumber = Steinberg_int16;
pub type Steinberg_Vst_TQuarterNotes = f64;
pub type Steinberg_Vst_TSamples = Steinberg_int64;
pub type Steinberg_Vst_SpeakerArrangement = Steinberg_uint64;
pub type Steinberg_Vst_Speaker = Steinberg_uint64;
pub type Steinberg_Vst_Sample32 = f32;
pub type Steinberg_Vst_Sample64 = f64;
pub type Steinberg_Vst_MediaType = Steinberg_int32;
pub type Steinberg_Vst_BusDirection = Steinberg_int32;
pub type Steinberg_Vst_BusType = Steinberg_int32;
pub type Steinberg_Vst_IoMode = Steinberg_int32;

/// Packs 4 32-bit words into a 16-byte `Steinberg_TUID` matching `SMTG_INLINE_UID`
/// (`SMTG_COM_COMPATIBLE = 0` on non-Windows, `SMTG_COM_COMPATIBLE = 1` on Windows).
pub const fn smtg_inline_uid(l1: u32, l2: u32, l3: u32, l4: u32) -> Steinberg_TUID {
    #[cfg(target_os = "windows")]
    {
        [
            (l1 & 0x0000_00FF) as u8,
            ((l1 & 0x0000_FF00) >> 8) as u8,
            ((l1 & 0x00FF_0000) >> 16) as u8,
            ((l1 & 0xFF00_0000) >> 24) as u8,
            ((l2 & 0x00FF_0000) >> 16) as u8,
            ((l2 & 0xFF00_0000) >> 24) as u8,
            (l2 & 0x0000_00FF) as u8,
            ((l2 & 0x0000_FF00) >> 8) as u8,
            ((l3 & 0xFF00_0000) >> 24) as u8,
            ((l3 & 0x00FF_0000) >> 16) as u8,
            ((l3 & 0x0000_FF00) >> 8) as u8,
            (l3 & 0x0000_00FF) as u8,
            ((l4 & 0xFF00_0000) >> 24) as u8,
            ((l4 & 0x00FF_0000) >> 16) as u8,
            ((l4 & 0x0000_FF00) >> 8) as u8,
            (l4 & 0x0000_00FF) as u8,
        ]
    }
    #[cfg(not(target_os = "windows"))]
    {
        [
            ((l1 & 0xFF00_0000) >> 24) as u8,
            ((l1 & 0x00FF_0000) >> 16) as u8,
            ((l1 & 0x0000_FF00) >> 8) as u8,
            (l1 & 0x0000_00FF) as u8,
            ((l2 & 0xFF00_0000) >> 24) as u8,
            ((l2 & 0x00FF_0000) >> 16) as u8,
            ((l2 & 0x0000_FF00) >> 8) as u8,
            (l2 & 0x0000_00FF) as u8,
            ((l3 & 0xFF00_0000) >> 24) as u8,
            ((l3 & 0x00FF_0000) >> 16) as u8,
            ((l3 & 0x0000_FF00) >> 8) as u8,
            (l3 & 0x0000_00FF) as u8,
            ((l4 & 0xFF00_0000) >> 24) as u8,
            ((l4 & 0x00FF_0000) >> 16) as u8,
            ((l4 & 0x0000_FF00) >> 8) as u8,
            (l4 & 0x0000_00FF) as u8,
        ]
    }
}

// Result codes (SMTG_COM_COMPATIBLE = 1 on Windows, 0 on non-Windows)
#[cfg(target_os = "windows")]
pub const kNoInterface: Steinberg_tresult = 0x8000_4002u32 as Steinberg_tresult;
#[cfg(target_os = "windows")]
pub const kResultOk: Steinberg_tresult = 0x0000_0000;
#[cfg(target_os = "windows")]
pub const kResultTrue: Steinberg_tresult = kResultOk;
#[cfg(target_os = "windows")]
pub const kResultFalse: Steinberg_tresult = 0x0000_0001;
#[cfg(target_os = "windows")]
pub const kInvalidArgument: Steinberg_tresult = 0x8007_0057u32 as Steinberg_tresult;
#[cfg(target_os = "windows")]
pub const kNotImplemented: Steinberg_tresult = 0x8000_4001u32 as Steinberg_tresult;
#[cfg(target_os = "windows")]
pub const kInternalError: Steinberg_tresult = 0x8000_4005u32 as Steinberg_tresult;
#[cfg(target_os = "windows")]
pub const kNotInitialized: Steinberg_tresult = 0x8000_FFFFu32 as Steinberg_tresult;
#[cfg(target_os = "windows")]
pub const kOutOfMemory: Steinberg_tresult = 0x8007_000Eu32 as Steinberg_tresult;

#[cfg(not(target_os = "windows"))]
pub const kNoInterface: Steinberg_tresult = -1;
#[cfg(not(target_os = "windows"))]
pub const kResultOk: Steinberg_tresult = 0;
#[cfg(not(target_os = "windows"))]
pub const kResultTrue: Steinberg_tresult = kResultOk;
#[cfg(not(target_os = "windows"))]
pub const kResultFalse: Steinberg_tresult = 1;
#[cfg(not(target_os = "windows"))]
pub const kInvalidArgument: Steinberg_tresult = 2;
#[cfg(not(target_os = "windows"))]
pub const kNotImplemented: Steinberg_tresult = 3;
#[cfg(not(target_os = "windows"))]
pub const kInternalError: Steinberg_tresult = 4;
#[cfg(not(target_os = "windows"))]
pub const kNotInitialized: Steinberg_tresult = 5;
#[cfg(not(target_os = "windows"))]
pub const kOutOfMemory: Steinberg_tresult = 6;

// IBStream seek modes
pub const Steinberg_IBStream_IStreamSeekMode_kIBSeekSet: i32 = 0;
pub const Steinberg_IBStream_IStreamSeekMode_kIBSeekCur: i32 = 1;
pub const Steinberg_IBStream_IStreamSeekMode_kIBSeekEnd: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_PFactoryInfo {
    pub vendor: [Steinberg_char8; 64],
    pub url: [Steinberg_char8; 256],
    pub email: [Steinberg_char8; 128],
    pub flags: Steinberg_int32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_PClassInfo {
    pub cid: Steinberg_TUID,
    pub cardinality: Steinberg_int32,
    pub category: [Steinberg_char8; 32],
    pub name: [Steinberg_char8; 64],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_PClassInfo2 {
    pub cid: Steinberg_TUID,
    pub cardinality: Steinberg_int32,
    pub category: [Steinberg_char8; 32],
    pub name: [Steinberg_char8; 64],
    pub classFlags: Steinberg_uint32,
    pub subCategories: [Steinberg_char8; 128],
    pub vendor: [Steinberg_char8; 64],
    pub version: [Steinberg_char8; 64],
    pub sdkVersion: [Steinberg_char8; 64],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Steinberg_PClassInfoW {
    pub cid: Steinberg_TUID,
    pub cardinality: Steinberg_int32,
    pub category: [Steinberg_char8; 32],
    pub name: [Steinberg_char16; 64],
    pub classFlags: Steinberg_uint32,
    pub subCategories: [Steinberg_char8; 128],
    pub vendor: [Steinberg_char16; 64],
    pub version: [Steinberg_char16; 64],
    pub sdkVersion: [Steinberg_char16; 64],
}

#[repr(C)]
pub struct Steinberg_FUnknownVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
}

#[repr(C)]
pub struct Steinberg_FUnknown {
    pub lpVtbl: *const Steinberg_FUnknownVtbl,
}

#[repr(C)]
pub struct Steinberg_IBStreamVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub read: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        buffer: *mut c_void,
        numBytes: Steinberg_int32,
        numBytesRead: *mut Steinberg_int32,
    ) -> Steinberg_tresult,
    pub write: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        buffer: *mut c_void,
        numBytes: Steinberg_int32,
        numBytesWritten: *mut Steinberg_int32,
    ) -> Steinberg_tresult,
    pub seek: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        pos: Steinberg_int64,
        mode: Steinberg_int32,
        result: *mut Steinberg_int64,
    ) -> Steinberg_tresult,
    pub tell: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        pos: *mut Steinberg_int64,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_IBStream {
    pub lpVtbl: *const Steinberg_IBStreamVtbl,
}

#[repr(C)]
pub struct Steinberg_IPluginBaseVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub initialize: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        context: *mut Steinberg_FUnknown,
    ) -> Steinberg_tresult,
    pub terminate: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_IPluginBase {
    pub lpVtbl: *const Steinberg_IPluginBaseVtbl,
}

#[repr(C)]
pub struct Steinberg_IPluginFactoryVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getFactoryInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        info: *mut Steinberg_PFactoryInfo,
    ) -> Steinberg_tresult,
    pub countClasses: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_int32,
    pub getClassInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        index: Steinberg_int32,
        info: *mut Steinberg_PClassInfo,
    ) -> Steinberg_tresult,
    pub createInstance: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        cid: Steinberg_FIDString,
        iid: Steinberg_FIDString,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_IPluginFactory {
    pub lpVtbl: *const Steinberg_IPluginFactoryVtbl,
}

#[repr(C)]
pub struct Steinberg_IPluginFactory2Vtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getFactoryInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        info: *mut Steinberg_PFactoryInfo,
    ) -> Steinberg_tresult,
    pub countClasses: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_int32,
    pub getClassInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        index: Steinberg_int32,
        info: *mut Steinberg_PClassInfo,
    ) -> Steinberg_tresult,
    pub createInstance: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        cid: Steinberg_FIDString,
        iid: Steinberg_FIDString,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub getClassInfo2: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        index: Steinberg_int32,
        info: *mut Steinberg_PClassInfo2,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_IPluginFactory2 {
    pub lpVtbl: *const Steinberg_IPluginFactory2Vtbl,
}

#[repr(C)]
pub struct Steinberg_IPluginFactory3Vtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub getFactoryInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        info: *mut Steinberg_PFactoryInfo,
    ) -> Steinberg_tresult,
    pub countClasses: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_int32,
    pub getClassInfo: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        index: Steinberg_int32,
        info: *mut Steinberg_PClassInfo,
    ) -> Steinberg_tresult,
    pub createInstance: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        cid: Steinberg_FIDString,
        iid: Steinberg_FIDString,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub getClassInfo2: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        index: Steinberg_int32,
        info: *mut Steinberg_PClassInfo2,
    ) -> Steinberg_tresult,
    pub getClassInfoUnicode: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        index: Steinberg_int32,
        info: *mut Steinberg_PClassInfoW,
    ) -> Steinberg_tresult,
    pub setHostContext: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        context: *mut Steinberg_FUnknown,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_IPluginFactory3 {
    pub lpVtbl: *const Steinberg_IPluginFactory3Vtbl,
}

// Helper utilities for UTF-16 String128, char8 arrays, and TUID formatting
pub fn utf16_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub fn string_to_utf16_buf(s: &str, out: &mut [u16]) {
    out.fill(0);
    let max_chars = out.len().saturating_sub(1);
    for (dst, src) in out.iter_mut().take(max_chars).zip(s.encode_utf16()) {
        *dst = src;
    }
}

pub fn cchar_slice_to_string(buf: &[c_char]) -> String {
    let bytes: &[u8] = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len()) };
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

pub fn uid_to_hex(uid: &Steinberg_TUID) -> String {
    let mut s = String::with_capacity(32);
    for b in uid {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02X}", b);
    }
    s
}
