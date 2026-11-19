//! Auto-trait and variance parity with the pinned standard library.
//!
//! Every row asks the compiler whether a jzap type is `Send` and `Sync` for an
//! element that is both, only `Send`, only `Sync` or neither, and compares the
//! answer with the standard counterpart. A wider implementation than std on a
//! type holding raw pointers is the usual shape of an unsound `unsafe impl`.
#![allow(dead_code)]
use std::{cell::Cell, marker::PhantomData, rc::Rc};

struct Probe<T: ?Sized>(PhantomData<T>);
trait Absent {
    const SEND: bool = false;
    const SYNC: bool = false;
}
impl<T: ?Sized> Absent for Probe<T> {}
struct SendProbe<T: ?Sized>(PhantomData<T>);
struct SyncProbe<T: ?Sized>(PhantomData<T>);
impl<T: ?Sized> Absent for SendProbe<T> {}
impl<T: ?Sized> Absent for SyncProbe<T> {}
impl<T: ?Sized + Send> SendProbe<T> {
    const SEND: bool = true;
}
impl<T: ?Sized + Sync> SyncProbe<T> {
    const SYNC: bool = true;
}
macro_rules! facts {
    ($t:ty) => {
        (<SendProbe<$t>>::SEND, <SyncProbe<$t>>::SYNC)
    };
}

macro_rules! element {
    ($name:ident, $field:ty) => {
        #[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
        struct $name($field);
    };
}
element!(Both, u8);
element!(Neither, Rc<u8>);
#[derive(PartialEq, Eq)]
struct SendOnly(Cell<u8>);
impl PartialOrd for SendOnly {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for SendOnly {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.get().cmp(&o.0.get())
    }
}
impl std::hash::Hash for SendOnly {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.0.get().hash(h)
    }
}
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SyncOnly(u8, PhantomData<Rc<u8>>);
// SAFETY: the marker holds no data; the type only models a Sync, non-Send value.
unsafe impl Sync for SyncOnly {}

#[test]
fn probes_distinguish_the_four_elements() {
    assert_eq!(facts!(Both), (true, true));
    assert_eq!(facts!(SendOnly), (true, false));
    assert_eq!(facts!(SyncOnly), (false, true));
    assert_eq!(facts!(Neither), (false, false));
}

