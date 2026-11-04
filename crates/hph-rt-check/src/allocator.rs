use std::cell::Cell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

thread_local! {
    pub(crate) static RT_GUARD_ARMED: Cell<bool> = const { Cell::new(false) };
}

pub(crate) static ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
pub(crate) static FREE_CALLS: AtomicUsize = AtomicUsize::new(0);
pub(crate) static BYTES_ALLOCATED: AtomicU64 = AtomicU64::new(0);

/// Global `GlobalAlloc` wrapper that users or test binaries can install,
/// while simultaneously reading OS heap statistics so allocations inside
/// foreign C/C++/Rust plugin `.dylib`/`.dll`/`.so` binaries are caught too!
pub struct RtCheckingAllocator<A = std::alloc::System>(pub A);

unsafe impl<A: std::alloc::GlobalAlloc> std::alloc::GlobalAlloc for RtCheckingAllocator<A> {
    #[inline]
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        if RT_GUARD_ARMED.try_with(|c| c.get()).unwrap_or(false) {
            ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES_ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        self.0.alloc(layout)
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        if RT_GUARD_ARMED.try_with(|c| c.get()).unwrap_or(false) {
            FREE_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        self.0.dealloc(ptr, layout)
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        if RT_GUARD_ARMED.try_with(|c| c.get()).unwrap_or(false) {
            ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES_ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        self.0.alloc_zeroed(layout)
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
        if RT_GUARD_ARMED.try_with(|c| c.get()).unwrap_or(false) {
            ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            BYTES_ALLOCATED.fetch_add(new_size as u64, Ordering::Relaxed);
        }
        self.0.realloc(ptr, layout, new_size)
    }
}
