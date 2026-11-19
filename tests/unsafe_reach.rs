//! Unsafe blocks that no other runtime test executed.
//!
//! `scripts/unsafe_coverage.py` lists the unsafe blocks whose lines never run
//! in the integration tests; Miri and the sanitizers cannot check those. Each
//! test here runs one of the listed blocks and compares the result with the
//! standard container.
use jzap::{
    collections::{BTreeMap, BinaryHeap, HashMap, btree_map::Entry},
    deque::VecDeque,
    rc::Rc,
    sync::Arc,
};
use std::hash::{BuildHasherDefault, DefaultHasher};

fn ok<T, X>(r: Result<T, X>) -> T {
    match r {
        Ok(v) => v,
        Err(_) => panic!("unexpected refusal"),
    }
}

#[test]
fn heap_drain_debug_reads_the_remaining_span() {
    let mut heap = ok(BinaryHeap::try_from_array([3u8, 9, 1, 7]));
    let mut drain = heap.drain();
    let first = drain.next();
    let text = format!("{drain:?}");
    assert_eq!(text.matches(',').count(), 2, "{text}");
    assert!(first.is_some_and(|v| !text.contains(&format!("{v},")) || text.len() > 4));
    drop(drain);
    assert!(heap.is_empty());
}

#[test]
fn btree_get_mut_entries_and_reverse_range() {
    let mut ours = BTreeMap::new();
    let mut std = std::collections::BTreeMap::new();
    for k in 0..200u32 {
        ok(ours.try_insert(k, k.to_string()));
        std.insert(k, k.to_string());
    }
    for k in (0..200).step_by(7) {
        ours.get_mut(&k).unwrap().push('!');
        std.get_mut(&k).unwrap().push('!');
    }
    assert!(ours.get_mut(&999).is_none());
    match ours.entry(14) {
        Entry::Occupied(entry) => {
            assert_eq!(*entry.key(), 14);
            assert_eq!(entry.get(), "14!");
        }
        Entry::Vacant(_) => panic!("key 14 is present"),
    }
    match ours.entry(500) {
        Entry::Vacant(entry) => {
            let slot = ok(entry.try_insert_with(|k| Ok::<_, ()>(format!("made {k}"))));
            slot.push('?');
        }
        Entry::Occupied(_) => panic!("key 500 is absent"),
    }
    std.insert(500, "made 500?".to_string());
    match ours.entry(501) {
        Entry::Vacant(entry) => assert!(entry.try_insert_with(|_| Err::<String, _>(7u8)).is_err()),
        Entry::Occupied(_) => panic!("key 501 is absent"),
    }
    let back: Vec<_> = ok(ours.try_range(20..180))
        .rev()
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    let expected: Vec<_> = std
        .range(20..180)
        .rev()
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    assert_eq!(back, expected);
    let mut range = ok(ours.try_range(50..60));
    let mut reference = std.range(50..60);
    loop {
        let (a, b) = (range.next(), reference.next());
        assert_eq!(a, b);
        assert_eq!(range.next_back(), reference.next_back());
        if a.is_none() {
            break;
        }
    }
    assert!(
        ours.iter()
            .map(|(k, v)| (*k, v.clone()))
            .eq(std.iter().map(|(k, v)| (*k, v.clone())))
    );
}

#[test]
fn hash_map_get_mut_and_in_place_rehash() {
    type Fixed = BuildHasherDefault<DefaultHasher>;
    let mut ours: HashMap<u32, String, Fixed> = HashMap::with_hasher(Fixed::default());
    let mut std = std::collections::HashMap::new();
    for k in 0..48u32 {
        ok(ours.try_insert(k, k.to_string()));
        std.insert(k, k.to_string());
    }
    for k in (0..48).step_by(5) {
        ours.get_mut(&k).unwrap().push('!');
        std.get_mut(&k).unwrap().push('!');
    }
    assert!(ours.get_mut(&999).is_none());
    // Deletion churn at a small length: tombstones use up the growth allowance, and the
    // table is at most half full, so the next reservation rehashes in place instead of growing.
    let capacity = ours.capacity();
    for k in 8..48 {
        assert_eq!(ours.remove(&k), std.remove(&k));
    }
    for k in 1000..1000 + 64 * capacity as u32 {
        ok(ours.try_insert(k, k.to_string()));
        assert_eq!(ours.remove(&k), Some(k.to_string()));
    }
    assert_eq!(
        ours.capacity(),
        capacity,
        "churn at a small length must not grow the table"
    );
    assert_eq!(ours.len(), std.len());
    for (k, v) in &std {
        assert_eq!(ours.get(k), Some(v));
    }
    for k in 2000..2040 {
        ok(ours.try_insert(k, k.to_string()));
        std.insert(k, k.to_string());
    }
    assert_eq!(ours.len(), std.len());
    for (k, v) in &std {
        assert_eq!(ours.get(k), Some(v));
    }
}

#[test]
fn deque_get_mut_push_mut_and_swap_on_a_wrapped_ring() {
    let mut ours = VecDeque::new();
    let mut std = std::collections::VecDeque::new();
    for k in 0..40u32 {
        if k % 3 == 0 {
            *ok(ours.try_push_front_mut(k.to_string())) += "f";
            std.push_front(k.to_string() + "f");
        } else {
            *ok(ours.try_push_back_mut(k.to_string())) += "b";
            std.push_back(k.to_string() + "b");
        }
    }
    for i in (0..40).step_by(3) {
        ok(ours.get_mut(i)).push('!');
        std.get_mut(i).unwrap().push('!');
        ok(ours.swap(i, 39 - i));
        std.swap(i, 39 - i);
    }
    ok(ours.swap(5, 5));
    assert!(ours.get_mut(40).is_err());
    assert!(ours.swap(0, 40).is_err());
    assert!(ours.iter().eq(std.iter()));
}

#[test]
fn shared_raw_counts_weak_strong_count_and_payload_clone() {
    let a = ok(Arc::try_new(41u64));
    let weak = ok(Arc::try_downgrade(&a));
    assert_eq!(weak.strong_count(), 1);
    let raw = Arc::into_raw(ok(Arc::try_clone(&a)));
    // SAFETY: `raw` came from `into_raw` and carries one live strong credit; the
    // increment adds a second one and each decrement consumes one.
    unsafe {
        ok(Arc::try_increment_strong_count(raw));
        assert_eq!(Arc::strong_count(&a), 3);
        assert_eq!(weak.strong_count(), 3);
        Arc::decrement_strong_count(raw);
        Arc::decrement_strong_count(raw);
    }
    assert_eq!(Arc::strong_count(&a), 1);
    let copy = ok(Arc::try_clone_payload_with(&a, jzap::budget::Unmetered));
    assert_eq!(*copy, 41);
    assert!(!Arc::ptr_eq(&a, &copy));
    drop(a);
    assert_eq!(weak.strong_count(), 0);
    assert_eq!(*copy, 41);

    let text: Rc<str> = ok(Rc::try_from_str("shared text"));
    let copy = ok(Rc::try_clone_payload_with(&text, jzap::budget::Unmetered));
    assert_eq!(copy.as_ref(), "shared text");
    assert!(!Rc::ptr_eq(&text, &copy));
    let slice: Rc<[u16]> = ok(Rc::try_from_slice(&[1, 2, 3]));
    let copy = ok(Rc::try_clone_payload_with(&slice, jzap::budget::Unmetered));
    assert_eq!(copy.as_ref(), &[1, 2, 3]);
}
