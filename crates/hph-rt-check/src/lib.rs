pub mod allocator;
pub mod guard;
pub mod platform;

pub use allocator::RtCheckingAllocator;
pub use guard::{with_rt_alloc_guard, RtAllocGuard, RtAllocStats};

#[cfg(test)]
mod tests {
    use super::*;

    #[global_allocator]
    static GLOBAL: RtCheckingAllocator = RtCheckingAllocator(std::alloc::System);

    #[test]
    fn test_rt_alloc_detector_catches_heap_alloc() {
        // 1. Zero allocations when doing pure stack math
        let (sum, clean_stats) = with_rt_alloc_guard(|| {
            let mut acc = 0.0f32;
            for i in 0..128 {
                acc += (i as f32) * 0.5;
            }
            acc
        });
        assert!(sum > 0.0);
        assert_eq!(clean_stats.alloc_calls, 0);
        assert_eq!(clean_stats.free_calls, 0);

        // 2. Catches Box/Vec allocation and drop inside guarded scope
        let (_, dirty_stats) = with_rt_alloc_guard(|| {
            let mut v: Vec<u64> = Vec::with_capacity(64);
            v.push(42);
            std::hint::black_box(&v);
        });
        assert!(
            dirty_stats.alloc_calls >= 1,
            "Expected >= 1 alloc call, got {:?}",
            dirty_stats
        );
        assert!(
            dirty_stats.free_calls >= 1,
            "Expected >= 1 free call, got {:?}",
            dirty_stats
        );
        assert!(dirty_stats.bytes_allocated >= 512);
    }
}
