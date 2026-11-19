use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
};
static MOVING: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static USABLE: AtomicUsize = AtomicUsize::new(0);
static UPEAK: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);
unsafe extern "C" {
    fn malloc_usable_size(ptr: *mut core::ffi::c_void) -> usize;
}
struct Allocator;
#[global_allocator]
static A: Allocator = Allocator;
fn add(n: usize, u: usize) {
    let v = LIVE.fetch_add(n, Relaxed) + n;
    PEAK.fetch_max(v, Relaxed);
    let x = USABLE.fetch_add(u, Relaxed) + u;
    UPEAK.fetch_max(x, Relaxed);
    TOTAL.fetch_add(n, Relaxed);
    CALLS.fetch_add(1, Relaxed);
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() && ACTIVE.load(Relaxed) {
            add(l.size(), unsafe { malloc_usable_size(p.cast()) });
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if ACTIVE.load(Relaxed) {
            LIVE.fetch_sub(l.size(), Relaxed);
            USABLE.fetch_sub(unsafe { malloc_usable_size(p.cast()) }, Relaxed);
        }
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let active = ACTIVE.load(Relaxed);
        let old = if active {
            unsafe { malloc_usable_size(p.cast()) }
        } else {
            0
        };
        if active && MOVING.load(Relaxed) {
            let new_layout = Layout::from_size_align(n, l.align()).unwrap();
            let q = unsafe { System.alloc(new_layout) };
            if !q.is_null() {
                // Both allocations really exist while this peak is observed.
                add(n, unsafe { malloc_usable_size(q.cast()) });
                unsafe {
                    std::ptr::copy_nonoverlapping(p, q, l.size().min(n));
                }
                unsafe {
                    System.dealloc(p, l);
                }
                LIVE.fetch_sub(l.size(), Relaxed);
                USABLE.fetch_sub(old, Relaxed);
            }
            return q;
        }
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() && active {
            LIVE.fetch_sub(l.size(), Relaxed);
            USABLE.fetch_sub(old, Relaxed);
            add(n, unsafe { malloc_usable_size(q.cast()) });
        }
        q
    }
}
pub fn start(moving: bool) {
    MOVING.store(moving, Relaxed);
    for a in [&LIVE, &PEAK, &USABLE, &UPEAK, &TOTAL, &CALLS] {
        a.store(0, Relaxed);
    }
    ACTIVE.store(true, Relaxed);
}
// Snapshots at allocator-call boundaries: hidden internal realloc transients are not observed.
pub fn snapshot() -> [usize; 6] {
    [
        LIVE.load(Relaxed),
        PEAK.load(Relaxed),
        USABLE.load(Relaxed),
        UPEAK.load(Relaxed),
        TOTAL.load(Relaxed),
        CALLS.load(Relaxed),
    ]
}
pub fn stop() -> [usize; 6] {
    ACTIVE.store(false, Relaxed);
    snapshot()
}
