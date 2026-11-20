//! Actual product functions; Kani assertions in bounded arithmetic remain active.
use super::*;
#[kani::proof]
#[kani::unwind(17)]
fn group_masks_all_bytes() {
    let bytes: [u8; 16] = kani::any();
    let tag: u8 = kani::any();
    let equal = unsafe { group::matching(bytes.as_ptr(), tag) };
    let vacant = unsafe { group::vacant(bytes.as_ptr()) };
    assert_eq!(
        unsafe { group::any_empty(bytes.as_ptr()) },
        bytes.contains(&group::EMPTY)
    );
    assert_eq!(
        unsafe { group::first_vacant(bytes.as_ptr()) },
        group::first(vacant)
    );
    let vacancy = unsafe { group::Vacancy::load(bytes.as_ptr()) };
    assert_eq!(vacancy.found(), vacant != 0);
    assert_eq!(vacancy.lane(), group::first(vacant));
    for i in 0..16 {
        assert_eq!((equal >> i) & 1 != 0, bytes[i] == tag);
        assert_eq!((vacant >> i) & 1 != 0, bytes[i] & 128 != 0);
    }
    if equal != 0 {
        let first = group::first(equal);
        assert!(first < 16);
        assert_eq!(bytes[first], tag);
        let rest = group::clear_first(equal);
        assert!(rest < equal);
        assert_eq!(rest, equal & !(1u16 << first));
    }
}
#[kani::proof]
fn mirror_and_probe_all_machine_word_shapes() {
    let buckets: usize = kani::any();
    let i: usize = kani::any();
    kani::assume(buckets >= 4 && buckets <= isize::MAX as usize - 16);
    kani::assume(buckets.is_power_of_two());
    kani::assume(i < buckets);
    let m = raw::mirror(i, buckets);
    assert!(m < buckets + 16);
    assert!(m >= 16);
    assert_eq!(m & (buckets - 1), i);
    let stride: usize = kani::any();
    kani::assume(stride < buckets && stride % 16 == 0);
    let (next, step) = raw::probe_next(i, stride, buckets - 1);
    assert!(next < buckets);
    assert!(step <= buckets.max(16));
    assert_eq!(step % 16, 0);
    let group_read_end = i + 16;
    assert!(group_read_end < buckets + 16);
    assert!(i + 1 <= buckets);
    assert!(raw::tag(kani::any()) < 128);
}
#[kani::proof]
fn checked_layout_representative_sizes_all_capacities() {
    let size: usize = match kani::any::<u8>() % 3 {
        0 => 0,
        1 => 2,
        _ => 64,
    };
    let align: usize = 2;
    let buckets: usize = kani::any();
    kani::assume(align.is_power_of_two());
    // All possible native tuple layouts. Size is a multiple of its alignment.
    kani::assume(size <= isize::MAX as usize && size & (align - 1) == 0);
    if let Ok((layout, offset)) = raw::checked_layout(size, align, buckets) {
        assert!(layout.size() <= isize::MAX as usize);
        assert!(buckets <= isize::MAX as usize - 16);
        assert_eq!(offset & (align.max(16) - 1), 0);
        assert!(size.checked_mul(buckets).unwrap() <= offset);
        assert_eq!(
            offset.checked_add(buckets).unwrap().checked_add(16),
            Some(layout.size())
        );
        assert!(offset + buckets <= layout.size() - 16);
    }
}
#[kani::proof]
#[kani::unwind(20)]
fn actual_table_slot_publication_and_retirement() {
    let mut map = HashMap::<u8, u8>::with_hasher(RandomState::from_seed(0, 0));
    map.table = RawTable::allocate(4, crate::budget::Unmetered.budget()).unwrap();
    let a: u8 = kani::any();
    let v: u8 = kani::any();
    let h: u64 = kani::any();
    let i = match map.table.find_insert(h, |_| false) {
        Some(raw::InsertSlot::Vacant(i)) => i,
        _ => panic!("fresh table has a real vacancy"),
    };
    assert!(i < 4);
    unsafe {
        map.table.insert_at(i, h, (a, v));
    }
    assert_eq!(map.table.len.get(), 1);
    assert!(
        matches!(map.table.find_insert(h, |k| *k == a), Some(raw::InsertSlot::Occupied(j)) if j==i)
    );
    if let Some(raw::InsertSlot::Vacant(j)) = map.table.find_insert(h, |_| false) {
        assert!(j < 4);
        assert_ne!(j, i);
        assert!(unsafe { map.table.control(j) } & 0x80 != 0);
    } else {
        panic!("remaining growth has a real vacancy");
    }

    assert_eq!(unsafe { map.table.control(i) }, raw::tag(h));
    assert_eq!(unsafe { map.table.slot(i).read() }, (a, v));
    let copy = map.try_clone().unwrap();
    assert_eq!(unsafe { copy.table.control(i) }, raw::tag(h));
    assert_eq!(unsafe { copy.table.slot(i).read() }, (a, v));
    assert_eq!(copy.len(), 1);
    let mut iter = copy.iter();
    assert_eq!(iter.next(), Some((&a, &v)));
    assert_eq!(iter.next(), None);
    assert_eq!(unsafe { map.table.erase(i) }, (a, v));
    assert_eq!(map.table.len.get(), 0);
    assert_eq!(map.table.growth.get(), 3);
}

