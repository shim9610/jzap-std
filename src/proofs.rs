#[kani::proof]
fn budget_arithmetic_and_raii() {
    let limit: usize = kani::any();
    let a: usize = kani::any();
    let b: usize = kani::any();
    let budget = crate::budget::MemoryBudget::new(limit);
    let first = budget.reserve(a);
    assert!((first.is_ok()) == (a <= limit));
    if let Ok(first) = first {
        let second = budget.reserve(b);
        assert!((second.is_ok()) == (b <= limit - a));
        drop(second);
        let charge = first.commit();
        assert!((budget.occupied()) == (a));
        drop(charge);
    }
    assert!((budget.occupied()) == (0));
    assert!((budget.reserved()) == (0));
    assert!((budget.available()) == (limit));
}
#[kani::proof]
fn copy_mismatch_preserves_destination() {
    let source: [u8; 2] = kani::any();
    let mut out: [u8; 3] = kani::any();
    let old = out;
    assert!(crate::clone::try_copy_slice(&source, &mut out).is_err());
    assert!((out) == (old));
}
#[kani::proof]
#[kani::unwind(5)]
fn scalar_array_clone_preserves_values() {
    use crate::TryClone;
    let source: [u8; 3] = kani::any();
    let result = source.try_clone();
    assert!(result.is_ok());
    if let Ok(copy) = result {
        assert!((source) == (copy));
    }
}

#[kani::proof]
fn checked_array_layout() {
    let n: usize = kani::any();
    let r = crate::layout::array::<u64>(n);
    assert!((r.is_ok()) == (n <= (isize::MAX as usize) / 8));
}

#[cfg(feature = "alloc")]
#[kani::proof]
fn slot_write_read_reject_and_cleanup() {
    use core::{cell::Cell, mem::MaybeUninit, ptr::NonNull};
    struct Value<'a> {
        payload: u64,
        drops: &'a Cell<usize>,
    }
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    let a: u64 = kani::any();
    let b: u64 = kani::any();
    let drops = Cell::new(0usize);
    let mut storage = MaybeUninit::<Value<'_>>::uninit();
    // SAFETY: local storage is aligned, uninitialized and exclusively borrowed.
    let mut slot = unsafe { crate::init::InitSlot::new(NonNull::from(&mut storage).cast()) };
    assert!(slot.get().is_none());
    assert!(
        slot.write(Value {
            payload: a,
            drops: &drops
        })
        .is_ok()
    );
    if let Some(value) = slot.get() {
        assert!(value.payload == a);
    } else {
        assert!(false);
    }
    let rejection = slot.write(Value {
        payload: b,
        drops: &drops,
    });
    assert!(rejection.is_err());
    if let Err(rejected) = rejection {
        assert!(rejected.value.payload == b);
        drop(rejected);
    }
    assert!(drops.get() == 1);
    drop(slot);
    assert!(drops.get() == 2);
}

#[cfg(feature = "alloc")]
#[kani::proof]
fn slot_finish_requires_write_and_transfers_ownership() {
    use core::{mem::MaybeUninit, ptr::NonNull};
    let write: bool = kani::any();
    let value: u64 = kani::any();
    let mut storage = MaybeUninit::<u64>::uninit();
    // SAFETY: unique local storage lives beyond the guard.
    let mut slot = unsafe { crate::init::InitSlot::new(NonNull::from(&mut storage).cast()) };
    if write {
        assert!(slot.write(value).is_ok());
    }
    let result = slot.finish();
    assert!(result.is_ok() == write);
    if write {
        // SAFETY: successful finish means the slot initialized this storage.
        assert!(unsafe { storage.assume_init() } == value);
    }
}

#[cfg(feature = "alloc")]
#[kani::proof]
fn heap_slot_initialization_without_union_model() {
    use core::{alloc::Layout, ptr::NonNull};
    let layout = Layout::new::<u64>();
    // SAFETY: exact nonzero layout; null is handled before constructing a slot.
    let pointer = unsafe { alloc::alloc::alloc(layout) };
    let Some(pointer) = NonNull::new(pointer.cast::<u64>()) else {
        return;
    };
    let initialize: bool = kani::any();
    let value: u64 = kani::any();
    // SAFETY: fresh heap allocation, no initialized value, exclusive ownership.
    let mut slot = unsafe { crate::init::InitSlot::new(pointer) };
    assert!(slot.get().is_none());
    if initialize {
        assert!(slot.write(value).is_ok());
        assert!(slot.write(value).is_err());
        if let Some(observed) = slot.get() {
            assert!(*observed == value);
        } else {
            assert!(false);
        }
    }
    let result = slot.finish();
    assert!(result.is_ok() == initialize);
    if initialize {
        // SAFETY: finish transferred the initialized value to this harness.
        assert!(unsafe { pointer.as_ptr().read() } == value);
    }
    // SAFETY: no owning guard/value remains; allocation and layout match.
    unsafe {
        alloc::alloc::dealloc(pointer.as_ptr().cast(), layout);
    }
}

