use hph_vst3_sys::*;
use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::ptr;
use std::sync::Mutex;

#[repr(C)]
pub(crate) struct Vst3HostApplication {
    lp_vtbl: *const Steinberg_Vst_IHostApplicationVtbl,
}

static HOST_APP_VTBL: Steinberg_Vst_IHostApplicationVtbl = Steinberg_Vst_IHostApplicationVtbl {
    queryInterface: host_app_query_interface,
    addRef: host_app_add_ref,
    release: host_app_release,
    getName: host_app_get_name,
    createInstance: host_app_create_instance,
};

impl Vst3HostApplication {
    pub(crate) fn new_boxed() -> Box<Self> {
        Box::new(Self {
            lp_vtbl: &HOST_APP_VTBL,
        })
    }
}

unsafe extern "system" fn host_app_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_Vst_IHostApplication_iid {
            *obj = this_interface;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn host_app_add_ref(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn host_app_release(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn host_app_get_name(
    _this_interface: *mut c_void,
    name: *mut Steinberg_Vst_TChar,
) -> Steinberg_tresult {
    if name.is_null() {
        return kInvalidArgument;
    }
    let slice = unsafe { std::slice::from_raw_parts_mut(name, 128) };
    string_to_utf16_buf("headless-plugin-host", slice);
    kResultOk
}

pub(crate) unsafe extern "system" fn host_app_create_instance(
    _this_interface: *mut c_void,
    cid: *const Steinberg_TUID,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if obj.is_null() || (cid.is_null() && iid.is_null()) {
            return kInvalidArgument;
        }
        *obj = ptr::null_mut();
        let target_iid = if !iid.is_null() { &*iid } else { &*cid };
        let target_cid = if !cid.is_null() { &*cid } else { target_iid };

        if *target_iid == Steinberg_Vst_IMessage_iid || *target_cid == Steinberg_Vst_IMessage_iid {
            let msg = Vst3HostMessage::new_raw();
            *obj = msg as *mut c_void;
            return kResultOk;
        }
        if *target_iid == Steinberg_Vst_IAttributeList_iid
            || *target_cid == Steinberg_Vst_IAttributeList_iid
        {
            let attr = Vst3HostAttributeList::new_raw();
            *obj = attr as *mut c_void;
            return kResultOk;
        }
        kNoInterface
    }
}

#[derive(Default)]
struct AttributeStore {
    ints: HashMap<String, i64>,
    floats: HashMap<String, f64>,
    strings: HashMap<String, Vec<u16>>,
    binaries: HashMap<String, Vec<u8>>,
}

#[repr(C)]
pub(crate) struct Vst3HostAttributeList {
    lp_vtbl: *const Steinberg_Vst_IAttributeListVtbl,
    ref_count: std::sync::atomic::AtomicU32,
    store: Mutex<AttributeStore>,
}

static HOST_ATTR_LIST_VTBL: Steinberg_Vst_IAttributeListVtbl = Steinberg_Vst_IAttributeListVtbl {
    queryInterface: attr_list_query_interface,
    addRef: attr_list_add_ref,
    release: attr_list_release,
    setInt: attr_list_set_int,
    getInt: attr_list_get_int,
    setFloat: attr_list_set_float,
    getFloat: attr_list_get_float,
    setString: attr_list_set_string,
    getString: attr_list_get_string,
    setBinary: attr_list_set_binary,
    getBinary: attr_list_get_binary,
};

impl Vst3HostAttributeList {
    fn new_raw() -> *mut Self {
        Box::into_raw(Box::new(Self {
            lp_vtbl: &HOST_ATTR_LIST_VTBL,
            ref_count: std::sync::atomic::AtomicU32::new(1),
            store: Mutex::new(AttributeStore::default()),
        }))
    }
}

unsafe fn attr_id_to_key(id: Steinberg_Vst_AttrID) -> Option<String> {
    if id.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(id).to_string_lossy().into_owned() })
    }
}

