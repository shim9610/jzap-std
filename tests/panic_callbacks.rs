//! User callbacks that unwind in the middle of a container operation.
//!
//! The library's failure contract covers callbacks that return. A callback that
//! panics is outside that contract, but the container must still be memory
//! safe: no element is dropped twice or read after it was moved out, and the
//! container can be inspected, reused and dropped afterwards.
//!
//! Each scenario first counts how many times the selected callback runs, then
//! repeats the operation once per invocation with a panic at that invocation.
//! After every unwind it walks the container, reuses it and drops it. The
//! element registry reports a second drop of the same element and counts the
//! elements that were never dropped. Run under Miri to also check the pointer
//! accesses on these paths.
use jzap::{
    Error, TryClone, TryIterator,
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet},
    deque::VecDeque,
    iter::from_std,
    vec::Vec,
};
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    collections::BTreeSet as Registry,
    hash::{BuildHasherDefault, Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind, panic_any},
    sync::Once,
};

const CMP: u8 = 1;
const HASH: u8 = 2;
const EQ: u8 = 4;
const CLONE: u8 = 8;
const DROP: u8 = 16;
const CALL: u8 = 32;

struct Trip;
thread_local! {
    static MASK: Cell<u8> = const { Cell::new(0) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
    static TRIP: Cell<usize> = const { Cell::new(usize::MAX) };
    static NEXT: Cell<u64> = const { Cell::new(0) };
    static LIVE: RefCell<Registry<u64>> = const { RefCell::new(Registry::new()) };
    static DOUBLE: Cell<usize> = const { Cell::new(0) };
}
/// Counts one invocation of a selected callback and unwinds at the armed one.
/// The fuse fires once, so a destructor never panics during the unwind it started.
fn tick(channel: u8) {
    if MASK.get() & channel == 0 {
        return;
    }
    COUNT.set(COUNT.get() + 1);
    if COUNT.get() == TRIP.get() {
        MASK.set(0);
        panic_any(Trip);
    }
}
fn arm(mask: u8, trip: usize) {
    COUNT.set(0);
    TRIP.set(trip);
    MASK.set(mask);
}
fn disarm() -> usize {
    MASK.set(0);
    COUNT.get()
}
fn quiet() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !info.payload().is::<Trip>() {
                previous(info)
            }
        }));
    });
}

/// An element with observable identity and callbacks on every trait the containers use.
#[derive(Debug)]
struct E {
    key: u32,
    id: u64,
}
fn e(key: u32) -> E {
    let id = NEXT.get();
    NEXT.set(id + 1);
    LIVE.with_borrow_mut(|l| l.insert(id));
    E { key, id }
}
impl Drop for E {
    fn drop(&mut self) {
        if !LIVE.with_borrow_mut(|l| l.remove(&self.id)) {
            DOUBLE.set(DOUBLE.get() + 1);
        }
        tick(DROP);
    }
}
impl E {
    /// Reads the element and reports a value that was already dropped.
    fn observe(&self) -> u32 {
        assert!(
            LIVE.with_borrow(|l| l.contains(&self.id)),
            "read of a dropped element"
        );
        self.key
    }
}
impl PartialEq for E {
    fn eq(&self, other: &Self) -> bool {
        tick(EQ);
        self.observe() == other.observe()
    }
}
impl Eq for E {}
impl PartialOrd for E {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for E {
    fn cmp(&self, other: &Self) -> Ordering {
        tick(CMP);
        self.observe().cmp(&other.observe())
    }
}
impl Hash for E {
    fn hash<H: Hasher>(&self, state: &mut H) {
        tick(HASH);
        self.observe().hash(state)
    }
}
impl TryClone for E {
    fn try_clone(&self) -> Result<Self, Error> {
        tick(CLONE);
        Ok(e(self.observe()))
    }
}

/// A fixed hasher keeps the probe sequence, and so the invocation count, repeatable.
#[derive(Default)]
struct Fnv(u64);
impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3);
        }
    }
}
type Fixed = BuildHasherDefault<Fnv>;

struct Outcome {
    invocations: usize,
    leaked: usize,
}
/// Runs `act` once per callback invocation with a panic at that invocation.
fn sweep<C>(
    mask: u8,
    setup: impl Fn() -> C,
    act: impl Fn(&mut C),
    check: impl Fn(&mut C),
) -> Outcome {
    quiet();
    let run = |trip: usize| -> (usize, bool, usize) {
        LIVE.with_borrow_mut(|l| l.clear());
        DOUBLE.set(0);
        let mut c = setup();
        arm(mask, trip);
        let unwound = catch_unwind(AssertUnwindSafe(|| act(&mut c))).is_err();
        let count = disarm();
        check(&mut c);
        drop(c);
        assert_eq!(
            DOUBLE.get(),
            0,
            "an element was dropped twice (panic at invocation {trip})"
        );
        (count, unwound, LIVE.with_borrow(|l| l.len()))
    };
    let (invocations, unwound, leaked) = run(usize::MAX);
    assert!(!unwound, "the operation panicked without an armed callback");
    assert_eq!(leaked, 0, "the uninterrupted operation leaked");
    assert!(
        invocations > 0,
        "the scenario never ran the selected callback"
    );
    let step = if cfg!(miri) {
        (invocations / 12).max(1)
    } else {
        1
    };
    let mut worst = 0;
    for trip in (1..=invocations).step_by(step).chain([invocations]) {
        let (_, unwound, leaked) = run(trip);
        assert!(unwound, "invocation {trip} of {invocations} did not unwind");
        worst = worst.max(leaked);
    }
    Outcome {
        invocations,
        leaked: worst,
    }
}
#[track_caller]
fn none(outcome: Outcome) {
    assert_eq!(
        outcome.leaked, 0,
        "elements leaked after one of {} unwinding callbacks",
        outcome.invocations
    );
}

