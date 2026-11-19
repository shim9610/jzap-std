use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
#[derive(Clone, Copy, Default, Debug)]
pub struct State {
    pub active: bool,
    pub calls: usize,
    pub fail: usize,
    pub live: isize,
    pub peak: isize,
}
thread_local! { pub(crate) static STATE:Cell<State>=const { Cell::new(State { active:false,calls:0,fail:usize::MAX,live:0,peak:0 }) }; }
thread_local! { pub(super) static WATCH:Cell<(usize, usize)>=const { Cell::new((0, 0)) }; }
pub struct FaultAllocator;
fn refuse() -> bool {
    STATE
        .try_with(|s| {
            let mut v = s.get();
            if !v.active {
                return false;
            }
            v.calls += 1;
            let fail = v.calls == v.fail;
            s.set(v);
            fail
        })
        .unwrap_or(false)
}
fn account(n: isize) {
    let _ = STATE.try_with(|s| {
        let mut v = s.get();
        if v.active {
            v.live += n;
            v.peak = v.peak.max(v.live);
            s.set(v)
        }
    });
}
unsafe impl GlobalAlloc for FaultAllocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if refuse() {
            return std::ptr::null_mut();
        }
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            account(l.size() as isize)
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        if refuse() {
            return std::ptr::null_mut();
        }
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            account(l.size() as isize)
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        let _ = WATCH.try_with(|w| {
            let (address, count) = w.get();
            if address == p.addr() {
                w.set((address, count + 1));
            }
        });
        account(-(l.size() as isize));
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if refuse() {
            return std::ptr::null_mut();
        }
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            account(n as isize - l.size() as isize)
        }
        q
    }
}
pub fn track<T>(fail: usize, f: impl FnOnce() -> T) -> (T, State) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            STATE.with(|s| {
                let mut v = s.get();
                v.active = false;
                s.set(v)
            })
        }
    }
    STATE.with(|s| {
        assert!(!s.get().active);
        s.set(State {
            active: true,
            fail,
            ..State::default()
        })
    });
    let guard = Reset;
    let value = f();
    let state = STATE.with(Cell::get);
    drop(guard);
    (value, state)
}
