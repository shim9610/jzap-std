//! Bounded physical std comparison; executed without loop abstraction.
use super::*;

// Separate bounded physical runs compile without jzap_loop_contracts so the
// solver executes the actual loops and compares their contents with std.
#[kani::proof]
#[kani::unwind(14)]
fn retain_error_and_mutation_match_standard_for_every_four_element_input() {
    let n: usize = kani::any();
    let fail_at: usize = kani::any();
    kani::assume(n <= 4 && fail_at <= 4);
    let values: [u8; 4] = kani::any();
    let decisions: [bool; 4] = kani::any();
    let Ok(mut owner) = Vec::<u8>::try_with_capacity(4) else {
        return;
    };
    let Ok(mut reference) = Vec::<u8>::try_with_capacity(4) else {
        return;
    };
    for i in 0..n {
        assert!(owner.try_push(values[i]).is_ok());
        assert!(reference.try_push(values[i]).is_ok());
    }
    let mut reference = reference.into_std();
    let mut ours_calls = 0usize;
    let result = owner.try_retain_mut(|value| {
        let i = ours_calls;
        ours_calls += 1;
        *value = value.wrapping_add(17);
        if i == fail_at {
            Err(*value)
        } else {
            Ok(decisions[i])
        }
    });
    let mut reference_calls = 0usize;
    let mut expected_error = None;
    reference.retain_mut(|value| {
        if expected_error.is_some() {
            return true;
        }
        let i = reference_calls;
        reference_calls += 1;
        *value = value.wrapping_add(17);
        if i == fail_at {
            expected_error = Some(*value);
            true
        } else {
            decisions[i]
        }
    });
    assert_eq!(result.err(), expected_error);
    assert_eq!(ours_calls, reference_calls);
    assert_eq!(owner.as_slice(), reference.as_slice());
}

#[kani::proof]
#[kani::unwind(14)]
fn dedup_error_and_mutation_match_standard_for_every_four_element_input() {
    let n: usize = kani::any();
    let fail_at: usize = kani::any();
    kani::assume(n <= 4 && fail_at <= 4);
    let values: [u8; 4] = kani::any();
    let decisions: [bool; 4] = kani::any();
    let Ok(mut owner) = Vec::<u8>::try_with_capacity(4) else {
        return;
    };
    let Ok(mut reference) = Vec::<u8>::try_with_capacity(4) else {
        return;
    };
    for i in 0..n {
        assert!(owner.try_push(values[i]).is_ok());
        assert!(reference.try_push(values[i]).is_ok());
    }
    let mut reference = reference.into_std();
    let mut ours_calls = 0usize;
    let result = owner.try_dedup_by(|current, previous| {
        let i = ours_calls;
        ours_calls += 1;
        *current = current.wrapping_add(17);
        *previous ^= 0x53;
        if i == fail_at {
            Err(*current ^ *previous)
        } else {
            Ok(decisions[i])
        }
    });
    let mut reference_calls = 0usize;
    let mut expected_error = None;
    reference.dedup_by(|current, previous| {
        if expected_error.is_some() {
            return false;
        }
        let i = reference_calls;
        reference_calls += 1;
        *current = current.wrapping_add(17);
        *previous ^= 0x53;
        if i == fail_at {
            expected_error = Some(*current ^ *previous);
            false
        } else {
            decisions[i]
        }
    });
    assert_eq!(result.err(), expected_error);
    assert_eq!(ours_calls, reference_calls);
    assert_eq!(owner.as_slice(), reference.as_slice());
}
