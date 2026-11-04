use hph_vst3_sys::*;
use std::ffi::c_void;
use std::ptr;
use std::sync::Mutex;

#[repr(C)]
pub(crate) struct Vst3LinuxRunLoopSubobject {
    lp_vtbl: *const Steinberg_Linux_IRunLoopVtbl,
    pub(crate) event_handlers: Mutex<
        Vec<(
            Steinberg_Linux_FileDescriptor,
            *mut Steinberg_Linux_IEventHandler,
        )>,
    >,
    pub(crate) timer_handlers: Mutex<Vec<(u64, u64, *mut Steinberg_Linux_ITimerHandler)>>,
}

#[repr(C)]
pub(crate) struct Vst3PlugFrame {
    lp_vtbl: *const Steinberg_IPlugFrameVtbl,
    pub(crate) run_loop: Vst3LinuxRunLoopSubobject,
}

static PLUG_FRAME_VTBL: Steinberg_IPlugFrameVtbl = Steinberg_IPlugFrameVtbl {
    queryInterface: plug_frame_query_interface,
    addRef: plug_frame_add_ref,
    release: plug_frame_release,
    resizeView: plug_frame_resize_view,
};

static LINUX_RUN_LOOP_VTBL: Steinberg_Linux_IRunLoopVtbl = Steinberg_Linux_IRunLoopVtbl {
    queryInterface: run_loop_query_interface,
    addRef: plug_frame_add_ref,
    release: plug_frame_release,
    registerEventHandler: run_loop_register_event_handler,
    unregisterEventHandler: run_loop_unregister_event_handler,
    registerTimer: run_loop_register_timer,
    unregisterTimer: run_loop_unregister_timer,
};

impl Vst3PlugFrame {
    pub(crate) fn new_boxed() -> Box<Self> {
        Box::new(Self {
            lp_vtbl: &PLUG_FRAME_VTBL,
            run_loop: Vst3LinuxRunLoopSubobject {
                lp_vtbl: &LINUX_RUN_LOOP_VTBL,
                event_handlers: Mutex::new(Vec::new()),
                timer_handlers: Mutex::new(Vec::new()),
            },
        })
    }
}

unsafe extern "system" fn plug_frame_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_IPlugFrame_iid {
            *obj = this_interface;
            return kResultOk;
        }
        if *target == Steinberg_Linux_IRunLoop_iid {
            let frame = &mut *(this_interface as *mut Vst3PlugFrame);
            *obj = &mut frame.run_loop as *mut Vst3LinuxRunLoopSubobject as *mut c_void;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn run_loop_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_Linux_IRunLoop_iid {
            *obj = this_interface;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn run_loop_register_event_handler(
    this_interface: *mut c_void,
    handler: *mut Steinberg_Linux_IEventHandler,
    fd: Steinberg_Linux_FileDescriptor,
) -> Steinberg_tresult {
    if handler.is_null() {
        return kInvalidArgument;
    }
    let rl = unsafe { &*(this_interface as *const Vst3LinuxRunLoopSubobject) };
    if let Ok(mut list) = rl.event_handlers.lock() {
        unsafe { ((*(*handler).lpVtbl).addRef)(handler as *mut c_void) };
        list.push((fd, handler));
        kResultOk
    } else {
        kInternalError
    }
}

unsafe extern "system" fn run_loop_unregister_event_handler(
    this_interface: *mut c_void,
    handler: *mut Steinberg_Linux_IEventHandler,
) -> Steinberg_tresult {
    if handler.is_null() {
        return kInvalidArgument;
    }
    let rl = unsafe { &*(this_interface as *const Vst3LinuxRunLoopSubobject) };
    if let Ok(mut list) = rl.event_handlers.lock() {
        let mut removed = false;
        list.retain(|&(_, h)| {
            if h == handler {
                unsafe { ((*(*h).lpVtbl).release)(h as *mut c_void) };
                removed = true;
                false
            } else {
                true
            }
        });
        if removed {
            kResultOk
        } else {
            kInvalidArgument
        }
    } else {
        kInternalError
    }
}

unsafe extern "system" fn run_loop_register_timer(
    this_interface: *mut c_void,
    handler: *mut Steinberg_Linux_ITimerHandler,
    milliseconds: Steinberg_Linux_TimerInterval,
) -> Steinberg_tresult {
    if handler.is_null() {
        return kInvalidArgument;
    }
    let rl = unsafe { &*(this_interface as *const Vst3LinuxRunLoopSubobject) };
    if let Ok(mut list) = rl.timer_handlers.lock() {
        unsafe { ((*(*handler).lpVtbl).addRef)(handler as *mut c_void) };
        list.push((milliseconds.max(1), 0, handler));
        kResultOk
    } else {
        kInternalError
    }
}

unsafe extern "system" fn run_loop_unregister_timer(
    this_interface: *mut c_void,
    handler: *mut Steinberg_Linux_ITimerHandler,
) -> Steinberg_tresult {
    if handler.is_null() {
        return kInvalidArgument;
    }
    let rl = unsafe { &*(this_interface as *const Vst3LinuxRunLoopSubobject) };
    if let Ok(mut list) = rl.timer_handlers.lock() {
        let mut removed = false;
        list.retain(|&(_, _, h)| {
            if h == handler {
                unsafe { ((*(*h).lpVtbl).release)(h as *mut c_void) };
                removed = true;
                false
            } else {
                true
            }
        });
        if removed {
            kResultOk
        } else {
            kInvalidArgument
        }
    } else {
        kInternalError
    }
}

unsafe extern "system" fn plug_frame_add_ref(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn plug_frame_release(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn plug_frame_resize_view(
    _this_interface: *mut c_void,
    view: *mut Steinberg_IPlugView,
    new_size: *mut Steinberg_ViewRect,
) -> Steinberg_tresult {
    if view.is_null() || new_size.is_null() {
        return kInvalidArgument;
    }
    unsafe { ((*(*view).lpVtbl).onSize)(view as *mut c_void, new_size) }
}
