use std::ffi::c_void;

use crate::base::{
    Steinberg_FIDString, Steinberg_TBool, Steinberg_TUID, Steinberg_char16, Steinberg_int16,
    Steinberg_tresult, Steinberg_uint32,
};
use crate::vst_types::Steinberg_ViewRect;

#[repr(C)]
pub struct Steinberg_IPlugFrameVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub resizeView: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        view: *mut Steinberg_IPlugView,
        newSize: *mut Steinberg_ViewRect,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_IPlugFrame {
    pub lpVtbl: *const Steinberg_IPlugFrameVtbl,
}

pub type Steinberg_Linux_FileDescriptor = i32;
pub type Steinberg_Linux_TimerInterval = u64;

#[repr(C)]
pub struct Steinberg_Linux_IEventHandlerVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub onFDIsSet:
        unsafe extern "system" fn(thisInterface: *mut c_void, fd: Steinberg_Linux_FileDescriptor),
}

#[repr(C)]
pub struct Steinberg_Linux_IEventHandler {
    pub lpVtbl: *const Steinberg_Linux_IEventHandlerVtbl,
}

#[repr(C)]
pub struct Steinberg_Linux_ITimerHandlerVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub onTimer: unsafe extern "system" fn(thisInterface: *mut c_void),
}

#[repr(C)]
pub struct Steinberg_Linux_ITimerHandler {
    pub lpVtbl: *const Steinberg_Linux_ITimerHandlerVtbl,
}

#[repr(C)]
pub struct Steinberg_Linux_IRunLoopVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub registerEventHandler: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        handler: *mut Steinberg_Linux_IEventHandler,
        fd: Steinberg_Linux_FileDescriptor,
    ) -> Steinberg_tresult,
    pub unregisterEventHandler: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        handler: *mut Steinberg_Linux_IEventHandler,
    ) -> Steinberg_tresult,
    pub registerTimer: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        handler: *mut Steinberg_Linux_ITimerHandler,
        milliseconds: Steinberg_Linux_TimerInterval,
    ) -> Steinberg_tresult,
    pub unregisterTimer: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        handler: *mut Steinberg_Linux_ITimerHandler,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_Linux_IRunLoop {
    pub lpVtbl: *const Steinberg_Linux_IRunLoopVtbl,
}

#[repr(C)]
pub struct Steinberg_IPlugViewVtbl {
    pub queryInterface: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        iid: *const Steinberg_TUID,
        obj: *mut *mut c_void,
    ) -> Steinberg_tresult,
    pub addRef: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub release: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_uint32,
    pub isPlatformTypeSupported: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        r#type: Steinberg_FIDString,
    ) -> Steinberg_tresult,
    pub attached: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        parent: *mut c_void,
        r#type: Steinberg_FIDString,
    ) -> Steinberg_tresult,
    pub removed: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_tresult,
    pub onWheel:
        unsafe extern "system" fn(thisInterface: *mut c_void, distance: f32) -> Steinberg_tresult,
    pub onKeyDown: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        key: Steinberg_char16,
        keyCode: Steinberg_int16,
        modifiers: Steinberg_int16,
    ) -> Steinberg_tresult,
    pub onKeyUp: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        key: Steinberg_char16,
        keyCode: Steinberg_int16,
        modifiers: Steinberg_int16,
    ) -> Steinberg_tresult,
    pub getSize: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        size: *mut Steinberg_ViewRect,
    ) -> Steinberg_tresult,
    pub onSize: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        newSize: *mut Steinberg_ViewRect,
    ) -> Steinberg_tresult,
    pub onFocus: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        state: Steinberg_TBool,
    ) -> Steinberg_tresult,
    pub setFrame: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        frame: *mut Steinberg_IPlugFrame,
    ) -> Steinberg_tresult,
    pub canResize: unsafe extern "system" fn(thisInterface: *mut c_void) -> Steinberg_tresult,
    pub checkSizeConstraint: unsafe extern "system" fn(
        thisInterface: *mut c_void,
        rect: *mut Steinberg_ViewRect,
    ) -> Steinberg_tresult,
}

#[repr(C)]
pub struct Steinberg_IPlugView {
    pub lpVtbl: *const Steinberg_IPlugViewVtbl,
}
