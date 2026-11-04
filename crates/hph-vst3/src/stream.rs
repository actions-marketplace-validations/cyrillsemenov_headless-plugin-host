use hph_vst3_sys::*;
use std::ffi::c_void;
use std::ptr;

#[repr(C)]
pub(crate) struct Vst3MemoryStream {
    lp_vtbl: *const Steinberg_IBStreamVtbl,
    pub(crate) data: Vec<u8>,
    pub(crate) cursor: usize,
}

static MEMORY_STREAM_VTBL: Steinberg_IBStreamVtbl = Steinberg_IBStreamVtbl {
    queryInterface: stream_query_interface,
    addRef: stream_add_ref,
    release: stream_release,
    read: stream_read,
    write: stream_write,
    seek: stream_seek,
    tell: stream_tell,
};

impl Vst3MemoryStream {
    pub(crate) fn new_empty() -> Self {
        Self {
            lp_vtbl: &MEMORY_STREAM_VTBL,
            data: Vec::new(),
            cursor: 0,
        }
    }

    pub(crate) fn from_slice(bytes: &[u8]) -> Self {
        Self {
            lp_vtbl: &MEMORY_STREAM_VTBL,
            data: bytes.to_vec(),
            cursor: 0,
        }
    }

    pub(crate) fn as_ibstream_ptr(&mut self) -> *mut Steinberg_IBStream {
        self as *mut Self as *mut Steinberg_IBStream
    }
}

unsafe extern "system" fn stream_query_interface(
    this_interface: *mut c_void,
    iid: *const Steinberg_TUID,
    obj: *mut *mut c_void,
) -> Steinberg_tresult {
    unsafe {
        if iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let target = &*iid;
        if *target == Steinberg_FUnknown_iid || *target == Steinberg_IBStream_iid {
            *obj = this_interface;
            return kResultOk;
        }
        *obj = ptr::null_mut();
        kNoInterface
    }
}

unsafe extern "system" fn stream_add_ref(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn stream_release(_this_interface: *mut c_void) -> Steinberg_uint32 {
    1
}

unsafe extern "system" fn stream_read(
    this_interface: *mut c_void,
    buffer: *mut c_void,
    num_bytes: Steinberg_int32,
    num_bytes_read: *mut Steinberg_int32,
) -> Steinberg_tresult {
    if buffer.is_null() || num_bytes < 0 {
        return kInvalidArgument;
    }
    let stream = unsafe { &mut *(this_interface as *mut Vst3MemoryStream) };
    let available = stream.data.len().saturating_sub(stream.cursor);
    let to_read = (num_bytes as usize).min(available);
    if to_read > 0 {
        unsafe {
            ptr::copy_nonoverlapping(
                stream.data.as_ptr().add(stream.cursor),
                buffer as *mut u8,
                to_read,
            );
        }
        stream.cursor += to_read;
    }
    if !num_bytes_read.is_null() {
        unsafe { *num_bytes_read = to_read as i32 };
    }
    kResultOk
}

unsafe extern "system" fn stream_write(
    this_interface: *mut c_void,
    buffer: *mut c_void,
    num_bytes: Steinberg_int32,
    num_bytes_written: *mut Steinberg_int32,
) -> Steinberg_tresult {
    if buffer.is_null() || num_bytes < 0 {
        return kInvalidArgument;
    }
    let stream = unsafe { &mut *(this_interface as *mut Vst3MemoryStream) };
    let count = num_bytes as usize;
    let src = unsafe { std::slice::from_raw_parts(buffer as *const u8, count) };
    let end_pos = stream.cursor + count;
    if end_pos > stream.data.len() {
        stream.data.resize(end_pos, 0);
    }
    stream.data[stream.cursor..end_pos].copy_from_slice(src);
    stream.cursor = end_pos;
    if !num_bytes_written.is_null() {
        unsafe { *num_bytes_written = count as i32 };
    }
    kResultOk
}

#[allow(non_upper_case_globals)]
unsafe extern "system" fn stream_seek(
    this_interface: *mut c_void,
    pos: Steinberg_int64,
    mode: Steinberg_int32,
    result: *mut Steinberg_int64,
) -> Steinberg_tresult {
    let stream = unsafe { &mut *(this_interface as *mut Vst3MemoryStream) };
    let base: i64 = match mode {
        Steinberg_IBStream_IStreamSeekMode_kIBSeekSet => 0,
        Steinberg_IBStream_IStreamSeekMode_kIBSeekCur => stream.cursor as i64,
        Steinberg_IBStream_IStreamSeekMode_kIBSeekEnd => stream.data.len() as i64,
        _ => return kInvalidArgument,
    };
    let next = base + pos;
    if next < 0 || (next as usize) > stream.data.len() {
        return kInvalidArgument;
    }
    stream.cursor = next as usize;
    if !result.is_null() {
        unsafe { *result = stream.cursor as i64 };
    }
    kResultOk
}

unsafe extern "system" fn stream_tell(
    this_interface: *mut c_void,
    pos: *mut Steinberg_int64,
) -> Steinberg_tresult {
    if pos.is_null() {
        return kInvalidArgument;
    }
    let stream = unsafe { &*(this_interface as *const Vst3MemoryStream) };
    unsafe { *pos = stream.cursor as i64 };
    kResultOk
}
