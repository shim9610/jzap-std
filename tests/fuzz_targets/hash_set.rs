#![no_main]
use jzap::{
    TryClone, TryIterator,
    collections::{HashSet, hash_map::RandomState},
};
use libfuzzer_sys::fuzz_target;
use std::collections::HashSet as Model;
fuzz_target!(|data: &[u8]| {
    let mut a = HashSet::with_hasher(RandomState::from_seed(0, 0));
    let mut b = Model::with_hasher(RandomState::from_seed(0, 0));
    for chunk in data.chunks_exact(4) {
        let (op, key, arg) = (
            chunk[0] % 13,
            u16::from_le_bytes([chunk[1], chunk[2]]),
            chunk[3],
        );
        match op {
            0 => assert_eq!(a.try_insert(key).unwrap(), b.insert(key)),
            1 => assert_eq!(a.try_replace(key).unwrap(), b.replace(key)),
            2 => assert_eq!(a.take(&key), b.take(&key)),
            3 => assert_eq!(a.get(&key), b.get(&key)),
            4 => {
                a.try_reserve(arg as usize).unwrap();
                b.reserve(arg as usize);
            }
            5 => {
                a.try_shrink_to(arg as usize).unwrap();
                b.shrink_to(arg as usize);
            }
            6 => {
                a.try_retain(|k| Ok::<_, ()>(*k % 3 != u16::from(arg) % 3))
                    .unwrap();
                b.retain(|k| *k % 3 != u16::from(arg) % 3);
            }
            7 => {
                let c = a.try_clone().unwrap();
                assert_eq!(c.len(), b.len());
                for k in &b {
                    assert!(c.contains(k));
                }
            }
            8 => {
                let mut i = a.try_extract_if(|k| Ok::<_, ()>(*k % 5 == u16::from(arg) % 5));
                while let Some(k) = i.try_next().unwrap() {
                    assert!(b.remove(&k));
                }
            }
            9 => {
                let other = HashSet::try_from_iter_in(
                    [key, u16::from(arg)],
                    RandomState::from_seed(0, 0),
                    jzap::budget::Unmetered,
                )
                .unwrap();
                let mut model = Model::with_hasher(RandomState::from_seed(0, 0));
                model.extend([key, u16::from(arg)]);
                for (result, expected) in [
                    (
                        a.union(&other).copied().collect::<Model<_>>(),
                        b.union(&model).copied().collect(),
                    ),
                    (
                        a.intersection(&other).copied().collect(),
                        b.intersection(&model).copied().collect(),
                    ),
                    (
                        a.difference(&other).copied().collect(),
                        b.difference(&model).copied().collect(),
                    ),
                    (
                        a.symmetric_difference(&other).copied().collect(),
                        b.symmetric_difference(&model).copied().collect(),
                    ),
                ] {
                    assert_eq!(result, expected);
                }
                assert_eq!(a.is_disjoint(&other), b.is_disjoint(&model));
                assert_eq!(a.is_subset(&other), b.is_subset(&model));
            }
            10 => {
                let mut copy = a.try_clone().unwrap();
                assert_eq!(copy.drain().count(), b.len());
                assert!(copy.is_empty());
            }
            11 => {
                if arg == 0 {
                    a.clear();
                    b.clear();
                }
            }
            _ => assert_eq!(a.contains(&key), b.contains(&key)),
        }
        assert_eq!(a.len(), b.len());
        for k in &b {
            assert!(a.contains(k));
        }
        assert_eq!(a.iter().count(), b.len());
    }
});
