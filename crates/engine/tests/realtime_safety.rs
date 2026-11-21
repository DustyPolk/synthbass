//! Runs alone in its own test binary so the allocation counter sees only
//! this test.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use common::*;
use synthbass_engine::Param;

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn processing_parameter_changes_and_readout_allocate_nothing() {
    let mut engine = engine();
    let dry = sine(110.0, 1.5, 0.5);
    let mut left = vec![0.0; MAX_BLOCK];
    let mut right = vec![0.0; MAX_BLOCK];

    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for (i, block) in dry.chunks(MAX_BLOCK).enumerate() {
        engine.set_param(Param::OutputLevel, i as f32 * 0.1);
        let n = block.len();
        engine.process(block, &mut left[..n], &mut right[..n]);
        std::hint::black_box(engine.readout());
    }
    let after = ALLOCATIONS.load(Ordering::Relaxed);

    assert_eq!(after - before, 0, "allocations in the processing path");
}