const N: u32 = if cfg!(miri) { 24 } else { 96 };
fn keys() -> impl Iterator<Item = u32> {
    (0..N).map(|i| i.wrapping_mul(2_654_435_761) % 1009)
}
fn ok<T, X>(r: Result<T, X>) -> T {
    match r {
        Ok(v) => v,
        Err(_) => panic!("unexpected refusal"),
    }
}

// ---- Vec ------------------------------------------------------------------------------
fn vec() -> Vec<E> {
    let mut v = Vec::new();
    for k in keys() {
        ok(v.try_push(e(k)));
    }
    v
}
fn check_vec(v: &mut Vec<E>) {
    assert_eq!(v.iter().map(E::observe).count(), v.len());
    ok(v.try_push(e(7)));
    assert_eq!(v.pop().map(|x| x.observe()), Some(7));
}
#[test]
fn vec_retain_closure() {
    none(sweep(
        CALL,
        vec,
        |v| {
            let _ = v.try_retain(|x| {
                tick(CALL);
                Ok::<_, ()>(x.observe() % 3 != 0)
            });
        },
        check_vec,
    ));
    none(sweep(
        CALL,
        vec,
        |v| {
            let _ = v.try_retain_mut(|x| {
                tick(CALL);
                Ok::<_, ()>(x.observe() % 2 == 0)
            });
        },
        check_vec,
    ));
}
#[test]
fn vec_retain_destructor() {
    let outcome = sweep(
        DROP,
        vec,
        |v| {
            let _ = v.try_retain(|x| Ok::<_, ()>(x.observe() % 3 != 0));
        },
        check_vec,
    );
    none(outcome);
}
#[test]
fn vec_dedup_comparison_and_destructor() {
    let sorted = || {
        let mut v = Vec::new();
        for k in 0..N {
            ok(v.try_push(e(k / 3)));
        }
        v
    };
    none(sweep(EQ, sorted, |v| v.dedup(), check_vec));
    none(sweep(DROP, sorted, |v| v.dedup(), check_vec));
    none(sweep(
        CALL,
        sorted,
        |v| {
            v.dedup_by(|a, b| {
                tick(CALL);
                a.observe() == b.observe()
            })
        },
        check_vec,
    ));
}
#[test]
fn vec_extract_if_closure_and_destructor() {
    none(sweep(
        CALL,
        vec,
        |v| {
            let it = ok(v.try_extract_if(.., |x| {
                tick(CALL);
                Ok::<_, ()>(x.observe() % 2 == 0)
            }));
            let mut it = it;
            while let Ok(Some(x)) = it.try_next() {
                drop(x)
            }
        },
        check_vec,
    ));
    none(sweep(
        DROP,
        vec,
        |v| {
            let it = ok(v.try_extract_if(.., |x| Ok::<_, ()>(x.observe() % 2 == 0)));
            let mut it = it;
            while let Ok(Some(x)) = it.try_next() {
                drop(x)
            }
        },
        check_vec,
    ));
}
#[test]
fn vec_resize_extend_and_clone_callbacks() {
    none(sweep(
        CALL,
        vec,
        |v| {
            let _ = v.try_resize_with(N as usize * 2, || {
                tick(CALL);
                Ok::<_, ()>(e(1))
            });
        },
        check_vec,
    ));
    none(sweep(
        CALL,
        vec,
        |v| {
            let _ = v.try_extend((0..N).map(|k| {
                tick(CALL);
                e(k)
            }));
        },
        check_vec,
    ));
    none(sweep(
        CLONE,
        vec,
        |v| {
            if let Ok(mut c) = v.try_clone() {
                check_vec(&mut c)
            }
        },
        check_vec,
    ));
    none(sweep(
        CLONE,
        vec,
        |v| {
            let src = vec();
            let _ = v.try_extend_cloned(src.as_slice());
        },
        check_vec,
    ));
}
#[test]
fn vec_destructors_in_truncate_clear_drain_and_drop() {
    none(sweep(DROP, vec, |v| v.truncate(3), check_vec));
    none(sweep(DROP, vec, |v| v.clear(), check_vec));
    none(sweep(
        DROP,
        vec,
        |v| {
            drop(ok(v.try_drain(2..N as usize - 2)));
        },
        check_vec,
    ));
    none(sweep(
        DROP,
        vec,
        |v| {
            let mut d = ok(v.try_drain(2..N as usize - 2));
            drop(d.next());
        },
        check_vec,
    ));
    none(sweep(DROP, || Some(vec()), |v| drop(v.take()), |_| ()));
    none(sweep(
        DROP,
        || Some(vec().into_iter()),
        |v| {
            let mut it = v.take().unwrap();
            drop(it.next());
            drop(it)
        },
        |_| (),
    ));
}

