use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;

use crate::allocator::{ALLOC_CALLS, BYTES_ALLOCATED, FREE_CALLS, RT_GUARD_ARMED};
#[cfg(any(target_os = "macos", all(target_os = "linux", target_env = "gnu")))]
use crate::platform;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RtAllocStats {
    pub alloc_calls: usize,
    pub free_calls: usize,
    pub bytes_allocated: u64,
}

impl RtAllocStats {
    pub fn has_violations(&self) -> bool {
        self.alloc_calls > 0 || self.free_calls > 0
    }
}

/// RAII Guard that intercepts heap allocations/deallocations on the current thread
/// and snapshots OS heap statistics across loaded plugin shared libraries.
pub struct RtAllocGuard {
    start_allocs: usize,
    start_frees: usize,
    start_bytes: u64,
    #[cfg(target_os = "macos")]
    start_zone: platform::macos::malloc_statistics_t,
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    start_mallinfo: platform::linux::mallinfo2,
}

impl Default for RtAllocGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl RtAllocGuard {
    pub fn new() -> Self {
        let start_allocs = ALLOC_CALLS.load(Ordering::Relaxed);
        let start_frees = FREE_CALLS.load(Ordering::Relaxed);
        let start_bytes = BYTES_ALLOCATED.load(Ordering::Relaxed);
        #[cfg(target_os = "macos")]
        let start_zone = platform::macos::snapshot_process_heap();
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        let start_mallinfo = platform::linux::snapshot_process_heap();
        RT_GUARD_ARMED.with(|c| c.set(true));
        Self {
            start_allocs,
            start_frees,
            start_bytes,
            #[cfg(target_os = "macos")]
            start_zone,
            #[cfg(all(target_os = "linux", target_env = "gnu"))]
            start_mallinfo,
        }
    }

    #[allow(unused_mut)]
    pub fn finish(self) -> RtAllocStats {
        RT_GUARD_ARMED.with(|c| c.set(false));
        let mut alloc_calls = ALLOC_CALLS
            .load(Ordering::Relaxed)
            .saturating_sub(self.start_allocs);
        let mut free_calls = FREE_CALLS
            .load(Ordering::Relaxed)
            .saturating_sub(self.start_frees);
        let mut bytes_allocated = BYTES_ALLOCATED
            .load(Ordering::Relaxed)
            .saturating_sub(self.start_bytes);

        #[cfg(target_os = "macos")]
        {
            let end_zone = platform::macos::snapshot_process_heap();
            if alloc_calls == 0 && end_zone.blocks_in_use > self.start_zone.blocks_in_use {
                alloc_calls = (end_zone.blocks_in_use - self.start_zone.blocks_in_use) as usize;
                bytes_allocated = end_zone
                    .size_in_use
                    .saturating_sub(self.start_zone.size_in_use)
                    as u64;
            } else if free_calls == 0 && end_zone.blocks_in_use < self.start_zone.blocks_in_use {
                free_calls = (self.start_zone.blocks_in_use - end_zone.blocks_in_use) as usize;
            }
        }

        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            let end_mi = platform::linux::snapshot_process_heap();
            if alloc_calls == 0 && end_mi.uordblks > self.start_mallinfo.uordblks {
                alloc_calls = 1;
                bytes_allocated = (end_mi.uordblks - self.start_mallinfo.uordblks) as u64;
            } else if free_calls == 0 && end_mi.uordblks < self.start_mallinfo.uordblks {
                free_calls = 1;
            }
        }

        std::mem::forget(self);
        RtAllocStats {
            alloc_calls,
            free_calls,
            bytes_allocated,
        }
    }
}

impl Drop for RtAllocGuard {
    fn drop(&mut self) {
        RT_GUARD_ARMED.with(|c| c.set(false));
    }
}

/// Run a closure under the real-time allocation detector and return `(result, RtAllocStats)`.
pub fn with_rt_alloc_guard<F, R>(f: F) -> (R, RtAllocStats)
where
    F: FnOnce() -> R,
{
    let guard = RtAllocGuard::new();
    let res = f();
    let stats = guard.finish();
    (res, stats)
}
