//! Bounded physical comparison of the actual iterator and std's retained order.
use super::*;
#[kani::proof]
#[kani::unwind(5)]
fn extraction_errors_mutation_and_order_match_standard_for_every_three_element_input() {
    let n: usize = kani::any();
    let start: usize = kani::any();
    let end: usize = kani::any();
    let fail_at: usize = kani::any();
    kani::assume(start <= end && end <= n && n <= 3 && fail_at <= 3);
    let values: [u8; 3] = kani::any();
    let replies: [bool; 3] = kani::any();
    let Ok(mut owner) = Vec::<u8>::try_with_capacity(3) else {
        return;
    };
    let Ok(mut reference) = Vec::<u8>::try_with_capacity(3) else {
        return;
    };
    let mut output = [(false, 0u8); 3];
    let mut expected = [(false, 0u8); 3];
    let mut output_len = 0;
    let mut expected_len = 0;
    for i in 0..n {
        assert!(owner.try_push(values[i]).is_ok());
        assert!(reference.try_push(values[i]).is_ok());
    }
    let mut reference = reference.into_std();
    let mut calls = 0usize;
    let mut iter = owner
        .try_extract_if(start..end, |v| {
            let i = calls;
            calls += 1;
            *v = v.wrapping_add(17);
            if i == fail_at {
                Err(*v)
            } else {
                Ok(replies[i])
            }
        })
        .unwrap();
    for _ in 0..4 {
        let Some(reply) = iter.next() else { break };
        match reply {
            Ok(v) => {
                output[output_len] = (true, v);
                output_len += 1;
            }
            Err(error) => {
                output[output_len] = (false, error);
                output_len += 1;
                assert!(iter.next().is_none());
                break;
            }
        }
    }
    assert!(iter.next().is_none());
    drop(iter);
    let mut reference_calls = 0usize;
    let mut error = None;
    let mut iter = reference.extract_if(start..end, |v| {
        if error.is_some() {
            return false;
        }
        let i = reference_calls;
        reference_calls += 1;
        *v = v.wrapping_add(17);
        if i == fail_at {
            error = Some(*v);
            false
        } else {
            replies[i]
        }
    });
    for _ in 0..4 {
        let Some(v) = iter.next() else { break };
        expected[expected_len] = (true, v);
        expected_len += 1;
    }
    assert!(iter.next().is_none());
    drop(iter);
    if let Some(error) = error {
        expected[expected_len] = (false, error);
        expected_len += 1;
    }
    assert_eq!(output_len, expected_len);
    for i in 0..output_len {
        assert_eq!(output[i].0, expected[i].0);
        assert_eq!(output[i].1, expected[i].1);
    }
    assert_eq!(calls, reference_calls);
    assert_eq!(owner.as_slice(), reference.as_slice());
}