// ---- VecDeque -------------------------------------------------------------------------
fn deque() -> VecDeque<E> {
    let mut v = VecDeque::new();
    for (i, k) in keys().enumerate() {
        // Alternate ends so the ring is wrapped.
        if i % 3 == 0 {
            ok(v.try_push_front(e(k)))
        } else {
            ok(v.try_push_back(e(k)))
        }
    }
    v
}
fn check_deque(v: &mut VecDeque<E>) {
    assert_eq!(v.iter().map(E::observe).count(), v.len());
    ok(v.try_push_front(e(7)));
    assert_eq!(v.pop_front().map(|x| x.observe()), Some(7));
    ok(v.try_push_back(e(8)));
    assert_eq!(v.pop_back().map(|x| x.observe()), Some(8));
}
#[test]
fn deque_retain_extend_clone_and_search_callbacks() {
    none(sweep(
        CALL,
        deque,
        |v| {
            let _ = v.try_retain(|x| {
                tick(CALL);
                Ok::<_, ()>(x.observe() % 3 != 0)
            });
        },
        check_deque,
    ));
    none(sweep(
        DROP,
        deque,
        |v| {
            let _ = v.try_retain(|x| Ok::<_, ()>(x.observe() % 3 != 0));
        },
        check_deque,
    ));
    none(sweep(
        CALL,
        deque,
        |v| {
            let _ = v.try_extend(from_std((0..N).map(|k| {
                tick(CALL);
                e(k)
            })));
        },
        check_deque,
    ));
    none(sweep(
        CALL,
        deque,
        |v| {
            let _ = v.try_resize_with(N as usize * 2, || {
                tick(CALL);
                Ok::<_, ()>(e(1))
            });
        },
        check_deque,
    ));
    none(sweep(
        CLONE,
        deque,
        |v| {
            if let Ok(mut c) = v.try_clone() {
                check_deque(&mut c)
            }
        },
        check_deque,
    ));
    none(sweep(
        EQ,
        deque,
        |v| {
            let probe = e(2000);
            assert!(!v.contains(&probe));
        },
        check_deque,
    ));
    none(sweep(
        CMP,
        deque,
        |v| {
            let probe = e(500);
            let _ = v.binary_search(&probe);
        },
        check_deque,
    ));
}
#[test]
fn deque_destructors_in_truncate_clear_drain_and_drop() {
    none(sweep(DROP, deque, |v| v.truncate(3), check_deque));
    none(sweep(DROP, deque, |v| v.clear(), check_deque));
    none(sweep(
        DROP,
        deque,
        |v| {
            drop(ok(v.drain(2..N as usize - 2)));
        },
        check_deque,
    ));
    none(sweep(
        DROP,
        deque,
        |v| {
            let mut d = ok(v.drain(2..N as usize - 2));
            drop(d.next());
            drop(d.next_back());
        },
        check_deque,
    ));
    none(sweep(DROP, || Some(deque()), |v| drop(v.take()), |_| ()));
    none(sweep(
        DROP,
        || Some(deque().into_iter()),
        |v| {
            let mut it = v.take().unwrap();
            drop(it.next());
            drop(it)
        },
        |_| (),
    ));
}

// ---- BinaryHeap -----------------------------------------------------------------------
fn heap() -> BinaryHeap<E> {
    let mut h = BinaryHeap::new();
    for k in keys() {
        ok(h.try_push(e(k)));
    }
    h
}
fn check_heap(h: &mut BinaryHeap<E>) {
    assert_eq!(h.iter().map(E::observe).count(), h.len());
    ok(h.try_push(e(7)));
    let before = h.len();
    drop(h.pop());
    assert_eq!(h.len(), before - 1);
}
#[test]
fn heap_comparison_in_every_sift() {
    none(sweep(
        CMP,
        heap,
        |h| {
            for k in 0..8 {
                ok(h.try_push(e(2000 - k)))
            }
        },
        check_heap,
    ));
    none(sweep(
        CMP,
        heap,
        |h| {
            for _ in 0..8 {
                drop(h.pop())
            }
        },
        check_heap,
    ));
    none(sweep(
        CMP,
        heap,
        |h| {
            for k in 0..8 {
                if let Some(mut top) = h.peek_mut() {
                    top.key = k
                }
            }
        },
        check_heap,
    ));
    none(sweep(
        CMP,
        heap,
        |h| {
            let mut other = heap();
            let _ = h.try_append(&mut other);
        },
        check_heap,
    ));
    none(sweep(
        CMP,
        || Some(heap()),
        |h| {
            let v = h.take().unwrap().into_sorted_vec();
            drop(v)
        },
        |_| (),
    ));
    none(sweep(
        CMP,
        || Some(vec()),
        |v| {
            let mut h = BinaryHeap::from_vec(v.take().unwrap());
            check_heap(&mut h)
        },
        |_| (),
    ));
    none(sweep(
        CMP,
        heap,
        |h| {
            let _ = h.try_extend(from_std((0..N).map(e)));
        },
        check_heap,
    ));
}
#[test]
fn heap_retain_and_destructors() {
    none(sweep(
        CALL,
        heap,
        |h| {
            let _ = h.try_retain(|x| {
                tick(CALL);
                Ok::<_, ()>(x.observe() % 3 != 0)
            });
        },
        check_heap,
    ));
    none(sweep(
        CMP,
        heap,
        |h| {
            let _ = h.try_retain(|x| Ok::<_, ()>(x.observe() % 3 != 0));
        },
        check_heap,
    ));
    none(sweep(
        DROP,
        heap,
        |h| {
            let _ = h.try_retain(|x| Ok::<_, ()>(x.observe() % 3 != 0));
        },
        check_heap,
    ));
    none(sweep(DROP, heap, |h| h.clear(), check_heap));
    none(sweep(
        DROP,
        heap,
        |h| {
            let mut d = h.drain();
            drop(d.next());
        },
        check_heap,
    ));
    none(sweep(
        CLONE,
        heap,
        |h| {
            if let Ok(mut c) = h.try_clone() {
                check_heap(&mut c)
            }
        },
        check_heap,
    ));
}

