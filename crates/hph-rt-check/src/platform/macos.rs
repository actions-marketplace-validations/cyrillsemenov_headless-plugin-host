use libc::{c_uint, c_void, size_t};

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct malloc_statistics_t {
    pub blocks_in_use: c_uint,
    pub size_in_use: size_t,
    pub max_size_in_use: size_t,
    pub size_allocated: size_t,
}

extern "C" {
    fn malloc_zone_statistics(zone: *mut c_void, stats: *mut malloc_statistics_t);
}

#[inline]
pub fn snapshot_process_heap() -> malloc_statistics_t {
    let mut st = malloc_statistics_t::default();
    unsafe {
        // Passing null queries across all malloc zones in the process
        malloc_zone_statistics(std::ptr::null_mut(), &mut st);
    }
    st
}
