use hph_vst3_sys::*;
use std::ffi::c_void;
use std::ptr;

const MAX_POINTS_PER_PARAM_QUEUE: usize = 128;

#[repr(C)]
pub(crate) struct Vst3ParamValueQueue {
    lp_vtbl: *const Steinberg_Vst_IParamValueQueueVtbl,
    pub(crate) param_id: Steinberg_Vst_ParamID,
    pub(crate) points: Vec<(Steinberg_int32, Steinberg_Vst_ParamValue)>,
}

static PARAM_VALUE_QUEUE_VTBL: Steinberg_Vst_IParamValueQueueVtbl =
    Steinberg_Vst_IParamValueQueueVtbl {
        queryInterface: param_queue_query_interface,
        addRef: param_queue_add_ref,
        release: param_queue_release,
        getParameterId: param_queue_get_parameter_id,
        getPointCount: param_queue_get_point_count,
        getPoint: param_queue_get_point,
        addPoint: param_queue_add_point,
    };

impl Vst3ParamValueQueue {
    pub(crate) fn new_preallocated() -> Self {
        Self {
            lp_vtbl: &PARAM_VALUE_QUEUE_VTBL,
            param_id: 0,
            points: Vec::with_capacity(MAX_POINTS_PER_PARAM_QUEUE),
        }
    }
}

