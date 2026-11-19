//! Actual original filter/chain kernels and word-sized count bounds.
use super::*;
#[kani::proof]
fn algebra_hint_all_machine_word_lengths() {
    let a: usize = kani::any();
    let b: usize = kani::any();
    let (lo, hi) = algebra::combined_hint(a, b);
    assert_eq!(lo, 0);
    assert_eq!(hi, a.checked_add(b));
    let n: usize = kani::any();
    assert_eq!(
        algebra::count_step(n),
        n.checked_add(1).unwrap_or(usize::MAX)
    );
    let allocation_bound = (isize::MAX as usize) - 16;
    if a <= allocation_bound && b <= allocation_bound {
        let total = hi.unwrap();
        assert!(total < usize::MAX);
        if n < total {
            assert_eq!(algebra::count_step(n), n.checked_add(1).unwrap());
        }
    }
}
#[kani::proof]
#[kani::unwind(8)]
fn algebra_filter_and_chain_preserve_progress_results_and_hints() {
    let a: [u8; 3] = kani::any();
    let b: [u8; 3] = kani::any();
    let n = kani::any::<usize>() % 4;
    let m = kani::any::<usize>() % 4;
    let needle: u8 = kani::any();
    let mut expected = [0u8; 3];
    let mut count = 0;
    let mut sum = 0u16;
    for key in &a[..n] {
        if *key == needle {
            expected[count] = *key;
            count += 1;
            sum += u16::from(*key);
        }
    }
    let mut iter = a[..n].iter().copied();
    let mut seen = 0;
    let mut visits = 0;
    loop {
        let previous = iter.len();
        let old_visits = visits;
        let next = algebra::filtered_next(&mut iter, |key| {
            visits += 1;
            *key == needle
        });
        assert_eq!(visits - old_visits, previous - iter.len());
        if let Some(key) = next {
            assert!(iter.len() < previous);
            assert!(seen < count);
            assert_eq!(key, expected[seen]);
            seen += 1;
        } else {
            break;
        }
    }
    assert_eq!(seen, count);
    assert_eq!(iter.len(), 0);
    assert_eq!(algebra::filtered_next(&mut iter, |_| true), None);
    let mut visits = 0;
    let folded = algebra::filtered_fold(
        a[..n].iter().copied(),
        0u16,
        |key| {
            visits += 1;
            *key == needle
        },
        |acc, key| acc + u16::from(key),
    );
    assert_eq!(folded, sum);
    assert_eq!(visits, n);
    let counted = algebra::filtered_fold(
        a[..n].iter().copied(),
        0usize,
        |key| *key == needle,
        |n, _| algebra::count_step(n),
    );
    assert_eq!(counted, count);
    let mut first = a[..n].iter().copied();
    let mut rest = b[..m].iter().copied();
    let mut seen = 0;
    while let Some(key) = algebra::chain_next(&mut first, &mut rest) {
        assert!(seen < n + m);
        assert_eq!(key, if seen < n { a[seen] } else { b[seen - n] });
        seen += 1;
        assert_eq!(first.len() + rest.len(), n + m - seen);
    }
    assert_eq!(seen, n + m);
    assert_eq!(algebra::chain_next(&mut first, &mut rest), None);
    let total = a[..n]
        .iter()
        .chain(b[..m].iter())
        .fold(0u16, |acc, key| acc + u16::from(*key));
    assert_eq!(
        algebra::chain_fold(
            a[..n].iter().copied(),
            b[..m].iter().copied(),
            0u16,
            |acc, key| acc + u16::from(key)
        ),
        total
    );
}
