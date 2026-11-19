#[kani::proof]
fn algebra_hint_all_machine_word_lengths() {
    let a: usize = kani::any();
    let b: usize = kani::any();
    let union: bool = kani::any();
    let (lower, upper) = super::algebra::merged_hint(a, b, union);
    let exact_sum = (a as u128) + (b as u128);
    assert_eq!(
        upper,
        if exact_sum <= usize::MAX as u128 {
            Some(exact_sum as usize)
        } else {
            None
        }
    );
    assert_eq!(
        lower,
        if union {
            a.max(b)
        } else if a >= b {
            a - b
        } else {
            b - a
        }
    );
    assert!((lower as u128) <= exact_sum);
    if let Some(upper) = upper {
        assert!(lower <= upper);
    }
}