// ---- BTreeMap and BTreeSet ------------------------------------------------------------
fn btree() -> BTreeMap<E, E> {
    let mut m = BTreeMap::new();
    for k in keys() {
        drop(ok(m.try_insert(e(k), e(k + 1))));
    }
    m
}
fn check_btree(m: &mut BTreeMap<E, E>) {
    assert_eq!(
        m.iter().map(|(k, v)| k.observe() + v.observe()).count(),
        m.len()
    );
    assert_eq!(m.iter_mut().rev().count(), m.len());
    drop(ok(m.try_insert(e(5000), e(1))));
    assert!(m.remove(&e(5000)).is_some());
    drop(m.pop_first());
    drop(m.pop_last());
}
#[test]
fn btree_comparison_in_search_insert_and_remove() {
    none(sweep(
        CMP,
        btree,
        |m| {
            for k in 0..12 {
                drop(ok(m.try_insert(e(3000 + k * 7 % 5), e(k))))
            }
        },
        check_btree,
    ));
    none(sweep(
        CMP,
        btree,
        |m| {
            for k in keys().take(12) {
                drop(m.remove(&e(k)))
            }
        },
        check_btree,
    ));
    none(sweep(
        CMP,
        btree,
        |m| {
            for k in keys().take(12) {
                let probe = e(k);
                assert!(m.get(&probe).is_some());
                assert!(m.contains_key(&probe));
            }
        },
        check_btree,
    ));
    none(sweep(
        CMP,
        btree,
        |m| {
            for k in 0..12 {
                let entry = m.entry(e(k * 31));
                drop(entry)
            }
        },
        check_btree,
    ));
    none(sweep(
        CMP,
        btree,
        |m| {
            let (a, b) = (e(100), e(700));
            if let Ok(r) = m.try_range(&a..&b) {
                assert!(r.count() <= m.len())
            }
        },
        check_btree,
    ));
    none(sweep(
        CMP,
        btree,
        |m| {
            let at = e(400);
            if let Ok(mut tail) = m.try_split_off(&at) {
                check_btree(&mut tail)
            }
        },
        check_btree,
    ));
    none(sweep(
        CMP,
        btree,
        |m| {
            let _ = m.try_extend(from_std((0..N).map(|k| (e(k * 13), e(k)))));
        },
        check_btree,
    ));
}
#[test]
fn btree_closures_clone_and_destructors() {
    none(sweep(
        CALL,
        btree,
        |m| {
            let _ = m.try_retain(|k, _| {
                tick(CALL);
                Ok::<_, ()>(k.observe() % 3 != 0)
            });
        },
        check_btree,
    ));
    none(sweep(
        DROP,
        btree,
        |m| {
            let _ = m.try_retain(|k, _| Ok::<_, ()>(k.observe() % 3 != 0));
        },
        check_btree,
    ));
    none(sweep(
        CALL,
        btree,
        |m| {
            let it = ok(m.try_extract_if(.., |k, _| {
                tick(CALL);
                Ok::<_, ()>(k.observe() % 2 == 0)
            }));
            let mut it = it;
            while let Ok(Some(x)) = it.try_next() {
                drop(x)
            }
        },
        check_btree,
    ));
    none(sweep(
        CLONE,
        btree,
        |m| {
            if let Ok(mut c) = m.try_clone() {
                check_btree(&mut c)
            }
        },
        check_btree,
    ));
    none(sweep(DROP, btree, |m| m.clear(), check_btree));
    none(sweep(DROP, || Some(btree()), |m| drop(m.take()), |_| ()));
    none(sweep(
        DROP,
        || Some(btree().into_iter()),
        |m| {
            let mut it = m.take().unwrap();
            drop(it.next());
            drop(it.next_back());
            drop(it)
        },
        |_| (),
    ));
}
fn bset() -> BTreeSet<E> {
    let mut s = BTreeSet::new();
    for k in keys() {
        ok(s.try_insert(e(k)));
    }
    s
}
fn check_bset(s: &mut BTreeSet<E>) {
    assert_eq!(s.iter().map(E::observe).count(), s.len());
    ok(s.try_insert(e(5000)));
    assert!(s.remove(&e(5000)));
}
#[test]
fn btree_set_comparison_in_set_algebra() {
    let other = || {
        let mut s = BTreeSet::new();
        for k in keys().step_by(2).chain(2000..2010) {
            ok(s.try_insert(e(k)));
        }
        s
    };
    none(sweep(
        CMP,
        bset,
        |s| {
            let o = other();
            assert!(s.intersection(&o).count() <= s.len());
        },
        check_bset,
    ));
    none(sweep(
        CMP,
        bset,
        |s| {
            let o = other();
            assert!(s.difference(&o).count() <= s.len());
        },
        check_bset,
    ));
    none(sweep(
        CMP,
        bset,
        |s| {
            let o = other();
            assert!(s.union(&o).count() >= s.len());
        },
        check_bset,
    ));
    none(sweep(
        CMP,
        bset,
        |s| {
            let o = other();
            assert!(s.symmetric_difference(&o).count() > 0);
        },
        check_bset,
    ));
    none(sweep(
        CMP,
        bset,
        |s| {
            let o = other();
            let _ = (s.is_subset(&o), s.is_superset(&o), s.is_disjoint(&o));
        },
        check_bset,
    ));
    none(sweep(
        CMP,
        bset,
        |s| {
            let mut o = other();
            let _ = s.try_append(&mut o);
            check_bset(&mut o)
        },
        check_bset,
    ));
    none(sweep(
        CMP,
        bset,
        |s| {
            for k in keys().take(8) {
                drop(ok(s.try_replace(e(k))));
                drop(s.take(&e(k + 1)))
            }
        },
        check_bset,
    ));
}