#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(6)]
fn heap_array_slot_checks_every_partial_position() {
    use core::{alloc::Layout, ptr::NonNull};
    let layout = Layout::new::<[u8; 4]>();
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    let Some(ptr) = NonNull::new(ptr.cast::<[u8; 4]>()) else {
        return;
    };
    let fail_at: usize = kani::any();
    kani::assume(fail_at <= 4);
    let fail_after_write: bool = kani::any();
    // SAFETY: exact exclusive uninitialized allocation.
    let mut slot = unsafe { crate::init::InitSlot::new(ptr) };
    let result = slot.try_init_array(|i, element| {
        if i == fail_at && !fail_after_write {
            return Err(crate::Error::Capacity);
        }
        element.write(i as u8).map_err(|e| e.error)?;
        if i == fail_at {
            return Err(crate::Error::Capacity);
        }
        Ok(())
    });
    assert!(result.is_ok() == (fail_at == 4));
    assert!(slot.get().is_some() == (fail_at == 4));
    if let Some(value) = slot.get() {
        assert!(*value == [0, 1, 2, 3]);
    }
    drop(slot);
    // SAFETY: guard released all element ownership; deallocate its storage once.
    unsafe {
        alloc::alloc::dealloc(ptr.as_ptr().cast(), layout);
    }
}

#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(8)]
fn dynamic_slice_initialization_and_budget_cleanup() {
    use crate::{
        Error,
        boxed::Box,
        budget::{Budget, MemoryBudget},
    };
    let fail: usize = kani::any();
    kani::assume(fail <= 3);
    let after: bool = kani::any();
    let limit: usize = kani::any();
    let budget = MemoryBudget::new(limit);
    let result = Box::<[u8], _>::try_init_slice_in(3, Budget::Limited(&budget), |i, slot| {
        if i == fail && !after {
            return Err(Error::Capacity);
        }
        let write = slot.write(i as u8);
        assert!(write.is_ok());
        if i == fail {
            return Err(Error::Capacity);
        }
        Ok(())
    });
    if let Ok(value) = result {
        assert!(fail == 3);
        assert!(limit >= 3);
        assert!(value.len() == 3);
        assert!(budget.occupied() == 3);
        for (i, byte) in value.iter().enumerate() {
            assert!(*byte == i as u8);
        }
        drop(value);
    }
    assert!(budget.occupied() == 0);
    assert!(budget.reserved() == 0);
    assert!(budget.available() == limit);
}
#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(5)]
fn collected_slice_reallocation_preserves_values() {
    use crate::{
        boxed::Box,
        budget::{Budget, MemoryBudget},
    };
    struct NoHint {
        position: usize,
        end: usize,
    }
    impl Iterator for NoHint {
        type Item = u8;
        fn next(&mut self) -> Option<u8> {
            if self.position >= self.end {
                return None;
            }
            let result = self.position as u8;
            self.position += 1;
            Some(result)
        }
    }
    // Exhaust the original 0..=3 domain explicitly. Constant lengths let CBMC
    // simplify allocation sizes before encoding the realloc memory model.
    for n in 0usize..=3 {
        let budget = MemoryBudget::new(16);
        let result = Box::<[u8], _>::try_from_iter_in(
            NoHint {
                position: 0,
                end: n,
            },
            Budget::Limited(&budget),
        );
        if let Ok(value) = result {
            assert!(value.len() == n);
            assert!(budget.occupied() == n);
            for (i, byte) in value.iter().enumerate() {
                assert!(*byte == i as u8);
            }
            drop(value);
        }
        assert!(budget.occupied() == 0);
        assert!(budget.reserved() == 0);
    }
}
#[cfg(feature = "alloc")]
#[kani::proof]
fn struct_fields_require_every_write() {
    use crate::{Error, boxed::Box, budget::Unmetered, init::InitSlot};
    struct Pair {
        a: u8,
        b: u8,
    }
    let write: bool = kani::any();
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let result = Box::<Pair>::try_init_in(Unmetered, |slot: &mut InitSlot<'_, Pair>| {
        crate::try_init_struct!(
            slot,
            Pair {
                a: |field: &mut InitSlot<'_, u8>| field.write(a).map_err(|e| e.error),
                b: |field: &mut InitSlot<'_, u8>| {
                    if write {
                        field.write(b).map_err(|e| e.error)?;
                    }
                    Ok::<_, Error>(())
                },
            }
        )
    });
    if let Ok(value) = result {
        assert!(write);
        assert!(value.a == a);
        assert!(value.b == b);
    }
}

