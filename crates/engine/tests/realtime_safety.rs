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
    // A low and a high note together keep the tracker busy with notes.
    let dry: Vec<f32> = sine(41.2, 0.4, 0.5)
        .iter()
        .zip(sine(130.8, 0.3, 0.5))
        .map(|(a, b)| a + b)
        .collect();
    let mut left = vec![0.0; MAX_BLOCK];
    let mut right = vec![0.0; MAX_BLOCK];

    let mut notes_seen = false;
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for (i, block) in dry.chunks(MAX_BLOCK).enumerate() {
        engine.set_param(Param::OutputLevel, i as f32 * 0.1);
        engine.set_noise_gate(0.001 * i as f32);
        let n = block.len();
        engine.process(block, &mut left[..n], &mut right[..n]);
        let readout = std::hint::black_box(engine.readout());
        notes_seen |= !readout.notes.is_empty();
    }
    let after = ALLOCATIONS.load(Ordering::Relaxed);

    assert_eq!(after - before, 0, "allocations in the processing path");
    assert!(notes_seen, "the tracker never detected a note");
}