/// Compares one jzap type constructor with its std counterpart for all four elements.
macro_rules! parity {
    ($failures:ident; $( $label:literal : [$($g:ident),*] $ours:ty => $std:ty ;)*) => {$(
        {
            macro_rules! one { ($e:ty) => {{
                $( #[allow(non_camel_case_types)] type $g = $e; )*
                let (ours, std) = (facts!($ours), facts!($std));
                if ours != std {
                    $failures.push(format!("{} with {}: jzap (Send, Sync) = {:?}, std = {:?}",
                        $label, stringify!($e), ours, std));
                }
            }}}
            one!(Both); one!(SendOnly); one!(SyncOnly); one!(Neither);
        }
    )*};
}

type Fn1 = fn(&mut Both) -> bool;

#[test]
fn send_and_sync_match_the_standard_counterparts() {
    use jzap::collections::{binary_heap, btree_map, btree_set, hash_map, hash_set};
    use std::collections as sc;
    let mut f: Vec<String> = Vec::new();
    parity!(f;
        "Box": [T] jzap::boxed::Box<T> => Box<T>;
        "Box<[T]>": [T] jzap::boxed::Box<[T]> => Box<[T]>;
        "boxed::IntoIter": [T] jzap::boxed::IntoIter<T> => std::vec::IntoIter<T>;
        "Vec": [T] jzap::vec::Vec<T> => Vec<T>;
        "vec::IntoIter": [T] jzap::vec::IntoIter<T> => std::vec::IntoIter<T>;
        "vec::Drain": [T] jzap::vec::Drain<'static, T> => std::vec::Drain<'static, T>;
        "vec::ExtractIf": [T] jzap::vec::ExtractIf<'static, T, fn(&mut T) -> bool> => std::vec::ExtractIf<'static, T, fn(&mut T) -> bool>;
        "String drain": [T] (jzap::string::Drain<'static>, T) => (std::string::Drain<'static>, T);
        "VecDeque": [T] jzap::deque::VecDeque<T> => sc::VecDeque<T>;
        "deque::Iter": [T] jzap::deque::Iter<'static, T> => sc::vec_deque::Iter<'static, T>;
        "deque::IterMut": [T] jzap::deque::IterMut<'static, T> => sc::vec_deque::IterMut<'static, T>;
        "deque::IntoIter": [T] jzap::deque::IntoIter<T> => sc::vec_deque::IntoIter<T>;
        "deque::Drain": [T] jzap::deque::Drain<'static, T> => sc::vec_deque::Drain<'static, T>;
        "BinaryHeap": [T] jzap::collections::BinaryHeap<T> => sc::BinaryHeap<T>;
        "binary_heap::PeekMut": [T] binary_heap::PeekMut<'static, T> => sc::binary_heap::PeekMut<'static, T>;
        "binary_heap::Drain": [T] binary_heap::Drain<'static, T> => sc::binary_heap::Drain<'static, T>;
        "BTreeMap key": [T] jzap::collections::BTreeMap<T, Both> => sc::BTreeMap<T, Both>;
        "BTreeMap value": [T] jzap::collections::BTreeMap<Both, T> => sc::BTreeMap<Both, T>;
        "btree_map::Iter key": [T] btree_map::Iter<'static, T, Both> => sc::btree_map::Iter<'static, T, Both>;
        "btree_map::Iter value": [T] btree_map::Iter<'static, Both, T> => sc::btree_map::Iter<'static, Both, T>;
        "btree_map::IterMut key": [T] btree_map::IterMut<'static, T, Both> => sc::btree_map::IterMut<'static, T, Both>;
        "btree_map::IterMut value": [T] btree_map::IterMut<'static, Both, T> => sc::btree_map::IterMut<'static, Both, T>;
        "btree_map::Range value": [T] btree_map::Range<'static, Both, T> => sc::btree_map::Range<'static, Both, T>;
        "btree_map::RangeMut value": [T] btree_map::RangeMut<'static, Both, T> => sc::btree_map::RangeMut<'static, Both, T>;
        "btree_map::IntoIter key": [T] btree_map::IntoIter<T, Both> => sc::btree_map::IntoIter<T, Both>;
        "btree_map::IntoIter value": [T] btree_map::IntoIter<Both, T> => sc::btree_map::IntoIter<Both, T>;
        "btree_map::IntoKeys": [T] btree_map::IntoKeys<T, Both> => sc::btree_map::IntoKeys<T, Both>;
        "btree_map::IntoValues": [T] btree_map::IntoValues<Both, T> => sc::btree_map::IntoValues<Both, T>;
        "btree_map::OccupiedEntry key": [T] btree_map::OccupiedEntry<'static, T, Both> => sc::btree_map::OccupiedEntry<'static, T, Both>;
        "btree_map::OccupiedEntry value": [T] btree_map::OccupiedEntry<'static, Both, T> => sc::btree_map::OccupiedEntry<'static, Both, T>;
        "btree_map::VacantEntry key": [T] btree_map::VacantEntry<'static, T, Both> => sc::btree_map::VacantEntry<'static, T, Both>;
        "btree_map::VacantEntry value": [T] btree_map::VacantEntry<'static, Both, T> => sc::btree_map::VacantEntry<'static, Both, T>;
        "btree_map::Entry value": [T] btree_map::Entry<'static, Both, T> => sc::btree_map::Entry<'static, Both, T>;
        "BTreeSet": [T] jzap::collections::BTreeSet<T> => sc::BTreeSet<T>;
        "btree_set::Iter": [T] btree_set::Iter<'static, T> => sc::btree_set::Iter<'static, T>;
        "btree_set::Range": [T] btree_set::Range<'static, T> => sc::btree_set::Range<'static, T>;
        "btree_set::IntoIter": [T] btree_set::IntoIter<T> => sc::btree_set::IntoIter<T>;
        "btree_set::Difference": [T] btree_set::Difference<'static, T> => sc::btree_set::Difference<'static, T>;
        "btree_set::Intersection": [T] btree_set::Intersection<'static, T> => sc::btree_set::Intersection<'static, T>;
        "btree_set::Union": [T] btree_set::Union<'static, T> => sc::btree_set::Union<'static, T>;
        "btree_set::SymmetricDifference": [T] btree_set::SymmetricDifference<'static, T> => sc::btree_set::SymmetricDifference<'static, T>;
        "HashMap key": [T] jzap::collections::HashMap<T, Both> => sc::HashMap<T, Both>;
        "HashMap value": [T] jzap::collections::HashMap<Both, T> => sc::HashMap<Both, T>;
        "HashMap builder": [T] jzap::collections::HashMap<Both, Both, T> => sc::HashMap<Both, Both, T>;
        "hash_map::Iter key": [T] hash_map::Iter<'static, T, Both> => sc::hash_map::Iter<'static, T, Both>;
        "hash_map::Iter value": [T] hash_map::Iter<'static, Both, T> => sc::hash_map::Iter<'static, Both, T>;
        "hash_map::IterMut key": [T] hash_map::IterMut<'static, T, Both> => sc::hash_map::IterMut<'static, T, Both>;
        "hash_map::IterMut value": [T] hash_map::IterMut<'static, Both, T> => sc::hash_map::IterMut<'static, Both, T>;
        "hash_map::Keys": [T] hash_map::Keys<'static, T, Both> => sc::hash_map::Keys<'static, T, Both>;
        "hash_map::Values": [T] hash_map::Values<'static, Both, T> => sc::hash_map::Values<'static, Both, T>;
        "hash_map::ValuesMut": [T] hash_map::ValuesMut<'static, Both, T> => sc::hash_map::ValuesMut<'static, Both, T>;
        "hash_map::IntoIter key": [T] hash_map::IntoIter<T, Both> => sc::hash_map::IntoIter<T, Both>;
        "hash_map::IntoIter value": [T] hash_map::IntoIter<Both, T> => sc::hash_map::IntoIter<Both, T>;
        "hash_map::IntoKeys": [T] hash_map::IntoKeys<T, Both> => sc::hash_map::IntoKeys<T, Both>;
        "hash_map::IntoValues": [T] hash_map::IntoValues<Both, T> => sc::hash_map::IntoValues<Both, T>;
        "hash_map::Drain key": [T] hash_map::Drain<'static, T, Both> => sc::hash_map::Drain<'static, T, Both>;
        "hash_map::Drain value": [T] hash_map::Drain<'static, Both, T> => sc::hash_map::Drain<'static, Both, T>;
        "hash_map::OccupiedEntry key": [T] hash_map::OccupiedEntry<'static, T, Both> => sc::hash_map::OccupiedEntry<'static, T, Both>;
        "hash_map::OccupiedEntry value": [T] hash_map::OccupiedEntry<'static, Both, T> => sc::hash_map::OccupiedEntry<'static, Both, T>;
        "hash_map::VacantEntry key": [T] hash_map::VacantEntry<'static, T, Both> => sc::hash_map::VacantEntry<'static, T, Both>;
        "hash_map::VacantEntry value": [T] hash_map::VacantEntry<'static, Both, T> => sc::hash_map::VacantEntry<'static, Both, T>;
        "HashSet": [T] jzap::collections::HashSet<T> => sc::HashSet<T>;
        "HashSet builder": [T] jzap::collections::HashSet<Both, T> => sc::HashSet<Both, T>;
        "hash_set::Iter": [T] hash_set::Iter<'static, T> => sc::hash_set::Iter<'static, T>;
        "hash_set::IntoIter": [T] hash_set::IntoIter<T> => sc::hash_set::IntoIter<T>;
        "hash_set::Drain": [T] hash_set::Drain<'static, T> => sc::hash_set::Drain<'static, T>;
        "hash_set::Difference": [T] hash_set::Difference<'static, T> => sc::hash_set::Difference<'static, T, sc::hash_map::RandomState>;
        "hash_set::Intersection": [T] hash_set::Intersection<'static, T> => sc::hash_set::Intersection<'static, T, sc::hash_map::RandomState>;
        "hash_set::Union": [T] hash_set::Union<'static, T> => sc::hash_set::Union<'static, T, sc::hash_map::RandomState>;
        "hash_set::SymmetricDifference": [T] hash_set::SymmetricDifference<'static, T> => sc::hash_set::SymmetricDifference<'static, T, sc::hash_map::RandomState>;
        "Rc": [T] jzap::rc::Rc<T> => Rc<T>;
        "rc::Weak": [T] jzap::rc::Weak<T> => std::rc::Weak<T>;
        "Arc": [T] jzap::sync::Arc<T> => std::sync::Arc<T>;
        "sync::Weak": [T] jzap::sync::Weak<T> => std::sync::Weak<T>;
        "Arc<[T]>": [T] jzap::sync::Arc<[T]> => std::sync::Arc<[T]>;
        "Slice": [T] jzap::slice::Slice<'static, T> => &'static [T];
        "SliceMut": [T] jzap::slice::SliceMut<'static, T> => &'static mut [T];
        "Scratch": [T] jzap::scratch::Scratch<'static, T> => &'static mut [T];
        "ScratchBuf": [T] jzap::scratch::ScratchBuf<T> => Vec<T>;
        "btree_map::ExtractIf value": [T] btree_map::ExtractIf<'static, Both, T, jzap::budget::Unmetered, fn(&Both, &mut T) -> Result<bool, ()>, ()> => sc::btree_map::ExtractIf<'static, Both, T, std::ops::RangeFull, fn(&Both, &mut T) -> bool>;
        "hash_map::ExtractIf value": [T] hash_map::ExtractIf<'static, Both, T, hash_map::RandomState, jzap::budget::Unmetered, fn(&Both, &mut T) -> bool> => sc::hash_map::ExtractIf<'static, Both, T, fn(&Both, &mut T) -> bool>;
        "btree_map::NodeView key": [T] btree_map::view::NodeView<'static, T, Both> => &'static [(T, Both)];
        "btree_map::NodeView value": [T] btree_map::view::NodeView<'static, Both, T> => &'static [(Both, T)];
        "btree_map::NodeSlice value": [T] btree_map::view::NodeSlice<'static, Both, T> => &'static [(Both, T)];
        "Rejected": [T] jzap::Rejected<T> => (T, u8);
    );
    assert!(
        f.is_empty(),
        "{} auto-trait differences:\n{}",
        f.len(),
        f.join("\n")
    );
}

/// Types without a standard counterpart: the expectation follows from what they hold.
#[test]
fn policy_and_slot_types_have_the_expected_auto_traits() {
    use jzap::budget::*;
    // A single-threaded ledger uses `Cell`; a borrow of it must stay on its thread.
    assert_eq!(facts!(MemoryBudget), (true, false));
    assert_eq!(facts!(Budget<'static>), (false, false));
    assert_eq!(facts!(Reservation<'static>), (false, false));
    assert_eq!(facts!(Charge<'static>), (false, false));
    assert_eq!(facts!(OwnedBudget), (false, false));
    assert_eq!(facts!(jzap::vec::Vec<u8, Budget<'static>>), (false, false));
    assert_eq!(facts!(jzap::vec::Vec<u8, OwnedBudget>), (false, false));
    assert_eq!(
        facts!(jzap::collections::BTreeMap<u8, u8, Budget<'static>>),
        (false, false)
    );
    assert_eq!(
        facts!(jzap::collections::HashMap<u8, u8, jzap::collections::hash_map::RandomState, OwnedBudget>),
        (false, false)
    );
    assert_eq!(facts!(jzap::rc::Rc<u8, OwnedBudget>), (false, false));
    // The atomic ledger and its handles cross threads.
    assert_eq!(facts!(ConcurrentMemoryBudget), (true, true));
    assert_eq!(facts!(ConcurrentBudget<'static>), (true, true));
    assert_eq!(facts!(ConcurrentReservation<'static>), (true, true));
    assert_eq!(facts!(ConcurrentCharge<'static>), (true, true));
    assert_eq!(facts!(OwnedConcurrentBudget), (true, true));
    assert_eq!(
        facts!(jzap::sync::Arc<u8, OwnedConcurrentBudget>),
        (true, true)
    );
    assert_eq!(
        facts!(jzap::vec::Vec<u8, ConcurrentBudget<'static>>),
        (true, true)
    );
    // An atomic handle whose policy is thread-bound stays thread-bound.
    assert_eq!(facts!(jzap::sync::Arc<u8, OwnedBudget>), (false, false));
    // `InitSlot` is narrower than the `&mut T` it stands for.
    assert_eq!(facts!(jzap::init::InitSlot<'static, Both>), (false, false));
}

/// Covariance that std offers for owners and shared iterators: a longer element
/// lifetime is accepted where a shorter one is expected. Mutable views and
/// entries must refuse the same conversion; `tools/compile-fail` checks that.
#[allow(clippy::needless_lifetimes)]
mod covariant {
    use jzap::collections::{
        BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, btree_map, btree_set, hash_map, hash_set,
    };
    type L = &'static str;
    macro_rules! shorten { ($($name:ident: $long:ty => $short:ty;)*) => {$(
        fn $name<'a>(v: $long) -> $short { v }
    )*}}
    shorten!(
        boxed: jzap::boxed::Box<L> => jzap::boxed::Box<&'a str>;
        vec: jzap::vec::Vec<L> => jzap::vec::Vec<&'a str>;
        vec_into_iter: jzap::vec::IntoIter<L> => jzap::vec::IntoIter<&'a str>;
        deque: jzap::deque::VecDeque<L> => jzap::deque::VecDeque<&'a str>;
        deque_iter: jzap::deque::Iter<'a, L> => jzap::deque::Iter<'a, &'a str>;
        heap: BinaryHeap<L> => BinaryHeap<&'a str>;
        btree_map: BTreeMap<L, L> => BTreeMap<&'a str, &'a str>;
        btree_iter: btree_map::Iter<'a, L, L> => btree_map::Iter<'a, &'a str, &'a str>;
        btree_into_iter: btree_map::IntoIter<L, L> => btree_map::IntoIter<&'a str, &'a str>;
        btree_set: BTreeSet<L> => BTreeSet<&'a str>;
        btree_set_iter: btree_set::Iter<'a, L> => btree_set::Iter<'a, &'a str>;
        hash_map: HashMap<L, L> => HashMap<&'a str, &'a str>;
        hash_iter: hash_map::Iter<'a, L, L> => hash_map::Iter<'a, &'a str, &'a str>;
        hash_into_iter: hash_map::IntoIter<L, L> => hash_map::IntoIter<&'a str, &'a str>;
        hash_set: HashSet<L> => HashSet<&'a str>;
        hash_set_iter: hash_set::Iter<'a, L> => hash_set::Iter<'a, &'a str>;
        rc: jzap::rc::Rc<L> => jzap::rc::Rc<&'a str>;
        rc_weak: jzap::rc::Weak<L> => jzap::rc::Weak<&'a str>;
        arc: jzap::sync::Arc<L> => jzap::sync::Arc<&'a str>;
        slice: jzap::slice::Slice<'a, L> => jzap::slice::Slice<'a, &'a str>;
        scratch_buf: jzap::scratch::ScratchBuf<L> => jzap::scratch::ScratchBuf<&'a str>;
    );
}