#[kani::proof]
#[kani::unwind(5)]
fn nested_array_reuse_preserves_values_and_mismatch_destination() {
    use crate::TryClone;
    let source: [[u8; 2]; 2] = kani::any();
    let mut destination: [[u8; 2]; 2] = kani::any();
    let result = <[u8; 2]>::try_copy_slice_from(&source, &mut destination);
    assert!(result == Some(Ok(())));
    assert!(source == destination);
    let old = destination;
    let shorter = [source[0]];
    assert!(
        <[u8; 2]>::try_copy_slice_from(&shorter, &mut destination)
            == Some(Err(crate::Error::Bounds))
    );
    assert!(destination == old);
}

#[cfg(feature = "alloc")]
#[kani::proof]
fn vec_required_length_checks_all_overflow_and_limit_cases() {
    let len: usize = kani::any();
    let additional: usize = kani::any();
    let limit: usize = kani::any();
    let r = crate::vec::required_len(len, additional, limit);
    let valid = len <= limit && additional <= limit.saturating_sub(len);
    assert!(r.is_ok() == valid);
    if let Ok(n) = r {
        assert!(n <= limit);
        assert!(n >= len);
        assert!(n - len == additional);
    } else {
        assert!(r == Err(crate::Error::Capacity));
    }
}
#[cfg(feature = "alloc")]
#[kani::proof]
fn vec_growth_proposal_fits_limit_without_rejecting_fitting_length() {
    let current: usize = kani::any();
    let needed: usize = kani::any();
    let ceiling: usize = kani::any();
    let minimum: usize = kani::any();
    kani::assume(needed <= ceiling);
    let n = crate::vec::growth_capacity(current, needed, ceiling, minimum);
    assert!(needed <= n && n <= ceiling);
}
#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(8)]
fn vec_bounded_push_pop_preserves_values_and_budget() {
    use crate::{
        budget::{Budget, MemoryBudget},
        vec::Vec,
    };
    let limit: usize = kani::any();
    kani::assume(limit <= 3);
    let budget = MemoryBudget::new(32);
    let mut v = Vec::<u8, _>::with_limit_in(limit, Budget::Limited(&budget));
    let value: u8 = kani::any();
    for _ in 0..4 {
        let old = v.len();
        let r = v.try_push(value);
        if r.is_ok() {
            assert!(old < limit);
            assert!(v.len() == old + 1);
        } else {
            assert!(v.len() == old);
        }
        assert!(v.len() <= v.capacity() && v.capacity() <= limit);
    }
    while let Some(x) = v.pop() {
        assert!(x == value);
    }
    drop(v);
    assert!(budget.occupied() == 0);
    assert!(budget.reserved() == 0);
}
#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(8)]
fn vec_insert_remove_and_iterator_transfer() {
    use crate::vec::Vec;
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let mut v = Vec::with_limit(2);
    if v.try_push(a).is_err() {
        return;
    }
    if v.try_insert(0, b).is_err() {
        return;
    }
    assert!(v.len() == 2);
    if let Ok(x) = v.remove(0) {
        assert!(x == b);
    } else {
        assert!(false);
    }
    let mut i = v.into_iter();
    assert!(i.next_back() == Some(a));
    assert!(i.next().is_none());
}

