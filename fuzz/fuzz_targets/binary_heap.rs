#![no_main]
use jzap::{
    TryClone,
    budget::{
        AllocationPolicy, Budget, ConcurrentBudget, ConcurrentMemoryBudget, MemoryBudget, Unmetered,
    },
    collections::BinaryHeap,
};
use libfuzzer_sys::fuzz_target;
use std::collections::BinaryHeap as Std;
fn run<P: AllocationPolicy + Copy>(input: &[u8], policy: P, source_policy: P) {
    // Exercise both heap-size relations before the lifecycle tends to grow its
    // receiver beyond the small appended arrays. Separate ledgers must retain
    // their original ownership even when their limits and balances are equal.
    let values: Vec<u16> = input
        .chunks_exact(2)
        .take(32)
        .map(|v| u16::from_le_bytes([v[0], v[1]]))
        .collect();
    for small in [true, false] {
        let split = values.len().min(2);
        let (left, right) = if small {
            (&values[..split], &values[split..])
        } else {
            (&values[split..], &values[..split])
        };
        let mut a = BinaryHeap::new_in(policy);
        let mut b = BinaryHeap::new_in(source_policy);
        a.try_extend(jzap::iter::from_std(left.iter().copied()))
            .unwrap();
        b.try_extend(jzap::iter::from_std(right.iter().copied()))
            .unwrap();
        let mut expected = Std::from(left.to_vec());
        let mut other = Std::from(right.to_vec());
        a.try_append(&mut b).unwrap();
        expected.append(&mut other);
        assert!(b.is_empty());
        assert_eq!(a.into_sorted_vec().as_slice(), expected.into_sorted_vec());
    }
    let mut a = BinaryHeap::new_in(policy);
    let mut b = Std::new();
    for data in input.chunks_exact(3) {
        let value = u16::from_le_bytes([data[1], data[2]]);
        match data[0] % 9 {
            0..=2 => {
                a.try_push(value).unwrap();
                b.push(value);
            }
            3 => assert_eq!(a.pop(), b.pop()),
            4 => {
                if let Some(mut p) = a.peek_mut() {
                    *p = value
                }
                if let Some(mut p) = b.peek_mut() {
                    *p = value
                }
            }
            5 => {
                a.try_retain(|v| Ok::<_, ()>(*v % 3 != 0)).unwrap();
                b.retain(|v| *v % 3 != 0)
            }
            6 => {
                let mut x =
                    BinaryHeap::try_from_array_in([value, value / 2], source_policy).unwrap();
                let mut y = Std::from([value, value / 2]);
                a.try_append(&mut x).unwrap();
                b.append(&mut y);
                assert!(x.is_empty());
            }
            7 => {
                a.try_shrink_to_fit().unwrap();
                b.shrink_to_fit();
            }
            _ => assert_eq!(
                a.try_clone().unwrap().into_sorted_vec().as_slice(),
                b.clone().into_sorted_vec()
            ),
        }
        assert_eq!(a.len(), b.len());
        assert_eq!(a.peek(), b.peek());
    }
    assert_eq!(a.into_sorted_vec().as_slice(), b.into_sorted_vec());
}
fuzz_target!(|input: &[u8]| {
    run(input, Unmetered, Unmetered);
    let local = MemoryBudget::new(usize::MAX);
    let other = MemoryBudget::new(usize::MAX);
    run(input, Budget::Limited(&local), Budget::Limited(&local));
    run(input, Budget::Limited(&local), Budget::Limited(&other));
    assert_eq!((local.occupied(), other.occupied()), (0, 0));
    let shared = ConcurrentMemoryBudget::new(usize::MAX);
    let other = ConcurrentMemoryBudget::new(usize::MAX);
    run(input, ConcurrentBudget(&shared), ConcurrentBudget(&shared));
    run(input, ConcurrentBudget(&shared), ConcurrentBudget(&other));
    assert_eq!((shared.used(), other.used()), (0, 0));
});