#[kani::proof]
#[kani::unwind(34)]
fn actual_insert_probe_keeps_tombstones_until_later_matches() {
    let mut map = HashMap::<u8, u8>::with_hasher(RandomState::from_seed(0, 0));
    map.table = RawTable::allocate(32, crate::budget::Unmetered.budget()).unwrap();
    for i in 0..17 {
        // SAFETY: distinct vacant slots, below the admitted 28-entry capacity.
        unsafe {
            map.table.insert_at(i, 0, (i as u8, 42));
        }
    }
    assert!(matches!(
        map.table.find_insert(0, |k| *k == 16),
        Some(raw::InsertSlot::Occupied(16))
    ));
    assert!(matches!(
        map.table.find_insert(0, |_| false),
        Some(raw::InsertSlot::Vacant(17))
    ));
    assert_eq!(unsafe { map.table.erase(0) }, (0, 42));
    assert_eq!(unsafe { map.table.control(0) }, group::DELETED);
    assert!(matches!(
        map.table.find_insert(0, |k| *k == 16),
        Some(raw::InsertSlot::Occupied(16))
    ));
    assert!(matches!(
        map.table.find_insert(0, |_| false),
        Some(raw::InsertSlot::Vacant(0))
    ));
    assert_eq!(map.len(), 16);
    map.clear();
    for i in 0..32 {
        // SAFETY: admitted controls, no FULL owner is being hidden.
        unsafe {
            map.table.set_control(i, group::DELETED);
        }
    }
    map.table.growth = Checked::new(0);
    assert!(matches!(
        map.table.find_insert(0, |_| false),
        Some(raw::InsertSlot::Vacant(0))
    ));
}

#[derive(Debug)]
struct SetKey {
    class: u8,
    identity: u8,
}
#[kani::proof]
#[kani::unwind(17)]
fn hash_set_replace_preserves_slot_class_length_and_credit() {
    let class: u8 = kani::any();
    let old: u8 = kani::any();
    let new: u8 = kani::any();
    let budget = crate::budget::Unmetered.budget();
    let mut owner = raw::Owner {
        table: RawTable::<SetKey, ()>::allocate(4, budget).unwrap(),
        budget,
    };
    let index = kani::any::<usize>() % 4;
    let hash = kani::any::<u64>();
    // The occupied-search proof supplies this FULL index. This kernel proves
    // the original replacement primitive, independently of hash-table probing.
    unsafe {
        owner.table.insert_at(
            index,
            hash,
            (
                SetKey {
                    class,
                    identity: old,
                },
                (),
            ),
        );
    }
    let capacity = owner.table.capacity();
    let tag = unsafe { owner.table.control(index) };
    let previous = unsafe {
        owner.table.replace_key(
            index,
            SetKey {
                class,
                identity: new,
            },
        )
    };
    assert_eq!(previous.class, class);
    assert_eq!(previous.identity, old);
    assert_eq!(owner.table.len.get(), 1);
    assert_eq!(owner.table.capacity(), capacity);
    assert_eq!(unsafe { owner.table.control(index) }, tag);
    let current = unsafe { &*owner.table.slot(index) };
    assert_eq!(current.0.class, class);
    assert_eq!(current.0.identity, new);
    assert_eq!(unsafe { owner.table.erase(index) }.0.identity, new);
    assert_eq!(owner.table.len.get(), 0);
}
