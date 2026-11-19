//! Allocation observability for the String benchmark; Linux System allocator only.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub active: bool,
    pub calls: usize,
    pub live: usize,
    pub peak: usize,
    pub actual: usize,
    pub actual_peak: usize,
}
thread_local! {static STATS:Cell<Stats>=Cell::new(Stats::default());}
unsafe extern "C" {
    fn malloc_usable_size(ptr: *mut core::ffi::c_void) -> usize;
}
pub struct Allocator;
#[global_allocator]
static ALLOC: Allocator = Allocator;
fn update(old: usize, new: usize, actual_old: usize, actual_new: usize, call: bool) {
    let _ = STATS.try_with(|s| {
        let mut v = s.get();
        if v.active {
            v.calls += usize::from(call);
            v.live = v.live + new - old;
            v.actual = v.actual + actual_new - actual_old;
            v.peak = v.peak.max(v.live);
            v.actual_peak = v.actual_peak.max(v.actual);
            s.set(v);
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            update(
                0,
                l.size(),
                0,
                unsafe { malloc_usable_size(p.cast()) },
                true,
            );
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        update(
            l.size(),
            0,
            unsafe { malloc_usable_size(p.cast()) },
            0,
            false,
        );
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let before = unsafe { malloc_usable_size(p.cast()) };
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            update(
                l.size(),
                n,
                before,
                unsafe { malloc_usable_size(q.cast()) },
                true,
            );
        }
        q
    }
}
pub fn start() {
    STATS.with(|s| {
        assert!(!s.get().active);
        s.set(Stats {
            active: true,
            ..Stats::default()
        });
    });
}
pub fn snapshot() -> Stats {
    STATS.with(Cell::get)
}
pub fn stop() -> Stats {
    let v = snapshot();
    STATS.with(|s| s.set(Stats::default()));
    v
}