unsafe extern "system" fn param_queue_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_Vst_IParamValueQueue_iid {
            *obj = this_interface;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn param_queue_add_ref(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn param_queue_release(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn param_queue_get_parameter_id(
    this_interface: *mut c_void,
) -> Steinberg_Vst_ParamID {
    let q = unsafe { &*(this_interface as *const Vst3ParamValueQueue) };
    q.param_id
}

unsafe extern "system" fn param_queue_get_point_count(
    this_interface: *mut c_void,
) -> Steinberg_int32 {
    let q = unsafe { &*(this_interface as *const Vst3ParamValueQueue) };
    q.points.len() as i32
}

unsafe extern "system" fn param_queue_get_point(
    this_interface: *mut c_void,
    index: Steinberg_int32,
    sample_offset: *mut Steinberg_int32,
    value: *mut Steinberg_Vst_ParamValue,
) -> Steinberg_tresult {
    let q = unsafe { &*(this_interface as *const Vst3ParamValueQueue) };
    if index < 0 || (index as usize) >= q.points.len() {
        return kInvalidArgument;
    }
    let (off, val) = q.points[index as usize];
    if !sample_offset.is_null() {
        unsafe { *sample_offset = off };
    }
    if !value.is_null() {
        unsafe { *value = val };
    }
    kResultOk
}

unsafe extern "system" fn param_queue_add_point(
    this_interface: *mut c_void,
    sample_offset: Steinberg_int32,
    value: Steinberg_Vst_ParamValue,
    index: *mut Steinberg_int32,
) -> Steinberg_tresult {
    let q = unsafe { &mut *(this_interface as *mut Vst3ParamValueQueue) };
    if q.points.len() >= q.points.capacity() {
        return kOutOfMemory;
    }
    let idx = q.points.len() as i32;
    q.points.push((sample_offset, value));
    if !index.is_null() {
        unsafe { *index = idx };
    }
    kResultOk
}

#[repr(C)]
pub(crate) struct Vst3ParameterChanges {
    lp_vtbl: *const Steinberg_Vst_IParameterChangesVtbl,
    pub(crate) queues: Vec<Vst3ParamValueQueue>,
    pub(crate) active_count: usize,
}

static PARAMETER_CHANGES_VTBL: Steinberg_Vst_IParameterChangesVtbl =
    Steinberg_Vst_IParameterChangesVtbl {
        queryInterface: param_changes_query_interface,
        addRef: param_changes_add_ref,
        release: param_changes_release,
        getParameterCount: param_changes_get_parameter_count,
        getParameterData: param_changes_get_parameter_data,
        addParameterData: param_changes_add_parameter_data,
    };

impl Vst3ParameterChanges {
    pub(crate) fn new_preallocated(max_queues: usize) -> Box<Self> {
        let cap = max_queues.max(64);
        let mut queues = Vec::with_capacity(cap);
        for _ in 0..cap {
            queues.push(Vst3ParamValueQueue::new_preallocated());
        }
        Box::new(Self {
            lp_vtbl: &PARAMETER_CHANGES_VTBL,
            queues,
            active_count: 0,
        })
    }

    pub(crate) fn clear_rt(&mut self) {
        for q in &mut self.queues[..self.active_count] {
            q.points.clear();
        }
        self.active_count = 0;
    }

    pub(crate) fn push_point_rt(
        &mut self,
        param_id: u32,
        sample_offset: i32,
        value_normalized: f64,
    ) {
        for i in 0..self.active_count {
            if self.queues[i].param_id == param_id {
                let q = &mut self.queues[i];
                if q.points.len() < q.points.capacity() {
                    q.points.push((sample_offset, value_normalized));
                }
                return;
            }
        }
        if self.active_count < self.queues.len() {
            let q = &mut self.queues[self.active_count];
            q.param_id = param_id;
            q.points.clear();
            if q.points.len() < q.points.capacity() {
                q.points.push((sample_offset, value_normalized));
            }
            self.active_count += 1;
        }
    }

    pub(crate) fn as_ptr(&mut self) -> *mut Steinberg_Vst_IParameterChanges {
        self as *mut Self as *mut Steinberg_Vst_IParameterChanges
    }
}

unsafe extern "system" fn param_changes_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_Vst_IParameterChanges_iid {
            *obj = this_interface;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn param_changes_add_ref(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn param_changes_release(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn param_changes_get_parameter_count(
    this_interface: *mut c_void,
) -> Steinberg_int32 {
    let pc = unsafe { &*(this_interface as *const Vst3ParameterChanges) };
    pc.active_count as i32
}

unsafe extern "system" fn param_changes_get_parameter_data(
    this_interface: *mut c_void,
    index: Steinberg_int32,
) -> *mut Steinberg_Vst_IParamValueQueue {
    let pc = unsafe { &mut *(this_interface as *mut Vst3ParameterChanges) };
    if index < 0 || (index as usize) >= pc.active_count {
        return ptr::null_mut();
    }
    &mut pc.queues[index as usize] as *mut Vst3ParamValueQueue
        as *mut Steinberg_Vst_IParamValueQueue
}

unsafe extern "system" fn param_changes_add_parameter_data(
    this_interface: *mut c_void,
    id: *const Steinberg_Vst_ParamID,
    index: *mut Steinberg_int32,
) -> *mut Steinberg_Vst_IParamValueQueue {
    if id.is_null() {
        return ptr::null_mut();
    }
    let param_id = unsafe { *id };
    let pc = unsafe { &mut *(this_interface as *mut Vst3ParameterChanges) };
    for i in 0..pc.active_count {
        if pc.queues[i].param_id == param_id {
            if !index.is_null() {
                unsafe { *index = i as i32 };
            }
            return &mut pc.queues[i] as *mut Vst3ParamValueQueue
                as *mut Steinberg_Vst_IParamValueQueue;
        }
    }
    if pc.active_count >= pc.queues.len() {
        return ptr::null_mut();
    }
    let idx = pc.active_count;
    pc.queues[idx].param_id = param_id;
    pc.queues[idx].points.clear();
    pc.active_count += 1;
    if !index.is_null() {
        unsafe { *index = idx as i32 };
    }
    &mut pc.queues[idx] as *mut Vst3ParamValueQueue as *mut Steinberg_Vst_IParamValueQueue
}

#[repr(C)]
pub(crate) struct Vst3EventList {
    lp_vtbl: *const Steinberg_Vst_IEventListVtbl,
    pub(crate) events: Vec<Steinberg_Vst_Event>,
}

static EVENT_LIST_VTBL: Steinberg_Vst_IEventListVtbl = Steinberg_Vst_IEventListVtbl {
    queryInterface: event_list_query_interface,
    addRef: event_list_add_ref,
    release: event_list_release,
    getEventCount: event_list_get_event_count,
    getEvent: event_list_get_event,
    addEvent: event_list_add_event,
};

impl Vst3EventList {
    pub(crate) fn new_preallocated(capacity: usize) -> Box<Self> {
        Box::new(Self {
            lp_vtbl: &EVENT_LIST_VTBL,
            events: Vec::with_capacity(capacity),
        })
    }

    pub(crate) fn as_ptr(&mut self) -> *mut Steinberg_Vst_IEventList {
        self as *mut Self as *mut Steinberg_Vst_IEventList
    }
}

unsafe extern "system" fn event_list_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_Vst_IEventList_iid {
            *obj = this_interface;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn event_list_add_ref(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn event_list_release(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn event_list_get_event_count(
    this_interface: *mut c_void,
) -> Steinberg_int32 {
    let el = unsafe { &*(this_interface as *const Vst3EventList) };
    el.events.len() as i32
}

unsafe extern "system" fn event_list_get_event(
    this_interface: *mut c_void,
    index: Steinberg_int32,
    e: *mut Steinberg_Vst_Event,
) -> Steinberg_tresult {
    if e.is_null() {
        return kInvalidArgument;
    }
    let el = unsafe { &*(this_interface as *const Vst3EventList) };
    if index < 0 || (index as usize) >= el.events.len() {
        return kInvalidArgument;
    }
    unsafe { *e = el.events[index as usize] };
    kResultOk
}

unsafe extern "system" fn event_list_add_event(
    this_interface: *mut c_void,
    e: *mut Steinberg_Vst_Event,
) -> Steinberg_tresult {
    if e.is_null() {
        return kInvalidArgument;
    }
    let el = unsafe { &mut *(this_interface as *mut Vst3EventList) };
    if el.events.len() >= el.events.capacity() {
        return kOutOfMemory;
    }
    el.events.push(unsafe { *e });
    kResultOk
}
