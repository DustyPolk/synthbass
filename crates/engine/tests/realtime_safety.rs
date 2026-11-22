//! Runs alone in its own test binary so the counting allocator does not
//! slow other tests down.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use common::*;
use synthbass_engine::{Param, Patch};

struct CountingAllocator;

thread_local! {
    /// Allocations made by the current thread. Per thread, because the test
    /// harness's own threads allocate while the test runs (its main thread
    /// does when it starts waiting for the result), and a shared count picked
    /// those up as if the engine had allocated.
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn allocations() -> usize {
    ALLOCATIONS.with(Cell::get)
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // `try_with`: a thread being torn down may allocate after its
        // thread-locals are gone.
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn processing_parameter_and_patch_changes_and_readout_allocate_nothing() {
    let mut engine = engine();
    engine.set_param(Param::DirectLevel, 1.0);
    engine.set_param(Param::TrackedLevel, 1.0);
    // A low and a high note together keep the tracker busy with notes.
    let dry: Vec<f32> = sine(41.2, 0.4, 0.5)
        .iter()
        .zip(sine(130.8, 0.3, 0.5))
        .map(|(a, b)| a + b)
        .collect();
    let mut left = vec![0.0; MAX_BLOCK];
    let mut right = vec![0.0; MAX_BLOCK];
    // Two patches to switch between mid-stream.
    let mut bright = Patch::new("Bright");
    bright.set(Param::TrackedCutoff, 4_000.0);
    bright.set(Param::TrackedOsc2Octave, 1.0);
    bright.set(Param::TrackedLevel, 1.0);
    let mut dark = Patch::new("Dark");
    dark.set(Param::TrackedCutoff, 100.0);
    dark.set(Param::DirectLevel, 1.0);
    let patches = [bright, dark];

    let mut notes_seen = false;
    let before = allocations();
    for (i, block) in dry.chunks(MAX_BLOCK).enumerate() {
        engine.set_param(Param::OutputLevel, i as f32 * 0.1);
        engine.set_param(Param::DirectCutoff, 100.0 * (i + 1) as f32);
        engine.set_param(Param::TrackedOsc1Waveshape, (i % 2) as f32);
        engine.set_noise_gate(0.001 * i as f32);
        if i % 3 == 0 {
            engine.apply_patch(&patches[i / 3 % 2]);
        }
        let n = block.len();
        engine.process(block, &mut left[..n], &mut right[..n]);
        let readout = std::hint::black_box(engine.readout());
        notes_seen |= !readout.notes.is_empty() && !readout.voices.is_empty();
    }
    let after = allocations();

    assert_eq!(after - before, 0, "allocations in the processing path");
    assert!(notes_seen, "the tracker never detected a note");
}