unsafe extern "system" fn attr_list_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_Vst_IAttributeList_iid {
            attr_list_add_ref(this_interface);
            *obj = this_interface;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn attr_list_add_ref(this_interface: *mut c_void) -> Steinberg_uint32 {
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    this.ref_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1
}

unsafe extern "system" fn attr_list_release(this_interface: *mut c_void) -> Steinberg_uint32 {
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    let prev = this
        .ref_count
        .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    if prev == 1 {
        unsafe {
            let _ = Box::from_raw(this_interface as *mut Vst3HostAttributeList);
        }
        0
    } else {
        prev - 1
    }
}

unsafe extern "system" fn attr_list_set_int(
    this_interface: *mut c_void,
    id: Steinberg_Vst_AttrID,
    value: Steinberg_int64,
) -> Steinberg_tresult {
    let Some(key) = (unsafe { attr_id_to_key(id) }) else {
        return kInvalidArgument;
    };
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    if let Ok(mut st) = this.store.lock() {
        st.ints.insert(key, value);
        kResultOk
    } else {
        kInternalError
    }
}

unsafe extern "system" fn attr_list_get_int(
    this_interface: *mut c_void,
    id: Steinberg_Vst_AttrID,
    value: *mut Steinberg_int64,
) -> Steinberg_tresult {
    if value.is_null() {
        return kInvalidArgument;
    }
    let Some(key) = (unsafe { attr_id_to_key(id) }) else {
        return kInvalidArgument;
    };
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    if let Ok(st) = this.store.lock() {
        if let Some(&v) = st.ints.get(&key) {
            unsafe { *value = v };
            return kResultOk;
        }
    }
    kResultFalse
}

unsafe extern "system" fn attr_list_set_float(
    this_interface: *mut c_void,
    id: Steinberg_Vst_AttrID,
    value: f64,
) -> Steinberg_tresult {
    let Some(key) = (unsafe { attr_id_to_key(id) }) else {
        return kInvalidArgument;
    };
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    if let Ok(mut st) = this.store.lock() {
        st.floats.insert(key, value);
        kResultOk
    } else {
        kInternalError
    }
}

unsafe extern "system" fn attr_list_get_float(
    this_interface: *mut c_void,
    id: Steinberg_Vst_AttrID,
    value: *mut f64,
) -> Steinberg_tresult {
    if value.is_null() {
        return kInvalidArgument;
    }
    let Some(key) = (unsafe { attr_id_to_key(id) }) else {
        return kInvalidArgument;
    };
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    if let Ok(st) = this.store.lock() {
        if let Some(&v) = st.floats.get(&key) {
            unsafe { *value = v };
            return kResultOk;
        }
    }
    kResultFalse
}

unsafe extern "system" fn attr_list_set_string(
    this_interface: *mut c_void,
    id: Steinberg_Vst_AttrID,
    string: *const Steinberg_Vst_TChar,
) -> Steinberg_tresult {
    if string.is_null() {
        return kInvalidArgument;
    }
    let Some(key) = (unsafe { attr_id_to_key(id) }) else {
        return kInvalidArgument;
    };
    let mut len = 0usize;
    unsafe {
        while len < 4096 && *string.add(len) != 0 {
            len += 1;
        }
    }
    let mut vec = unsafe { std::slice::from_raw_parts(string, len).to_vec() };
    vec.push(0);
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    if let Ok(mut st) = this.store.lock() {
        st.strings.insert(key, vec);
        kResultOk
    } else {
        kInternalError
    }
}

unsafe extern "system" fn attr_list_get_string(
    this_interface: *mut c_void,
    id: Steinberg_Vst_AttrID,
    string: *mut Steinberg_Vst_TChar,
    size_in_bytes: Steinberg_uint32,
) -> Steinberg_tresult {
    if string.is_null() || size_in_bytes < 2 {
        return kInvalidArgument;
    }
    let Some(key) = (unsafe { attr_id_to_key(id) }) else {
        return kInvalidArgument;
    };
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    if let Ok(st) = this.store.lock() {
        if let Some(src) = st.strings.get(&key) {
            let max_u16 = (size_in_bytes as usize) / 2;
            let dst = unsafe { std::slice::from_raw_parts_mut(string, max_u16) };
            dst.fill(0);
            let copy_len = src.len().min(max_u16.saturating_sub(1));
            dst[..copy_len].copy_from_slice(&src[..copy_len]);
            return kResultOk;
        }
    }
    kResultFalse
}

unsafe extern "system" fn attr_list_set_binary(
    this_interface: *mut c_void,
    id: Steinberg_Vst_AttrID,
    data: *const c_void,
    size_in_bytes: Steinberg_uint32,
) -> Steinberg_tresult {
    let Some(key) = (unsafe { attr_id_to_key(id) }) else {
        return kInvalidArgument;
    };
    let bytes = if data.is_null() || size_in_bytes == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(data as *const u8, size_in_bytes as usize).to_vec() }
    };
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    if let Ok(mut st) = this.store.lock() {
        st.binaries.insert(key, bytes);
        kResultOk
    } else {
        kInternalError
    }
}

