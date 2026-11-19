//! Borrow and variance rules that the compiler must enforce for jzap types.
//!
//! Each case is a pair of doctests with the same prelude. The first must
//! compile, so a typo cannot make the second one "fail" for the wrong reason.
//! The second must be rejected. A nightly toolchain also checks the error code;
//! a stable toolchain only checks that compilation fails.
//!
//! ```sh
//! cargo test --locked --offline --manifest-path tools/compile-fail/Cargo.toml --doc
//! ```
macro_rules! case {
    ($(#[$m:meta])* $name:ident [] { $prelude:literal accepted: $ok:literal rejected: $bad:literal }) => {
        $(#[$m])*
        #[doc = concat!("```\n", $prelude, "\n", $ok, "\n```\n\n```compile_fail\n", $prelude, "\n", $bad, "\n```")]
        pub mod $name {}
    };
    ($(#[$m:meta])* $name:ident [$code:literal] { $prelude:literal accepted: $ok:literal rejected: $bad:literal }) => {
        $(#[$m])*
        #[doc = concat!("```\n", $prelude, "\n", $ok, "\n```\n\n```compile_fail,", $code, "\n", $prelude, "\n", $bad, "\n```")]
        pub mod $name {}
    };
}

// ---- Variance: a mutable view must not shorten the element lifetime. -------------------
// Otherwise a short-lived reference could be written through the view into a
// container that promises `'static` elements.
macro_rules! invariant {
    ($($name:ident: $shared:literal, $unique:literal;)*) => {$(
        // The borrow checker reports this as an uncoded lifetime error.
        case!($name [] {
            "use jzap::collections::*; use jzap::*;"
            accepted: $shared
            rejected: $unique
        });
    )*};
}
invariant!(
    deque_iter_mut:
        "fn f<'a, 'b>(v: deque::Iter<'a, &'static str>) -> deque::Iter<'a, &'b str> { v }",
        "fn f<'a, 'b>(v: deque::IterMut<'a, &'static str>) -> deque::IterMut<'a, &'b str> { v }";
    deque_drain_source:
        "fn f<'a, 'b>(v: &'a deque::VecDeque<&'static str>) -> &'a deque::VecDeque<&'b str> { v }",
        "fn f<'a, 'b>(v: &'a mut deque::VecDeque<&'static str>) -> &'a mut deque::VecDeque<&'b str> { v }";
    btree_iter_mut:
        "fn f<'a, 'b>(v: btree_map::Iter<'a, u8, &'static str>) -> btree_map::Iter<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: btree_map::IterMut<'a, u8, &'static str>) -> btree_map::IterMut<'a, u8, &'b str> { v }";
    btree_range_mut:
        "fn f<'a, 'b>(v: btree_map::Range<'a, u8, &'static str>) -> btree_map::Range<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: btree_map::RangeMut<'a, u8, &'static str>) -> btree_map::RangeMut<'a, u8, &'b str> { v }";
    btree_values_mut:
        "fn f<'a, 'b>(v: btree_map::Values<'a, u8, &'static str>) -> btree_map::Values<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: btree_map::ValuesMut<'a, u8, &'static str>) -> btree_map::ValuesMut<'a, u8, &'b str> { v }";
    btree_occupied_entry:
        "fn f<'a, 'b>(v: btree_map::Iter<'a, u8, &'static str>) -> btree_map::Iter<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: btree_map::OccupiedEntry<'a, u8, &'static str>) -> btree_map::OccupiedEntry<'a, u8, &'b str> { v }";
    btree_vacant_entry:
        "fn f<'a, 'b>(v: btree_map::Iter<'a, u8, &'static str>) -> btree_map::Iter<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: btree_map::VacantEntry<'a, u8, &'static str>) -> btree_map::VacantEntry<'a, u8, &'b str> { v }";
    btree_entry_key:
        "fn f<'a, 'b>(v: btree_map::Iter<'a, &'static str, u8>) -> btree_map::Iter<'a, &'b str, u8> { v }",
        "fn f<'a, 'b>(v: btree_map::Entry<'a, &'static str, u8>) -> btree_map::Entry<'a, &'b str, u8> { v }";
    hash_iter_mut:
        "fn f<'a, 'b>(v: hash_map::Iter<'a, u8, &'static str>) -> hash_map::Iter<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: hash_map::IterMut<'a, u8, &'static str>) -> hash_map::IterMut<'a, u8, &'b str> { v }";
    hash_values_mut:
        "fn f<'a, 'b>(v: hash_map::Values<'a, u8, &'static str>) -> hash_map::Values<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: hash_map::ValuesMut<'a, u8, &'static str>) -> hash_map::ValuesMut<'a, u8, &'b str> { v }";
    hash_occupied_entry:
        "fn f<'a, 'b>(v: hash_map::Iter<'a, u8, &'static str>) -> hash_map::Iter<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: hash_map::OccupiedEntry<'a, u8, &'static str>) -> hash_map::OccupiedEntry<'a, u8, &'b str> { v }";
    hash_vacant_entry:
        "fn f<'a, 'b>(v: hash_map::Iter<'a, u8, &'static str>) -> hash_map::Iter<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: hash_map::VacantEntry<'a, u8, &'static str>) -> hash_map::VacantEntry<'a, u8, &'b str> { v }";
    hash_drain:
        "fn f<'a, 'b>(v: hash_map::Iter<'a, u8, &'static str>) -> hash_map::Iter<'a, u8, &'b str> { v }",
        "fn f<'a, 'b>(v: hash_map::Drain<'a, u8, &'static str>) -> hash_map::Drain<'a, u8, &'b str> { v }";
    heap_peek_mut:
        "fn f<'a, 'b>(v: &'a BinaryHeap<&'static str>) -> &'a BinaryHeap<&'b str> { v }",
        "fn f<'a, 'b>(v: binary_heap::PeekMut<'a, &'static str>) -> binary_heap::PeekMut<'a, &'b str> { v }";
    slice_mut:
        "fn f<'a, 'b>(v: slice::Slice<'a, &'static str>) -> slice::Slice<'a, &'b str> { v }",
        "fn f<'a, 'b>(v: slice::SliceMut<'a, &'static str>) -> slice::SliceMut<'a, &'b str> { v }";
    scratch:
        "fn f<'a, 'b>(v: slice::Slice<'a, &'static str>) -> slice::Slice<'a, &'b str> { v }",
        "fn f<'a, 'b>(v: scratch::Scratch<'a, &'static str>) -> scratch::Scratch<'a, &'b str> { v }";
    init_slot:
        "fn f<'a, 'b>(v: slice::Slice<'a, &'static str>) -> slice::Slice<'a, &'b str> { v }",
        "fn f<'a, 'b>(v: init::InitSlot<'a, &'static str>) -> init::InitSlot<'a, &'b str> { v }";
    vec_drain_source:
        "fn f<'a, 'b>(v: &'a vec::Vec<&'static str>) -> &'a vec::Vec<&'b str> { v }",
        "fn f<'a, 'b>(v: &'a mut vec::Vec<&'static str>) -> &'a mut vec::Vec<&'b str> { v }";
    rc_weak_cell:
        "fn f<'a>(v: rc::Rc<&'static str>) -> rc::Rc<&'a str> { v }",
        "fn f<'a, 'b>(v: &'a mut rc::Rc<&'static str>) -> &'a mut rc::Rc<&'b str> { v }";
);

// ---- Borrows: a view never outlives or aliases its owner. -------------------------------
case!(vec_iter_outlives_owner ["E0597"] {
    "use jzap::vec::Vec; let it;"
    accepted: "let v = Vec::<u8>::new(); it = v.iter(); assert!(it.len() == 0);"
    rejected: "{ let v = Vec::<u8>::new(); it = v.iter(); } assert!(it.len() == 0);"
});
case!(vec_reference_survives_push ["E0502"] {
    "use jzap::vec::Vec; let mut v = Vec::new(); v.try_push(1u8).unwrap();"
    accepted: "let first = v.get(0).unwrap(); assert_eq!(*first, 1); v.try_push(2).unwrap();"
    rejected: "let first = v.get(0).unwrap(); v.try_push(2).unwrap(); assert_eq!(*first, 1);"
});
case!(vec_used_while_drained ["E0502"] {
    "use jzap::vec::Vec; let mut v = Vec::new(); v.try_push(1u8).unwrap();"
    accepted: "let d = v.try_drain(..).unwrap(); drop(d); assert_eq!(v.len(), 0);"
    rejected: "let d = v.try_drain(..).unwrap(); assert_eq!(v.len(), 0); drop(d);"
});
case!(vec_outlives_budget ["E0597"] {
    "use jzap::{vec::Vec, budget::{Budget, MemoryBudget}}; let outer = MemoryBudget::new(64); let v;"
    accepted: "let ledger = &outer; v = Vec::<u8, _>::new_in(Budget::Limited(ledger)); assert_eq!(v.len(), 0);"
    rejected: "{ let ledger = MemoryBudget::new(64); v = Vec::<u8, _>::new_in(Budget::Limited(&ledger)); } assert_eq!(v.len(), 0);"
});
case!(string_used_while_drained ["E0502"] {
    "use jzap::string::String; let mut s = String::new(); s.try_push_str(\"ab\").unwrap();"
    accepted: "let d = s.try_drain(..).unwrap(); drop(d); assert_eq!(s.len(), 0);"
    rejected: "let d = s.try_drain(..).unwrap(); assert_eq!(s.len(), 0); drop(d);"
});
case!(deque_iter_mut_aliases_owner ["E0499"] {
    "use jzap::deque::VecDeque; let mut v = VecDeque::new(); v.try_push_back(1u8).unwrap();"
    accepted: "let a = v.iter_mut(); drop(a); let b = v.iter_mut(); drop(b);"
    rejected: "let a = v.iter_mut(); let b = v.iter_mut(); drop(a); drop(b);"
});
case!(deque_used_while_drained ["E0502"] {
    "use jzap::deque::VecDeque; let mut v = VecDeque::new(); v.try_push_back(1u8).unwrap();"
    accepted: "let d = v.drain(..).unwrap(); drop(d); assert_eq!(v.len(), 0);"
    rejected: "let d = v.drain(..).unwrap(); assert_eq!(v.len(), 0); drop(d);"
});
case!(heap_used_while_peeked ["E0502"] {
    "use jzap::collections::BinaryHeap; let mut h = BinaryHeap::new(); h.try_push(1u8).unwrap();"
    accepted: "let top = h.peek_mut().unwrap(); drop(top); assert_eq!(h.len(), 1);"
    rejected: "let top = h.peek_mut().unwrap(); assert_eq!(h.len(), 1); drop(top);"
});
case!(btree_value_survives_insert ["E0502"] {
    "use jzap::collections::BTreeMap; let mut m = BTreeMap::new(); m.try_insert(1u8, 1u8).unwrap();"
    accepted: "let value = m.get(&1).unwrap(); assert_eq!(*value, 1); m.try_insert(2, 2).unwrap();"
    rejected: "let value = m.get(&1).unwrap(); m.try_insert(2, 2).unwrap(); assert_eq!(*value, 1);"
});
case!(btree_used_while_entry_held ["E0502"] {
    "use jzap::collections::BTreeMap; let mut m = BTreeMap::new(); m.try_insert(1u8, 1u8).unwrap();"
    accepted: "let e = m.entry(1); drop(e); assert_eq!(m.len(), 1);"
    rejected: "let e = m.entry(1); assert_eq!(m.len(), 1); drop(e);"
});
case!(btree_iter_mut_outlives_owner ["E0597"] {
    "use jzap::collections::BTreeMap; let it;"
    accepted: "let mut m = BTreeMap::<u8, u8>::new(); it = m.iter_mut(); drop(it);"
    rejected: "{ let mut m = BTreeMap::<u8, u8>::new(); it = m.iter_mut(); } drop(it);"
});
case!(btree_node_slice_outlives_storage ["E0597"] {
    "use jzap::collections::btree_map::view::NodeSlice; let node;"
    accepted: "let (k, v) = ([1u8, 2], [3u8, 4]); node = NodeSlice::<u8, u8>::new(&k, &v, None).unwrap(); assert_eq!(node.keys().len(), 2);"
    rejected: "{ let (k, v) = ([1u8, 2], [3u8, 4]); node = NodeSlice::<u8, u8>::new(&k, &v, None).unwrap(); } assert_eq!(node.keys().len(), 2);"
});
case!(hash_value_survives_insert ["E0502"] {
    "use jzap::collections::HashMap; let mut m = HashMap::try_new().unwrap(); m.try_insert(1u8, 1u8).unwrap();"
    accepted: "let value = m.get(&1).unwrap(); assert_eq!(*value, 1); m.try_insert(2, 2).unwrap();"
    rejected: "let value = m.get(&1).unwrap(); m.try_insert(2, 2).unwrap(); assert_eq!(*value, 1);"
});
case!(hash_used_while_drained ["E0502"] {
    "use jzap::collections::HashMap; let mut m = HashMap::try_new().unwrap(); m.try_insert(1u8, 1u8).unwrap();"
    accepted: "let d = m.drain(); drop(d); assert_eq!(m.len(), 0);"
    rejected: "let d = m.drain(); assert_eq!(m.len(), 0); drop(d);"
});
case!(hash_used_while_entry_held ["E0502"] {
    "use jzap::collections::HashMap; let mut m = HashMap::try_new().unwrap(); m.try_insert(1u8, 1u8).unwrap();"
    accepted: "let e = m.entry(1); drop(e); assert_eq!(m.len(), 1);"
    rejected: "let e = m.entry(1); assert_eq!(m.len(), 1); drop(e);"
});
case!(slice_view_outlives_storage ["E0597"] {
    "use jzap::slice::Slice; let view;"
    accepted: "let data = [1u8, 2]; view = Slice::new(&data); assert_eq!(*view.get(0).unwrap(), 1);"
    rejected: "{ let data = [1u8, 2]; view = Slice::new(&data); } assert_eq!(*view.get(0).unwrap(), 1);"
});
case!(scratch_outlives_storage ["E0597"] {
    "use jzap::scratch::Scratch; use core::mem::MaybeUninit; let mut outer = [MaybeUninit::<u8>::uninit(); 4]; let s;"
    accepted: "let store = &mut outer; s = Scratch::new(store); assert_eq!(s.as_slice().len(), 0);"
    rejected: "{ let mut store = [MaybeUninit::<u8>::uninit(); 4]; s = Scratch::new(&mut store); } assert_eq!(s.as_slice().len(), 0);"
});
case!(scratch_storage_used_while_borrowed ["E0506"] {
    "use jzap::scratch::Scratch; use core::mem::MaybeUninit; let mut store = [MaybeUninit::<u8>::uninit(); 4];"
    accepted: "let mut s = Scratch::new(&mut store); s.try_push(1).unwrap(); drop(s); store[0] = MaybeUninit::new(2);"
    rejected: "let mut s = Scratch::new(&mut store); store[0] = MaybeUninit::new(2); s.try_push(1).unwrap();"
});
case!(scratch_element_survives_push ["E0499"] {
    "use jzap::scratch::Scratch; use core::mem::MaybeUninit; let mut store = [MaybeUninit::<u8>::uninit(); 4]; let mut s = Scratch::new(&mut store);"
    accepted: "let a = s.try_push(1).unwrap(); *a = 3; s.try_push(2).unwrap();"
    rejected: "let a = s.try_push(1).unwrap(); s.try_push(2).unwrap(); *a = 3;"
});
case!(rc_get_mut_survives_clone_site ["E0502"] {
    "use jzap::rc::Rc; let mut a = Rc::try_new(1u8).unwrap();"
    accepted: "let unique = Rc::get_mut(&mut a).unwrap(); *unique = 2; assert_eq!(*a, 2);"
    rejected: "let unique = Rc::get_mut(&mut a).unwrap(); assert_eq!(*a, 1); *unique = 2;"
});

// ---- Threads: thread-bound values stay on their thread. ---------------------------------
case!(rc_crosses_thread ["E0277"] {
    "fn send<T: Send>(_: T) {}"
    accepted: "send(jzap::sync::Arc::try_new(1u8).unwrap());"
    rejected: "send(jzap::rc::Rc::try_new(1u8).unwrap());"
});
case!(arc_of_cell_crosses_thread ["E0277"] {
    "fn send<T: Send>(_: T) {} use core::cell::Cell;"
    accepted: "send(jzap::sync::Arc::try_new(1u8).unwrap());"
    rejected: "send(jzap::sync::Arc::try_new(Cell::new(1u8)).unwrap());"
});
case!(local_budget_crosses_thread ["E0277"] {
    "fn send<T: Send>(_: T) {} use jzap::{vec::Vec, budget::*};"
    accepted: "static L: ConcurrentMemoryBudget = ConcurrentMemoryBudget::new(64); send(Vec::<u8, _>::new_in(ConcurrentBudget(&L)));"
    rejected: "let l = MemoryBudget::new(64); send(Vec::<u8, _>::new_in(Budget::Limited(&l)));"
});
case!(shared_iter_of_cell_crosses_thread ["E0277"] {
    "fn send<T: Send>(_: T) {} use core::cell::Cell; use jzap::collections::HashMap;"
    accepted: "let m = HashMap::<u8, u8>::try_new().unwrap(); send(m.iter());"
    rejected: "let m = HashMap::<u8, Cell<u8>>::try_new().unwrap(); send(m.iter());"
});
