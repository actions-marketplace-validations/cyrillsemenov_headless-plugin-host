use hph_vst3_sys::*;
use std::ffi::c_void;
use std::ptr;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct SharedHostState {
    pub(crate) logs: Vec<String>,
    pub(crate) restart_requested: bool,
    pub(crate) dirty: bool,
    pub(crate) group_edit_depth: u32,
    pub(crate) edited_params: Vec<(u32, f64)>,
}

#[repr(C)]
pub(crate) struct Vst3ComponentHandler2Sub {
    lp_vtbl: *const Steinberg_Vst_IComponentHandler2Vtbl,
    parent: *mut Vst3ComponentHandler,
}

#[repr(C)]
pub(crate) struct Vst3ComponentHandler {
    lp_vtbl: *const Steinberg_Vst_IComponentHandlerVtbl,
    pub(crate) handler2: Vst3ComponentHandler2Sub,
    pub(crate) state: Arc<Mutex<SharedHostState>>,
}

static COMPONENT_HANDLER_VTBL: Steinberg_Vst_IComponentHandlerVtbl =
    Steinberg_Vst_IComponentHandlerVtbl {
        queryInterface: comp_handler_query_interface,
        addRef: comp_handler_add_ref,
        release: comp_handler_release,
        beginEdit: comp_handler_begin_edit,
        performEdit: comp_handler_perform_edit,
        endEdit: comp_handler_end_edit,
        restartComponent: comp_handler_restart_component,
    };

static COMPONENT_HANDLER2_VTBL: Steinberg_Vst_IComponentHandler2Vtbl =
    Steinberg_Vst_IComponentHandler2Vtbl {
        queryInterface: comp_handler2_query_interface,
        addRef: comp_handler_add_ref,
        release: comp_handler_release,
        setDirty: comp_handler2_set_dirty,
        requestOpenEditor: comp_handler2_request_open_editor,
        startGroupEdit: comp_handler2_start_group_edit,
        finishGroupEdit: comp_handler2_finish_group_edit,
    };

impl Vst3ComponentHandler {
    pub(crate) fn new_boxed(state: Arc<Mutex<SharedHostState>>) -> Box<Self> {
        let mut boxed = Box::new(Self {
            lp_vtbl: &COMPONENT_HANDLER_VTBL,
            handler2: Vst3ComponentHandler2Sub {
                lp_vtbl: &COMPONENT_HANDLER2_VTBL,
                parent: ptr::null_mut(),
            },
            state,
        });
        let parent_ptr = &mut *boxed as *mut Vst3ComponentHandler;
        boxed.handler2.parent = parent_ptr;
        boxed
    }
}

pub(crate) unsafe extern "system" fn comp_handler_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_Vst_IComponentHandler_iid {
            *obj = this_interface;
            return kResultOk;
        }
        if *target == Steinberg_Vst_IComponentHandler2_iid {
            let handler = &mut *(this_interface as *mut Vst3ComponentHandler);
            *obj = &mut handler.handler2 as *mut Vst3ComponentHandler2Sub as *mut c_void;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn comp_handler2_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        let sub = &*(this_interface as *const Vst3ComponentHandler2Sub);
        comp_handler_query_interface(sub.parent as *mut c_void, iid, obj)
    }
}

unsafe extern "system" fn comp_handler2_set_dirty(
    this_interface: *mut c_void,
    state: Steinberg_TBool,
) -> Steinberg_tresult {
    let sub = unsafe { &*(this_interface as *const Vst3ComponentHandler2Sub) };
    let handler = unsafe { &*sub.parent };
    if let Ok(mut st) = handler.state.lock() {
        st.dirty = state != 0;
    }
    kResultOk
}

unsafe extern "system" fn comp_handler2_request_open_editor(
    _this_interface: *mut c_void,
    _name: Steinberg_FIDString,
) -> Steinberg_tresult {
    kResultOk
}

unsafe extern "system" fn comp_handler2_start_group_edit(
    this_interface: *mut c_void,
) -> Steinberg_tresult {
    let sub = unsafe { &*(this_interface as *const Vst3ComponentHandler2Sub) };
    let handler = unsafe { &*sub.parent };
    if let Ok(mut st) = handler.state.lock() {
        st.group_edit_depth = st.group_edit_depth.saturating_add(1);
    }
    kResultOk
}

unsafe extern "system" fn comp_handler2_finish_group_edit(
    this_interface: *mut c_void,
) -> Steinberg_tresult {
    let sub = unsafe { &*(this_interface as *const Vst3ComponentHandler2Sub) };
    let handler = unsafe { &*sub.parent };
    if let Ok(mut st) = handler.state.lock() {
        st.group_edit_depth = st.group_edit_depth.saturating_sub(1);
    }
    kResultOk
}

unsafe extern "system" fn comp_handler_add_ref(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn comp_handler_release(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn comp_handler_begin_edit(
    _this_interface: *mut c_void,
    _id: Steinberg_Vst_ParamID,
) -> Steinberg_tresult {
    kResultOk
}

unsafe extern "system" fn comp_handler_perform_edit(
    this_interface: *mut c_void,
    id: Steinberg_Vst_ParamID,
    value_normalized: Steinberg_Vst_ParamValue,
) -> Steinberg_tresult {
    let handler = unsafe { &*(this_interface as *const Vst3ComponentHandler) };
    if let Ok(mut st) = handler.state.lock() {
        st.edited_params.push((id, value_normalized));
    }
    kResultOk
}

unsafe extern "system" fn comp_handler_end_edit(
    _this_interface: *mut c_void,
    _id: Steinberg_Vst_ParamID,
) -> Steinberg_tresult {
    kResultOk
}

unsafe extern "system" fn comp_handler_restart_component(
    this_interface: *mut c_void,
    _flags: Steinberg_int32,
) -> Steinberg_tresult {
    let handler = unsafe { &*(this_interface as *const Vst3ComponentHandler) };
    if let Ok(mut st) = handler.state.lock() {
        st.restart_requested = true;
    }
    kResultOk
}
