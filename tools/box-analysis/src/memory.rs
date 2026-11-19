use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
#[derive(Clone, Copy, Default)]
pub struct Stats {
    pub enabled: bool,
    pub usable: bool,
    pub calls: usize,
    pub live: usize,
    pub peak: usize,
    pub total: usize,
}
thread_local! {static STATS:Cell<Stats>=const{Cell::new(Stats{enabled:false,usable:false,calls:0,live:0,peak:0,total:0})};}
#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn malloc_usable_size(p: *mut core::ffi::c_void) -> usize;
}
fn size(p: *mut u8, l: Layout, usable: bool) -> usize {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    if usable {
        return unsafe { malloc_usable_size(p.cast()) };
    }
    let _ = (p, usable);
    l.size()
}
fn add(p: *mut u8, l: Layout) {
    if p.is_null() {
        return;
    }
    let _ = STATS.try_with(|s| {
        let mut v = s.get();
        if v.enabled {
            let n = size(p, l, v.usable);
            v.calls += 1;
            v.live += n;
            v.total += n;
            v.peak = v.peak.max(v.live);
            s.set(v)
        }
    });
}
fn sub(p: *mut u8, l: Layout) {
    let _ = STATS.try_with(|s| {
        let mut v = s.get();
        if v.enabled {
            v.live -= size(p, l, v.usable);
            s.set(v)
        }
    });
}
struct Meter;
unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        add(p, l);
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        add(p, l);
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        sub(p, l);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        // Bench workloads preallocate handles and do not reallocate during measurement.
        sub(p, l);
        let q = unsafe { System.realloc(p, l, n) };
        if q.is_null() {
            add(p, l)
        } else {
            add(q, Layout::from_size_align(n, l.align()).unwrap())
        }
        q
    }
}
#[global_allocator]
static ALLOC: Meter = Meter;
pub fn snapshot() -> Stats {
    STATS.with(Cell::get)
}

pub fn start(usable: bool) {
    STATS.with(|s| {
        s.set(Stats {
            enabled: true,
            usable,
            ..Stats::default()
        })
    });
}
pub fn stop() {
    STATS.with(|s| {
        let mut value = s.get();
        value.enabled = false;
        s.set(value)
    });
}