unsafe extern "system" fn attr_list_get_binary(
    this_interface: *mut c_void,
    id: Steinberg_Vst_AttrID,
    data: *mut *const c_void,
    size_in_bytes: *mut Steinberg_uint32,
) -> Steinberg_tresult {
    if data.is_null() || size_in_bytes.is_null() {
        return kInvalidArgument;
    }
    let Some(key) = (unsafe { attr_id_to_key(id) }) else {
        return kInvalidArgument;
    };
    let this = unsafe { &*(this_interface as *const Vst3HostAttributeList) };
    if let Ok(st) = this.store.lock() {
        if let Some(vec) = st.binaries.get(&key) {
            unsafe {
                *data = vec.as_ptr() as *const c_void;
                *size_in_bytes = vec.len() as u32;
            }
            return kResultOk;
        }
    }
    kResultFalse
}

#[repr(C)]
pub(crate) struct Vst3HostMessage {
    lp_vtbl: *const Steinberg_Vst_IMessageVtbl,
    ref_count: std::sync::atomic::AtomicU32,
    message_id: Mutex<CString>,
    attrs: *mut Vst3HostAttributeList,
}

static HOST_MESSAGE_VTBL: Steinberg_Vst_IMessageVtbl = Steinberg_Vst_IMessageVtbl {
    queryInterface: host_msg_query_interface,
    addRef: host_msg_add_ref,
    release: host_msg_release,
    getMessageID: host_msg_get_id,
    setMessageID: host_msg_set_id,
    getAttributes: host_msg_get_attributes,
};

impl Vst3HostMessage {
    fn new_raw() -> *mut Self {
        Box::into_raw(Box::new(Self {
            lp_vtbl: &HOST_MESSAGE_VTBL,
            ref_count: std::sync::atomic::AtomicU32::new(1),
            message_id: Mutex::new(CString::default()),
            attrs: Vst3HostAttributeList::new_raw(),
        }))
    }
}

impl Drop for Vst3HostMessage {
    fn drop(&mut self) {
        if !self.attrs.is_null() {
            unsafe {
                attr_list_release(self.attrs as *mut c_void);
            }
            self.attrs = ptr::null_mut();
        }
    }
}

unsafe extern "system" fn host_msg_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_Vst_IMessage_iid {
            host_msg_add_ref(this_interface);
            *obj = this_interface;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn host_msg_add_ref(this_interface: *mut c_void) -> Steinberg_uint32 {
    let this = unsafe { &*(this_interface as *const Vst3HostMessage) };
    this.ref_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1
}

unsafe extern "system" fn host_msg_release(this_interface: *mut c_void) -> Steinberg_uint32 {
    let this = unsafe { &*(this_interface as *const Vst3HostMessage) };
    let prev = this
        .ref_count
        .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    if prev == 1 {
        unsafe {
            let _ = Box::from_raw(this_interface as *mut Vst3HostMessage);
        }
        0
    } else {
        prev - 1
    }
}

unsafe extern "system" fn host_msg_get_id(this_interface: *mut c_void) -> Steinberg_FIDString {
    let this = unsafe { &*(this_interface as *const Vst3HostMessage) };
    if let Ok(guard) = this.message_id.lock() {
        guard.as_ptr()
    } else {
        ptr::null()
    }
}

unsafe extern "system" fn host_msg_set_id(this_interface: *mut c_void, id: Steinberg_FIDString) {
    let this = unsafe { &*(this_interface as *const Vst3HostMessage) };
    let s = if id.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(id).to_string_lossy().into_owned() }
    };
    if let Ok(mut guard) = this.message_id.lock() {
        if let Ok(cs) = CString::new(s) {
            *guard = cs;
        }
    }
}

unsafe extern "system" fn host_msg_get_attributes(
    this_interface: *mut c_void,
) -> *mut Steinberg_Vst_IAttributeList {
    let this = unsafe { &*(this_interface as *const Vst3HostMessage) };
    this.attrs as *mut Steinberg_Vst_IAttributeList
}