// ---- HashMap and HashSet --------------------------------------------------------------
fn hmap() -> HashMap<E, E, Fixed> {
    let mut m = HashMap::with_hasher(Fixed::default());
    for k in keys() {
        drop(ok(m.try_insert(e(k), e(k + 1))));
    }
    m
}
fn check_hmap(m: &mut HashMap<E, E, Fixed>) {
    assert_eq!(
        m.iter().map(|(k, v)| k.observe() + v.observe()).count(),
        m.len()
    );
    assert_eq!(m.values_mut().count(), m.len());
    drop(ok(m.try_insert(e(5000), e(1))));
    assert!(m.remove(&e(5000)).is_some());
}
#[test]
fn hash_map_hashing_and_equality_in_lookup_insert_and_growth() {
    // Inserting many keys forces resizes, so the hash callback also runs while entries move.
    let grow = |m: &mut HashMap<E, E, Fixed>| {
        for k in 0..N * 4 {
            drop(ok(m.try_insert(e(3000 + k), e(k))))
        }
    };
    none(sweep(HASH, hmap, grow, check_hmap));
    none(sweep(
        EQ,
        hmap,
        |m| {
            for k in keys() {
                drop(ok(m.try_insert(e(k), e(0))))
            }
        },
        check_hmap,
    ));
    none(sweep(
        HASH | EQ,
        hmap,
        |m| {
            for k in keys().take(16) {
                drop(m.remove(&e(k)))
            }
        },
        check_hmap,
    ));
    none(sweep(
        HASH | EQ,
        hmap,
        |m| {
            for k in keys().take(16) {
                let probe = e(k);
                assert!(m.get(&probe).is_some())
            }
        },
        check_hmap,
    ));
    none(sweep(
        HASH | EQ,
        hmap,
        |m| {
            for k in 0..16 {
                let entry = m.entry(e(k * 31));
                drop(entry)
            }
        },
        check_hmap,
    ));
    none(sweep(
        HASH,
        hmap,
        |m| {
            let _ = m.try_reserve(N as usize * 8);
        },
        check_hmap,
    ));
    none(sweep(
        HASH,
        hmap,
        |m| {
            for k in keys().skip(8) {
                drop(m.remove(&e(k)))
            }
            let _ = m.try_shrink_to_fit();
        },
        check_hmap,
    ));
    none(sweep(
        HASH | EQ,
        hmap,
        |m| {
            let _ = m.try_extend(from_std((0..N).map(|k| (e(k * 13), e(k)))));
        },
        check_hmap,
    ));
}
#[test]
fn hash_map_hashing_during_in_place_rehash() {
    // Deletion churn at a small length leaves tombstones until no growth allowance remains.
    // The next insertion then rehashes in place: every kept pair is rehashed and moved inside
    // the same allocation while the hash callback may unwind.
    let full = || {
        // The table size is fixed, also under Miri: tombstones only appear once whole
        // probe groups are occupied, and a smaller table never uses up its allowance.
        let mut m = HashMap::with_hasher(Fixed::default());
        for k in 0..56 {
            drop(ok(m.try_insert(e(k * 17), e(k))));
        }
        for k in 4..56 {
            drop(m.remove(&e(k * 17)))
        }
        let mut k = 10_000;
        // `capacity()` is the length plus the remaining growth allowance.
        while m.capacity() != m.len() {
            drop(ok(m.try_insert(e(k), e(k))));
            if m.capacity() != m.len() {
                drop(m.remove(&e(k)))
            }
            k += 1;
            assert!(k < 20_000, "churn never used up the growth allowance");
        }
        m
    };
    let churn = |m: &mut HashMap<E, E, Fixed>| {
        // An insertion that lands on a tombstone needs no allowance, so keep inserting new
        // keys: the first one that needs an empty slot starts the rehash. The unsafe-block
        // coverage listing is the check that these sweeps reach `RawTable::rehash`.
        for k in 0..48 {
            drop(ok(m.try_insert(e(9000 + k), e(k))));
            drop(m.remove(&e(9000 + k)))
        }
    };
    none(sweep(HASH, full, churn, check_hmap));
    none(sweep(EQ, full, churn, check_hmap));
}
#[test]
fn hash_map_closures_clone_and_destructors() {
    none(sweep(
        CALL,
        hmap,
        |m| {
            let _ = m.try_retain(|k, _| {
                tick(CALL);
                Ok::<_, ()>(k.observe() % 3 != 0)
            });
        },
        check_hmap,
    ));
    none(sweep(
        DROP,
        hmap,
        |m| {
            let _ = m.try_retain(|k, _| Ok::<_, ()>(k.observe() % 3 != 0));
        },
        check_hmap,
    ));
    none(sweep(
        CALL,
        hmap,
        |m| {
            let it = m.try_extract_if(|k, _| {
                tick(CALL);
                Ok::<_, ()>(k.observe() % 2 == 0)
            });
            let mut it = it;
            while let Ok(Some(x)) = it.try_next() {
                drop(x)
            }
        },
        check_hmap,
    ));
    none(sweep(
        DROP,
        hmap,
        |m| {
            let it = m.try_extract_if(|k, _| Ok::<_, ()>(k.observe() % 2 == 0));
            let mut it = it;
            while let Ok(Some(x)) = it.try_next() {
                drop(x)
            }
        },
        check_hmap,
    ));
    none(sweep(
        CLONE,
        hmap,
        |m| {
            if let Ok(mut c) = m.try_clone() {
                check_hmap(&mut c)
            }
        },
        check_hmap,
    ));
    none(sweep(DROP, hmap, |m| m.clear(), check_hmap));
    none(sweep(
        DROP,
        hmap,
        |m| {
            let mut d = m.drain();
            drop(d.next());
        },
        check_hmap,
    ));
    none(sweep(DROP, || Some(hmap()), |m| drop(m.take()), |_| ()));
    none(sweep(
        DROP,
        || Some(hmap().into_iter()),
        |m| {
            let mut it = m.take().unwrap();
            drop(it.next());
            drop(it)
        },
        |_| (),
    ));
}
fn hset() -> HashSet<E, Fixed> {
    let mut s = HashSet::with_hasher(Fixed::default());
    for k in keys() {
        ok(s.try_insert(e(k)));
    }
    s
}
fn check_hset(s: &mut HashSet<E, Fixed>) {
    assert_eq!(s.iter().map(E::observe).count(), s.len());
    ok(s.try_insert(e(5000)));
    assert!(s.remove(&e(5000)));
}
#[test]
fn hash_set_hashing_and_equality_in_set_algebra() {
    let other = || {
        let mut s = HashSet::with_hasher(Fixed::default());
        for k in keys().step_by(2).chain(2000..2010) {
            ok(s.try_insert(e(k)));
        }
        s
    };
    none(sweep(
        HASH | EQ,
        hset,
        |s| {
            let o = other();
            assert!(s.intersection(&o).count() <= s.len());
        },
        check_hset,
    ));
    none(sweep(
        HASH | EQ,
        hset,
        |s| {
            let o = other();
            assert!(s.difference(&o).count() <= s.len());
        },
        check_hset,
    ));
    none(sweep(
        HASH | EQ,
        hset,
        |s| {
            let o = other();
            assert!(s.union(&o).count() >= s.len());
        },
        check_hset,
    ));
    none(sweep(
        HASH | EQ,
        hset,
        |s| {
            let o = other();
            assert!(s.symmetric_difference(&o).count() > 0);
        },
        check_hset,
    ));
    none(sweep(
        HASH | EQ,
        hset,
        |s| {
            let o = other();
            let _ = (s.is_subset(&o), s.is_superset(&o), s.is_disjoint(&o));
        },
        check_hset,
    ));
    none(sweep(
        HASH | EQ,
        hset,
        |s| {
            for k in keys().take(8) {
                drop(ok(s.try_replace(e(k))));
                drop(s.take(&e(k + 1)))
            }
        },
        check_hset,
    ));
    none(sweep(
        CALL,
        hset,
        |s| {
            let _ = s.try_retain(|x| {
                tick(CALL);
                Ok::<_, ()>(x.observe() % 3 != 0)
            });
        },
        check_hset,
    ));
}

