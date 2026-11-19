//! Run with RUSTFLAGS=-Cpanic=abort: allocation denial must return normally.
use jzap::{
    TryClone,
    boxed::Box,
    budget::{Budget, MemoryBudget},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};
static DENY: AtomicUsize = AtomicUsize::new(0);
struct Refusing;
unsafe impl GlobalAlloc for Refusing {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if DENY.load(Ordering::Relaxed) > 0 && DENY.fetch_sub(1, Ordering::Relaxed) == 1 {
            core::ptr::null_mut()
        } else {
            unsafe { System.alloc(l) }
        }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOC: Refusing = Refusing;
fn main() {
    let budget = MemoryBudget::new(1024);
    let source = Box::try_new_in(17u64, Budget::Limited(&budget))
        .ok()
        .unwrap();
    DENY.store(1, Ordering::Relaxed);
    let error = source.try_clone().err().unwrap();
    DENY.store(0, Ordering::Relaxed);
    assert!(matches!(error, jzap::Error::Allocation { .. }));
    assert_eq!(*source, 17);
    assert_eq!(budget.occupied(), 8);
    drop(source);
    assert_eq!(budget.occupied(), 0);
    DENY.store(1, Ordering::Relaxed);
    let slice = Box::<[u8]>::try_copy_from_slice(&[1, 2, 3]);
    DENY.store(0, Ordering::Relaxed);
    assert!(matches!(slice, Err(jzap::Error::Allocation { .. })));
    DENY.store(1, Ordering::Relaxed);
    let zeroed = Box::<[u8]>::try_new_zeroed_slice(128);
    DENY.store(0, Ordering::Relaxed);
    assert!(matches!(zeroed, Err(jzap::Error::Allocation { .. })));
    for fail in 1..=5 {
        DENY.store(fail, Ordering::Relaxed);
        let collected =
            Box::<[u8]>::try_from_iter_in((0..5).filter(|_| true), jzap::budget::Unmetered);
        DENY.store(0, Ordering::Relaxed);
        assert!(matches!(collected, Err(jzap::Error::Allocation { .. })));
    }
}
