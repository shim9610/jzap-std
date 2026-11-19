//! Isolated-process std comparison. Instrumentation is disabled for timings.
use jzap::{
    TryClone,
    boxed::Box as JBox,
    budget::{AllocationPolicy, Budget, MemoryBudget, Unmetered},
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    hint::black_box,
    time::Instant,
};
#[derive(Clone, Copy, Default)]
struct Stats {
    enabled: bool,
    usable: bool,
    calls: usize,
    live: usize,
    peak: usize,
    total: usize,
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
fn snapshot() -> Stats {
    STATS.with(Cell::get)
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let implementation = &args[1];
    let n: usize = args[2].parse().unwrap();
    let mode = &args[3];
    let memory = mode != "time";
    let budget = MemoryBudget::new(usize::MAX);
    STATS.with(|s| {
        s.set(Stats {
            enabled: memory,
            usable: mode == "usable",
            ..Stats::default()
        })
    });
    let t = Instant::now();
    let (create_ns, clone_ns, drop_ns, built, cloned, handle) = if implementation == "std" {
        let mut source = Vec::with_capacity(n);
        for i in 0..n {
            source.push(black_box(std::boxed::Box::new(black_box(i as u64))))
        }
        let create = t.elapsed().as_nanos();
        let built = snapshot();
        let t = Instant::now();
        let mut target = Vec::with_capacity(n);
        for v in &source {
            target.push(black_box(black_box(v).clone()))
        }
        let clone = t.elapsed().as_nanos();
        let cloned = snapshot();
        let t = Instant::now();
        black_box(&target);
        drop(target);
        drop(source);
        (
            create,
            clone,
            t.elapsed().as_nanos(),
            built,
            cloned,
            std::mem::size_of::<std::boxed::Box<u64>>(),
        )
    } else if implementation == "unmetered" {
        measure_jzap(n, Unmetered)
    } else {
        measure_jzap(n, Budget::Limited(&budget))
    };
    let end = snapshot();
    STATS.with(|s| {
        let mut v = s.get();
        v.enabled = false;
        s.set(v)
    });
    assert_eq!(budget.occupied(), 0);
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let rss = status
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("0");
    let hwm = status
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("0");
    println!(
        "{{\"implementation\":\"{implementation}\",\"n\":{n},\"mode\":\"{mode}\",\"create_ns\":{create_ns},\"clone_ns\":{clone_ns},\"drop_ns\":{drop_ns},\"build_live\":{},\"clone_live\":{},\"peak\":{},\"total\":{},\"calls\":{},\"end_live\":{},\"handle_bytes\":{handle},\"rss\":\"{rss}\",\"hwm\":\"{hwm}\"}}",
        built.live, cloned.live, end.peak, end.total, end.calls, end.live
    );
}

fn measure_jzap<P: AllocationPolicy>(
    n: usize,
    policy: P,
) -> (u128, u128, u128, Stats, Stats, usize) {
    let t = Instant::now();
    let mut source = Vec::with_capacity(n);
    for i in 0..n {
        source.push(black_box(
            JBox::try_new_in(black_box(i as u64), policy.try_clone().unwrap())
                .ok()
                .unwrap(),
        ))
    }
    let create = t.elapsed().as_nanos();
    let built = snapshot();
    let t = Instant::now();
    let mut target = Vec::with_capacity(n);
    for v in &source {
        target.push(black_box(black_box(v).try_clone().unwrap()))
    }
    let clone = t.elapsed().as_nanos();
    let cloned = snapshot();
    let t = Instant::now();
    black_box(&target);
    drop(target);
    drop(source);
    (
        create,
        clone,
        t.elapsed().as_nanos(),
        built,
        cloned,
        std::mem::size_of::<JBox<u64, P>>(),
    )
}