// ---- Box, Rc and Arc ------------------------------------------------------------------
#[test]
fn shared_and_boxed_initializers_clones_and_destructors() {
    use jzap::{boxed::Box, rc::Rc, sync::Arc};
    none(sweep(
        DROP,
        || Some(ok(Box::try_new(e(1)))),
        |b| drop(b.take()),
        |_| (),
    ));
    none(sweep(
        DROP,
        || Some(ok(Box::<[E]>::try_from_iter((0..8).map(e)))),
        |b| drop(b.take()),
        |_| (),
    ));
    none(sweep(
        CALL,
        || (),
        |_| {
            let _ = Box::<[E]>::try_from_iter((0..8).map(|k| {
                tick(CALL);
                e(k)
            }));
        },
        |_| (),
    ));
    none(sweep(
        CLONE,
        || ok(Box::<[E]>::try_from_iter((0..8).map(e))),
        |b| {
            let _ = Box::<[E]>::try_clone_from_slice(b.as_slice());
        },
        |_| (),
    ));
    // The last strong handle runs the destructor while a weak handle keeps the allocation.
    none(sweep(
        DROP,
        || {
            let a = ok(Rc::try_new(e(1)));
            let w = ok(Rc::try_downgrade(&a));
            (Some(a), w)
        },
        |p| drop(p.0.take()),
        |p| assert!(matches!(p.1.try_upgrade(), Ok(None))),
    ));
    none(sweep(
        DROP,
        || {
            let a = ok(Arc::try_new(e(1)));
            let w = ok(Arc::try_downgrade(&a));
            (Some(a), w)
        },
        |p| drop(p.0.take()),
        |p| assert!(matches!(p.1.try_upgrade(), Ok(None))),
    ));
    none(sweep(
        DROP,
        || Some(ok(Arc::try_new(e(1)))),
        |a| drop(a.take()),
        |_| (),
    ));
    none(sweep(
        CLONE,
        || {
            let a = ok(Rc::try_new(e(1)));
            let b = ok(Rc::try_clone(&a));
            (Some(a), b)
        },
        |p| {
            if let Ok(x) = Rc::try_unwrap_or_clone(p.0.take().unwrap()) {
                drop(x)
            }
        },
        |p| assert_eq!(p.1.observe(), 1),
    ));
}

