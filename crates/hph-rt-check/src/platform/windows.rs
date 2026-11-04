use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct HEAP_SUMMARY {
    pub cb: u32,
    pub cb_allocated: usize,
    pub cb_committed: usize,
    pub cb_reserved: usize,
    pub cb_max_reserve: usize,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetProcessHeap() -> *mut c_void;
    fn HeapSummary(hHeap: *mut c_void, dwFlags: u32, lpSummary: *mut HEAP_SUMMARY) -> i32;
}

#[inline]
pub fn snapshot_process_heap() -> HEAP_SUMMARY {
    let mut summary = HEAP_SUMMARY {
        cb: std::mem::size_of::<HEAP_SUMMARY>() as u32,
        ..Default::default()
    };
    unsafe {
        let heap = GetProcessHeap();
        if !heap.is_null() {
            let _ = HeapSummary(heap, 0, &mut summary);
        }
    }
    summary
}
