#[allow(dead_code)]
#[path = "../../../tests/common/mod.rs"]
mod common;
use jzap::{TryClone, boxed::Box as JBox};
use std::cell::Cell;
#[global_allocator]
static ALLOC: common::FaultAllocator = common::FaultAllocator;
struct Value<'a> {
    dropped: &'a Cell<usize>,
    cloned: &'a Cell<usize>,
}
impl Drop for Value<'_> {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}
impl jzap::TryClone for Value<'_> {
    fn try_clone(&self) -> Result<Self, jzap::Error> {
        self.cloned.set(self.cloned.get() + 1);
        Ok(Self {
            dropped: self.dropped,
            cloned: self.cloned,
        })
    }
}
impl varlit_core::TryClone for Value<'_> {
    fn try_clone(&self) -> varlit_core::Result<Self> {
        self.cloned.set(self.cloned.get() + 1);
        Ok(Self {
            dropped: self.dropped,
            cloned: self.cloned,
        })
    }
}
#[test]
fn actual_varlit_and_jzap_input_rejection_contracts() {
    let dropped = Cell::new(0);
    let cloned = Cell::new(0);
    let (result, stats) = common::track(1, || {
        varlit_core::try_box(Value {
            dropped: &dropped,
            cloned: &cloned,
        })
    });
    assert!(result.is_err());
    assert_eq!(dropped.get(), 1);
    assert_eq!((stats.calls, stats.live), (1, 0));
    let (result, stats) = common::track(1, || {
        JBox::try_new(Value {
            dropped: &dropped,
            cloned: &cloned,
        })
    });
    let rejected = result.err().unwrap();
    assert_eq!(dropped.get(), 1);
    assert_eq!((stats.calls, stats.live), (1, 0));
    drop(rejected);
    assert_eq!(dropped.get(), 2);
}
#[test]
fn both_reserve_before_clone_callback_and_cleanup() {
    let dropped = Cell::new(0);
    let cloned = Cell::new(0);
    let v = varlit_core::try_box(Value {
        dropped: &dropped,
        cloned: &cloned,
    })
    .unwrap();
    let j = JBox::try_new(Value {
        dropped: &dropped,
        cloned: &cloned,
    })
    .ok()
    .unwrap();
    let (r, s) = common::track(1, || varlit_core::TryClone::try_clone(&v));
    assert!(r.is_err());
    assert_eq!((s.calls, s.live, cloned.get(), dropped.get()), (1, 0, 0, 0));
    let (r, s) = common::track(1, || j.try_clone());
    assert!(r.is_err());
    assert_eq!((s.calls, s.live, cloned.get(), dropped.get()), (1, 0, 0, 0));
    let ((), s) = common::track(usize::MAX, || {
        drop(varlit_core::TryClone::try_clone(&v).unwrap());
        drop(j.try_clone().unwrap());
    });
    assert_eq!((s.calls, s.live, cloned.get(), dropped.get()), (2, 0, 2, 2));
    drop(v);
    drop(j);
    assert_eq!(dropped.get(), 4);
}
#[test]
fn equal_success_values_and_zst_allocations() {
    for x in [0, 1, u64::MAX] {
        let a = Box::new(x);
        let b = varlit_core::try_box(x).unwrap();
        let c = JBox::try_new(x).unwrap();
        assert_eq!(*a, *b);
        assert_eq!(*a, *c);
        assert_eq!(*a.clone(), *varlit_core::TryClone::try_clone(&b).unwrap());
        assert_eq!(*a.clone(), *c.try_clone().unwrap());
    }
    let ((), s) = common::track(1, || {
        drop(Box::new(()));
        drop(varlit_core::try_box(()).unwrap());
        drop(JBox::try_new(()).unwrap());
    });
    assert_eq!((s.calls, s.live), (0, 0));
}