#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(5)]
fn vec_drain_preserves_retained_order() {
    use crate::vec::Vec;
    let input: [u8; 3] = kani::any();
    let Ok(mut v) = Vec::try_from_array(input) else {
        return;
    };
    let start: usize = kani::any();
    let end: usize = kani::any();
    kani::assume(start <= end && end <= 3);
    let mut drain = match v.try_drain(start..end) {
        Ok(d) => d,
        Err(_) => {
            assert!(false);
            return;
        }
    };
    if start < end {
        assert!(drain.next() == Some(input[start]));
    }
    drop(drain);
    assert!(v.len() == 3 - (end - start));
    for i in 0..v.len() {
        assert!(v.as_slice()[i] == input[if i < start { i } else { i + (end - start) }]);
    }
}
#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(8)]
fn vec_retain_error_restores_suffix() {
    use crate::vec::Vec;
    let input: [u8; 3] = kani::any();
    let Ok(mut v) = Vec::try_from_array(input) else {
        return;
    };
    let fail: usize = kani::any();
    kani::assume(fail < 3);
    let mut calls = 0;
    let result = v.try_retain_mut(|_| {
        let i = calls;
        calls += 1;
        if i == fail { Err(()) } else { Ok(false) }
    });
    assert!(result.is_err());
    assert!(v.len() == 3 - fail);
    for i in 0..v.len() {
        assert!(v.as_slice()[i] == input[fail + i]);
    }
}
#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(8)]
fn vec_splice_checks_limit_and_keeps_tail() {
    use crate::{
        budget::Unmetered,
        vec::{LengthLimit, Vec},
    };
    let input: [u8; 3] = kani::any();
    let replacement: u8 = kani::any();
    let Ok(mut v) = Vec::try_from_array_in(input, Unmetered, LengthLimit::new(3)) else {
        return;
    };
    let splice = match v.try_splice(1..2, [replacement, replacement]) {
        Ok(s) => s,
        Err(_) => {
            assert!(false);
            return;
        }
    };
    let result = splice.finish();
    assert!(result == Err(crate::Error::Capacity));
    assert!(v.as_slice() == [input[0], replacement, input[2]]);
}

#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(5)]
fn vec_extract_preserves_retained_order() {
    use crate::vec::Vec;
    let input: [u8; 3] = kani::any();
    let Ok(mut v) = Vec::try_from_array(input) else {
        return;
    };
    let key: u8 = kani::any();
    let mut extracted = match v.try_extract_if(.., |x| Ok::<_, ()>(*x == key)) {
        Ok(e) => e,
        Err(_) => {
            assert!(false);
            return;
        }
    };
    while let Some(x) = extracted.next() {
        assert!(x == Ok(key));
    }
    drop(extracted);
    let mut kept = 0;
    for x in input {
        if x != key {
            assert!(v.as_slice()[kept] == x);
            kept += 1;
        }
    }
    assert!(v.len() == kept);
}

#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(11)]
fn vec_extract_long_prefix_preserves_unvisited_ownership() {
    use crate::vec::Vec;
    // Only the first next() can traverse the untouched prefix. Isolate it from
    // repeated compaction calls while covering two batches plus a remainder.
    let input: [u8; 9] = kani::any();
    let Ok(mut v) = Vec::try_from_array(input) else {
        return;
    };
    let key: u8 = kani::any();
    let mut first = input.len();
    for (i, x) in input.iter().enumerate() {
        if *x == key {
            first = i;
            break;
        }
    }
    let mut extracted = match v.try_extract_if(.., |x| Ok::<_, ()>(*x == key)) {
        Ok(e) => e,
        Err(_) => {
            assert!(false);
            return;
        }
    };
    let item = extracted.next();
    assert!(
        item == if first < input.len() {
            Some(Ok(key))
        } else {
            None
        }
    );
    let unvisited = if first < input.len() {
        input.len() - first - 1
    } else {
        0
    };
    assert!(extracted.size_hint() == (0, Some(unvisited)));
    drop(extracted);
    let mut kept = 0;
    for (i, x) in input.iter().enumerate() {
        if i != first {
            assert!(v.as_slice()[kept] == *x);
            kept += 1;
        }
    }
    assert!(v.len() == kept);
}

#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(8)]
fn vec_unhinted_splice_restores_tail_and_budget() {
    use crate::{
        budget::{Budget, MemoryBudget},
        vec::{LengthLimit, Vec},
    };
    let input: [u8; 2] = kani::any();
    let replacements: [u8; 2] = kani::any();
    let budget = MemoryBudget::new(8);
    let Ok(mut v) = Vec::try_from_array_in(input, Budget::Limited(&budget), LengthLimit::new(4))
    else {
        return;
    };
    let mut iter = replacements.into_iter();
    let edit = match v.try_splice(1..1, core::iter::from_fn(|| iter.next())) {
        Ok(s) => s,
        Err(_) => {
            assert!(false);
            return;
        }
    };
    if edit.finish().is_ok() {
        assert!(v.as_slice() == [input[0], replacements[0], replacements[1], input[1]]);
    } else {
        assert!(v.as_slice() == input);
    }
    drop(v);
    assert!(budget.occupied() == 0);
    assert!(budget.reserved() == 0);
}
