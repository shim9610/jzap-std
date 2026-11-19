use jzap::{
    Error, TryIterator, WrappingArithmetic,
    boxed::Box as B,
    budget::{Budget, MemoryBudget, Unmetered},
    error::InitError,
    vec::Vec as V,
};

fn wrapped_inputs() -> Vec<usize> {
    let mut values = Vec::new();
    for a in [0usize, 1, usize::MAX / 2, usize::MAX - 1, usize::MAX] {
        for b in [0, 1, 2, 8, usize::MAX] {
            values.extend([a.wrap_add(b), a.wrap_sub(b), a.wrap_mul(b)]);
        }
    }
    values.sort_unstable();
    values.dedup();
    values
}

#[test]
fn wrapped_indices_match_checked_std_access_and_preserve_refused_inputs() {
    for len in 0..=8 {
        for index in wrapped_inputs() {
            let mut value = V::with_limit(8);
            value.try_extend(0u64..len as u64).unwrap();
            let mut expected: Vec<u64> = (0..len as u64).collect();
            let result = value.get_mut(index);
            assert_eq!(result.is_ok(), index < len);
            if let Ok(place) = result {
                *place = 97;
            }
            if let Some(place) = expected.get_mut(index) {
                *place = 97;
            }
            assert_eq!(value.as_slice(), expected);
            let before = expected.clone();
            let result = value.try_insert(index, 101);
            if index <= len && len < 8 {
                assert!(result.is_ok());
                expected.insert(index, 101);
            } else {
                let refused = result.unwrap_err();
                assert_eq!(refused.value, 101);
                assert_eq!(
                    refused.error,
                    if index > len {
                        Error::Bounds
                    } else {
                        Error::Capacity
                    }
                );
                assert_eq!(value.as_slice(), before);
            }
            assert_eq!(value.as_slice(), expected);
            assert!(value.len() <= value.capacity() && value.capacity() <= 8);
        }
    }
}

#[test]
fn wrapped_counts_are_refused_before_layout_or_budget_can_underallocate() {
    for count in wrapped_inputs() {
        let value = V::<u64, _>::try_with_capacity_and_limit_in(count, 8, Unmetered);
        assert_eq!(value.is_ok(), count <= 8);
        drop(value);
        let budget = MemoryBudget::new(64);
        let result =
            B::<[u64], _>::try_init_slice_in(count, Budget::Limited(&budget), |i, slot| {
                assert!(i < 8);
                slot.write(i as u64).map_err(|e| e.error)
            });
        assert_eq!(result.is_ok(), count <= 8);
        if let Ok(value) = &result {
            assert_eq!(value.len(), count);
            for (i, &item) in value.iter().enumerate() {
                assert_eq!(item, i as u64);
            }
        }
        drop(result);
        assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
    }
}

#[test]
fn wrapped_ranges_refuse_before_removal_and_preserve_retained_order() {
    let inputs = wrapped_inputs();
    for &start in &inputs {
        for &end in &inputs {
            let mut value = V::try_from_array([11u64, 17, 23, 31]).unwrap();
            let mut expected = vec![11u64, 17, 23, 31];
            match value.try_drain(start..end) {
                Ok(drain) => {
                    assert!(start <= end && end <= 4);
                    let actual: Vec<_> = drain.collect();
                    let removed: Vec<_> = expected.drain(start..end).collect();
                    assert_eq!(actual, removed);
                }
                Err(error) => {
                    assert_eq!(error, Error::Bounds);
                    assert!(start > end || end > 4);
                }
            }
            assert_eq!(value.as_slice(), expected);
        }
    }
}

#[test]
fn wrapped_hints_and_source_errors_never_escape_reserved_box_spans() {
    struct Source {
        hint: usize,
        next: u8,
        fail: bool,
    }
    impl TryIterator for Source {
        type Item = u64;
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<u64>, u8> {
            if self.next == 2 {
                return if self.fail { Err(7) } else { Ok(None) };
            }
            let value = self.next;
            self.next += 1;
            Ok(Some(value as u64))
        }
        fn try_size_hint(&self) -> Result<(usize, Option<usize>), u8> {
            Ok((self.hint, None))
        }
    }
    for hint in wrapped_inputs() {
        for fail in [false, true] {
            let budget = MemoryBudget::new(256);
            let result = B::<[u64], _>::try_from_fallible_iter_in(
                Source {
                    hint,
                    next: 0,
                    fail,
                },
                Budget::Limited(&budget),
            );
            if let Ok(value) = &result {
                assert!(!fail);
                assert_eq!(value.as_slice(), &[0, 1]);
            }
            if let Err(InitError::Initialize(error)) = &result {
                assert_eq!(*error, 7);
                assert!(fail);
            }
            drop(result);
            assert_eq!((budget.occupied(), budget.reserved()), (0, 0));
        }
    }
}

#[test]
fn an_in_bounds_wrapped_value_is_used_as_a_value_not_rejected_for_its_history() {
    let index = usize::MAX.wrap_add(2);
    assert_eq!(index, 1);
    let mut value = V::try_from_array([11, 17]).unwrap();
    *value.get_mut(index).unwrap() = 97;
    assert_eq!(value.as_slice(), &[11, 97]);
}
