use crate::support::input;
use nmr::ExecutionContext;
use nmr::processing::{
    FourierTransform, ProcessingOperation as Op, ProcessingOptions, ProcessingPlan,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct CountedAllocator;
pub(crate) static COUNTING: AtomicBool = AtomicBool::new(false);
pub(crate) static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
// This isolated, synchronous probe counts allocations; it forwards every allocation
// and deallocation unchanged to System and never accesses the allocated memory.
unsafe impl GlobalAlloc for CountedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: CountedAllocator = CountedAllocator;
pub(crate) fn cancelled_nus_preparation_does_not_allocate_index_or_sample_buffers() {
    let input = input();
    let plan = ProcessingPlan::new(vec![Op::FourierTransform {
        axis: 1,
        transform: FourierTransform::default(),
    }])
    .unwrap();
    let token = nmr::CancellationToken::new();
    token.cancel();
    let mut context = ExecutionContext::default().with_cancellation(token);
    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    let result = nmr::processing::NusSettings::default().prepare_with_context(
        &input,
        plan,
        ProcessingOptions::new(),
        &mut context,
    );
    COUNTING.store(false, Ordering::Relaxed);
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    assert_eq!(
        result.unwrap_err().code(),
        nmr::processing::ProcessingErrorCode::Cancelled
    );
    // The sole permitted allocation is the structured error's location wrapper.
    assert!(
        allocations <= 1,
        "pre-cancelled preparation allocated {allocations} times"
    );
    println!("Pre-cancelled NUS preparation: no index/sample allocation");
}