// ---- A discarded half of a removed pair ------------------------------------------------
/// Operations that return one half of a stored pair and drop the other half. When the
/// dropped half unwinds, the returned half must still be dropped: it has to be owned by a
/// local at that point and not already moved into the return slot.
#[test]
fn destructor_of_the_discarded_half_does_not_leak_the_returned_half() {
    use jzap::collections::{btree_map, hash_map};
    let mut leaks: std::vec::Vec<(&str, usize)> = std::vec::Vec::new();
    let mut record = |name: &'static str, outcome: Outcome| {
        if outcome.leaked != 0 {
            leaks.push((name, outcome.leaked))
        }
    };
    record(
        "BTreeMap::remove",
        sweep(
            DROP,
            btree,
            |m| {
                for k in keys().take(12) {
                    drop(m.remove(&e(k)))
                }
            },
            check_btree,
        ),
    );
    record(
        "BTreeMap::remove_entry",
        sweep(
            DROP,
            btree,
            |m| {
                for k in keys().take(12) {
                    drop(m.remove_entry(&e(k)))
                }
            },
            check_btree,
        ),
    );
    record(
        "BTreeMap::pop_first",
        sweep(
            DROP,
            btree,
            |m| {
                for _ in 0..12 {
                    drop(m.pop_first());
                    drop(m.pop_last())
                }
            },
            check_btree,
        ),
    );
    record(
        "BTreeMap::try_insert existing",
        sweep(
            DROP,
            btree,
            |m| {
                for k in keys().take(12) {
                    drop(ok(m.try_insert(e(k), e(0))))
                }
            },
            check_btree,
        ),
    );
    record(
        "btree_map::OccupiedEntry::remove",
        sweep(
            DROP,
            btree,
            |m| {
                for k in keys().take(12) {
                    if let btree_map::Entry::Occupied(entry) = m.entry(e(k)) {
                        drop(entry.remove())
                    }
                }
            },
            check_btree,
        ),
    );
    record(
        "btree_map::OccupiedEntry::insert",
        sweep(
            DROP,
            btree,
            |m| {
                for k in keys().take(12) {
                    if let btree_map::Entry::Occupied(mut entry) = m.entry(e(k)) {
                        drop(entry.insert(e(0)))
                    }
                }
            },
            check_btree,
        ),
    );
    record(
        "btree_map::IntoKeys",
        sweep(
            DROP,
            || Some(btree().into_keys()),
            |it| {
                for k in it.take().unwrap() {
                    drop(k)
                }
            },
            |_| (),
        ),
    );
    record(
        "btree_map::IntoValues",
        sweep(
            DROP,
            || Some(btree().into_values()),
            |it| {
                for v in it.take().unwrap() {
                    drop(v)
                }
            },
            |_| (),
        ),
    );
    record(
        "btree_map::IntoKeys back",
        sweep(
            DROP,
            || Some(btree().into_keys()),
            |it| {
                let mut it = it.take().unwrap();
                while let Some(k) = it.next_back() {
                    drop(k)
                }
            },
            |_| (),
        ),
    );
    record(
        "btree_map::IntoValues back",
        sweep(
            DROP,
            || Some(btree().into_values()),
            |it| {
                let mut it = it.take().unwrap();
                while let Some(v) = it.next_back() {
                    drop(v)
                }
            },
            |_| (),
        ),
    );
    record(
        "BTreeSet::remove",
        sweep(
            DROP,
            bset,
            |s| {
                for k in keys().take(12) {
                    assert!(s.remove(&e(k)))
                }
            },
            check_bset,
        ),
    );
    record(
        "BTreeSet::take",
        sweep(
            DROP,
            bset,
            |s| {
                for k in keys().take(12) {
                    drop(s.take(&e(k)))
                }
            },
            check_bset,
        ),
    );
    record(
        "BTreeSet::try_replace",
        sweep(
            DROP,
            bset,
            |s| {
                for k in keys().take(12) {
                    drop(ok(s.try_replace(e(k))))
                }
            },
            check_bset,
        ),
    );
    record(
        "BTreeSet::try_insert existing",
        sweep(
            DROP,
            bset,
            |s| {
                for k in keys().take(12) {
                    let _ = ok(s.try_insert(e(k)));
                }
            },
            check_bset,
        ),
    );
    record(
        "BTreeSet::pop_first",
        sweep(
            DROP,
            bset,
            |s| {
                for _ in 0..12 {
                    drop(s.pop_first());
                    drop(s.pop_last())
                }
            },
            check_bset,
        ),
    );
    record(
        "HashMap::remove",
        sweep(
            DROP,
            hmap,
            |m| {
                for k in keys().take(12) {
                    drop(m.remove(&e(k)))
                }
            },
            check_hmap,
        ),
    );
    record(
        "HashMap::remove_entry",
        sweep(
            DROP,
            hmap,
            |m| {
                for k in keys().take(12) {
                    drop(m.remove_entry(&e(k)))
                }
            },
            check_hmap,
        ),
    );
    record(
        "HashMap::try_insert existing",
        sweep(
            DROP,
            hmap,
            |m| {
                for k in keys().take(12) {
                    drop(ok(m.try_insert(e(k), e(0))))
                }
            },
            check_hmap,
        ),
    );
    record(
        "hash_map::OccupiedEntry::remove",
        sweep(
            DROP,
            hmap,
            |m| {
                for k in keys().take(12) {
                    if let hash_map::Entry::Occupied(entry) = m.entry(e(k)) {
                        drop(entry.remove())
                    }
                }
            },
            check_hmap,
        ),
    );
    record(
        "hash_map::OccupiedEntry::insert",
        sweep(
            DROP,
            hmap,
            |m| {
                for k in keys().take(12) {
                    if let hash_map::Entry::Occupied(mut entry) = m.entry(e(k)) {
                        drop(entry.insert(e(0)))
                    }
                }
            },
            check_hmap,
        ),
    );
    record(
        "hash_map::IntoKeys",
        sweep(
            DROP,
            || Some(hmap().into_keys()),
            |it| {
                for k in it.take().unwrap() {
                    drop(k)
                }
            },
            |_| (),
        ),
    );
    record(
        "hash_map::IntoValues",
        sweep(
            DROP,
            || Some(hmap().into_values()),
            |it| {
                for v in it.take().unwrap() {
                    drop(v)
                }
            },
            |_| (),
        ),
    );
    record(
        "hash_map::IntoKeys fold",
        sweep(
            DROP,
            || Some(hmap().into_keys()),
            |it| it.take().unwrap().fold((), |(), k| drop(k)),
            |_| (),
        ),
    );
    record(
        "hash_map::IntoValues fold",
        sweep(
            DROP,
            || Some(hmap().into_values()),
            |it| it.take().unwrap().fold((), |(), v| drop(v)),
            |_| (),
        ),
    );
    record(
        "HashSet::remove",
        sweep(
            DROP,
            hset,
            |s| {
                for k in keys().take(12) {
                    assert!(s.remove(&e(k)))
                }
            },
            check_hset,
        ),
    );
    record(
        "HashSet::take",
        sweep(
            DROP,
            hset,
            |s| {
                for k in keys().take(12) {
                    drop(s.take(&e(k)))
                }
            },
            check_hset,
        ),
    );
    record(
        "HashSet::try_replace",
        sweep(
            DROP,
            hset,
            |s| {
                for k in keys().take(12) {
                    drop(ok(s.try_replace(e(k))))
                }
            },
            check_hset,
        ),
    );
    record(
        "HashSet::try_insert existing",
        sweep(
            DROP,
            hset,
            |s| {
                for k in keys().take(12) {
                    let _ = ok(s.try_insert(e(k)));
                }
            },
            check_hset,
        ),
    );
    record(
        "BinaryHeap PeekMut::pop",
        sweep(
            DROP,
            heap,
            |h| {
                for _ in 0..8 {
                    if let Some(top) = h.peek_mut() {
                        drop(jzap::collections::binary_heap::PeekMut::pop(top))
                    }
                }
            },
            check_heap,
        ),
    );
    assert!(leaks.is_empty(), "returned halves leaked: {leaks:?}");
}

// ---- Control: the registry reports a defective owner ------------------------------------
/// An owner that forgets it already moved an element out when the callback unwinds.
struct Careless(std::vec::Vec<std::mem::ManuallyDrop<E>>);
impl Drop for Careless {
    fn drop(&mut self) {
        for slot in &mut self.0 {
            // SAFETY: deliberately wrong after an interrupted `take_first`; see the test.
            unsafe { std::mem::ManuallyDrop::drop(slot) }
        }
    }
}
impl Careless {
    fn take_first(&mut self) {
        // SAFETY: the slot is initialized; the copy is dropped here and the slot is
        // removed below. An unwind between the two leaves a slot that was already dropped.
        let moved = unsafe { std::ptr::read(&*self.0[0]) };
        tick(CALL);
        drop(moved);
        self.0.remove(0);
    }
}
#[test]
#[should_panic(expected = "an element was dropped twice")]
fn registry_detects_a_double_drop_after_an_unwinding_callback() {
    let outcome = sweep(
        CALL,
        || Careless((0..3).map(|k| std::mem::ManuallyDrop::new(e(k))).collect()),
        |c| c.take_first(),
        |_| (),
    );
    none(outcome);
}
