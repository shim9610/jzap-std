//! Analysis only: actual Heap insertion, same input/output addresses, no timed allocation.
use jzap::{budget::Unmetered, collections::BinaryHeap as Heap, vec::Unlimited};
use std::{collections::BinaryHeap as Std, hint::black_box, time::Instant};

fn random(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

#[inline(never)]
fn std_fill(heap: &mut Std<u64>, values: &[u64]) {
    heap.clear();
    for value in values {
        heap.push(*black_box(value));
    }
}
#[inline(never)]
fn jzap_fill(heap: &mut Heap<u64>, values: &[u64]) {
    heap.clear();
    for value in values {
        heap.try_push(*black_box(value)).unwrap();
    }
}

fn elapsed(mut action: impl FnMut() -> u64, epochs: u64, expected: u64) -> f64 {
    assert_eq!(action(), expected);
    assert_eq!(action(), expected);
    let mut checksum = 0u64;
    let start = Instant::now();
    for _ in 0..epochs {
        checksum = checksum.wrapping_add(black_box(action()));
    }
    let seconds = start.elapsed().as_secs_f64();
    assert_eq!(checksum, expected.wrapping_mul(epochs));
    seconds
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let size: usize = args[1].parse().unwrap();
    let seed: u64 = args[2].parse().unwrap();
    let epochs: u64 = args[3].parse().unwrap();
    assert!(epochs > 0);
    let mut state = seed;
    let mut cases: Vec<Vec<u64>> = (0..64)
        .map(|_| (0..size).map(|_| random(&mut state)).collect())
        .collect();
    for i in (1..cases.len()).rev() {
        let j = random(&mut state) as usize % (i + 1);
        cases.swap(i, j);
    }
    let expected = cases.iter().fold(0u64, |s, c| {
        s.wrapping_add(c.iter().copied().max().unwrap_or(0))
    });
    let mut storage: Vec<Vec<u64>> = cases.iter().map(|_| Vec::with_capacity(size)).collect();
    let pointers: Vec<_> = storage.iter().map(|v| (v.as_ptr(), v.capacity())).collect();
    let mut order = ["std", "std_control", "jzap"];
    for i in (1..order.len()).rev() {
        let j = random(&mut state) as usize % (i + 1);
        order.swap(i, j);
    }
    for label in order {
        for v in &mut storage {
            v.clear();
        }
        let seconds = if label == "jzap" {
            let mut heaps: Vec<_> = storage
                .drain(..)
                .map(|v| {
                    Heap::from_vec(
                        jzap::vec::Vec::try_from_std_in(v, Unmetered, Unlimited).unwrap(),
                    )
                })
                .collect();
            let seconds = elapsed(
                || {
                    let mut sum = 0u64;
                    for (heap, values) in heaps.iter_mut().zip(&cases) {
                        jzap_fill(heap, values);
                        sum = sum.wrapping_add(*heap.peek().unwrap_or(&0));
                    }
                    sum
                },
                epochs,
                expected,
            );
            for (heap, values) in heaps.iter().zip(&cases) {
                let mut reference = Std::new();
                for v in values {
                    reference.push(*v);
                }
                assert_eq!(heap.as_slice(), reference.as_slice());
            }
            storage.extend(heaps.into_iter().map(|h| h.into_vec().into_std()));
            seconds
        } else {
            let mut heaps: Vec<_> = storage.drain(..).map(Std::from).collect();
            let seconds = elapsed(
                || {
                    let mut sum = 0u64;
                    for (heap, values) in heaps.iter_mut().zip(&cases) {
                        std_fill(heap, values);
                        sum = sum.wrapping_add(*heap.peek().unwrap_or(&0));
                    }
                    sum
                },
                epochs,
                expected,
            );
            storage.extend(heaps.into_iter().map(Std::into_vec));
            seconds
        };
        assert_eq!(
            storage
                .iter()
                .map(|v| (v.as_ptr(), v.capacity()))
                .collect::<Vec<_>>(),
            pointers
        );
        println!(
            "{{\"label\":\"{label}\",\"size\":{size},\"seed\":{seed},\"epochs\":{epochs},\"operations\":{},\"elements\":{},\"checksum\":{},\"seconds\":{seconds}}}",
            epochs * 64,
            epochs * 64 * size as u64,
            expected.wrapping_mul(epochs)
        );
    }
}
