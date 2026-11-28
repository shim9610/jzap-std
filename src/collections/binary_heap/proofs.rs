//! Actual product kernels; no extracted or rewritten function bodies.
use super::*;
#[path = "closure_proofs.rs"]
mod closure;
#[kani::proof]
fn geometry_all_words() {
    let pos: usize = kani::any();
    let end: usize = kani::any();
    // Reversing the append owner order preserves checked addition, and exact
    // growth never exceeds an admissible geometric request.
    if let Some(needed) = pos.checked_add(end) {
        assert_eq!(end.checked_add(pos), Some(needed));
        let cap: usize = kani::any();
        let limit: usize = kani::any();
        if cap < needed && needed <= limit {
            assert!(crate::vec::growth_capacity(cap, needed, limit, 1) >= needed);
        }
    }
    if pos > 0 {
        let p = geometry::parent(pos);
        assert!(p < pos);
        assert_eq!(p, (pos - 1) / 2);
    }
    if pos <= isize::MAX as usize {
        let child = geometry::child_offset(pos);
        assert_eq!(
            Some(child),
            pos.checked_mul(2).and_then(|v| v.checked_add(1))
        );
        assert!(child > pos);
    }
    if pos < end / 2 {
        let (a, b) = geometry::children(pos, end);
        assert!(pos < a && a < end && b <= end);
        assert_eq!(Some(a), pos.checked_mul(2).and_then(|n| n.checked_add(1)));
        assert_eq!(Some(b), a.checked_add(1));
        let choose_right = b < end && kani::any::<bool>();
        let selected = geometry::selected(a, end, choose_right);
        assert!(selected < end);
        assert_eq!(selected, if choose_right { b } else { a });
    }
}
#[kani::proof]
#[kani::unwind(7)]
fn actual_metered_append_accepts_and_preserves_storage_policy() {
    let values: [u8; 4] = kani::any();
    let budget = crate::budget::MemoryBudget::new(8);
    let policy_a = Budget::Limited(&budget);
    let policy_b = Budget::Limited(&budget);
    let mut a = BinaryHeap::try_from_array_in([values[0]], &policy_a).unwrap();
    let mut b =
        BinaryHeap::try_from_array_in([values[1], values[2], values[3]], &policy_b).unwrap();
    a.try_append(&mut b).unwrap();
    assert!(::core::ptr::eq(*a.allocation_policy(), &policy_a));
    assert!(::core::ptr::eq(*b.allocation_policy(), &policy_b));
    is_heap(a.as_slice());
    same_bag(a.as_slice(), &values);
    assert_eq!((a.capacity(), b.capacity()), (4, 1));
    assert!(b.is_empty());
    assert_eq!(budget.occupied(), 5);
    drop((a, b));
    assert_eq!(budget.occupied(), 0);
    assert_eq!(budget.reserved(), 0);
}
#[kani::proof]
#[kani::unwind(7)]
fn actual_metered_storage_reuse_refuses_without_owner_or_budget_change() {
    // Prove the new storage-admission unit directly. The heap caller's `?`
    // propagation is reviewed and checked by native/Miri and a compiled defect
    // control. Existing kernels separately prove heap repair; do not expand its
    // unreachable success path again in this quota-refusal query.
    let values: [u8; 4] = kani::any();
    let budget = crate::budget::MemoryBudget::new(7);
    let policy_a = Budget::Limited(&budget);
    let policy_b = Budget::Limited(&budget);
    let mut a =
        crate::vec::Vec::try_from_array_in([values[0]], &policy_a, crate::vec::Unlimited).unwrap();
    let mut b = crate::vec::Vec::try_from_array_in(
        [values[1], values[2], values[3]],
        &policy_b,
        crate::vec::Unlimited,
    )
    .unwrap();
    let pointers = (a.as_ptr(), b.as_ptr());
    let result = a.try_reuse_larger_buffer(&mut b);
    assert!(matches!(
        result,
        Err(Error::Budget {
            requested: 4,
            available: 3
        })
    ));
    assert!(::core::ptr::eq(*a.allocation_policy(), &policy_a));
    assert!(::core::ptr::eq(*b.allocation_policy(), &policy_b));
    assert_eq!((a.len(), b.len()), (1, 3));
    assert_eq!((a.capacity(), b.capacity()), (1, 3));
    assert!(::core::ptr::eq(a.as_ptr(), pointers.0));
    assert!(::core::ptr::eq(b.as_ptr(), pointers.1));
    assert_eq!(a.as_slice()[0], values[0]);
    for index in 0..3 {
        assert_eq!(b.as_slice()[index], values[index + 1]);
    }
    assert_eq!(budget.occupied(), 4);
    drop((a, b));
    assert_eq!(budget.occupied(), 0);
    assert_eq!(budget.reserved(), 0);
}
fn same_bag(a: &[u8], b: &[u8]) {
    assert_eq!(a.len(), b.len());
    // One universally quantified value proves the whole multiset equality;
    // avoid re-expanding the same count loop once for each symbolic array slot.
    let value: u8 = kani::any();
    assert_eq!(
        a.iter().filter(|v| **v == value).count(),
        b.iter().filter(|v| **v == value).count()
    );
}
fn is_heap(a: &[u8]) {
    for pos in 1..a.len() {
        assert!(a[(pos - 1) / 2] >= a[pos]);
    }
}
#[kani::proof]
#[kani::unwind(7)]
fn actual_sifts_preserve_permutation_order_and_bounds() {
    let values: [u8; 4] = kani::any();
    let n: usize = kani::any();
    // Same 0..=4 prefix domain as any::<usize>() % 5, without encoding a
    // 64-bit remainder circuit in a bounded-array ownership proof.
    kani::assume(n <= values.len());
    let mut data = values;
    core::rebuild(&mut data[..n]);
    is_heap(&data[..n]);
    same_bag(&data[..n], &values[..n]);
    if n > 0 {
        let last = n - 1;
        core::rebuild(&mut data[..last]);
        // SAFETY: newly appended last slot initialized, prefix heapified.
        unsafe { core::sift_up(&mut data[..n], 0, last) };
        is_heap(&data[..n]);
        same_bag(&data[..n], &values[..n]);
        data[0] = kani::any();
        let before = data;
        // SAFETY: only root changed.
        unsafe { core::sift_down(&mut data[..n], 0, n) };
        is_heap(&data[..n]);
        same_bag(&data[..n], &before[..n]);
        data[0] = kani::any();
        let before = data;
        // SAFETY: only root changed.
        unsafe { core::sift_bottom(&mut data[..n], 0, n) };
        is_heap(&data[..n]);
        same_bag(&data[..n], &before[..n]);
    }
}
#[kani::proof]
#[kani::unwind(7)]
fn actual_owner_pop_peek_and_drain_protocols() {
    let values: [u8; 4] = kani::any();
    // Exercise the actual empty-owner allocation/growth path before testing
    // existing-capacity mutation, including the published-length insertion index.
    let mut grown = BinaryHeap::new();
    for value in values {
        grown.try_push(value).unwrap();
    }
    is_heap(grown.as_slice());
    same_bag(grown.as_slice(), &values);
    drop(grown);
    let mut heap = BinaryHeap::try_from_array(values).unwrap();
    let cap = heap.capacity();
    // The existing setter's acceptance follows from valid Vec + unchanged cap.
    let len = heap.len();
    assert!(len <= cap);
    assert!(unsafe { heap.data.try_set_len(1) }.is_ok());
    assert!(unsafe { heap.data.try_set_len(len) }.is_ok());
    let replacement: u8 = kani::any();
    *heap.peek_mut().unwrap() = replacement;
    assert_eq!(heap.len(), 4);
    is_heap(heap.as_slice());
    let mut expected = values;
    let max = *expected.iter().max().unwrap();
    let pos = expected.iter().position(|x| *x == max).unwrap();
    expected[pos] = replacement;
    same_bag(heap.as_slice(), &expected);
    let mut previous = u8::MAX;
    while let Some(value) = heap.pop() {
        assert!(value <= previous);
        previous = value;
    }
    assert_eq!(heap.len(), 0);
    assert_eq!(heap.capacity(), cap);
    let mut heap = BinaryHeap::try_from_array(values).unwrap();
    let original: [u8; 4] = heap.as_slice().try_into().unwrap();
    {
        let mut drain = heap.drain();
        assert_eq!(drain.next(), Some(original[0]));
        assert_eq!(drain.next_back(), Some(original[3]));
        assert_eq!(drain.len(), 2);
    }
    assert!(heap.is_empty());
    heap.try_push(replacement).unwrap();
    assert_eq!(PeekMut::pop(heap.peek_mut().unwrap()), replacement);
    assert!(heap.is_empty());
}
#[kani::proof]
#[kani::unwind(8)]
fn actual_tail_repair_and_sorted_recovery() {
    let values: [u8; 4] = kani::any();
    let start: usize = kani::any();
    kani::assume(start <= values.len());
    let mut data = values;
    core::rebuild(&mut data[..start]);
    core::rebuild_tail(&mut data, start);
    is_heap(&data);
    same_bag(&data, &values);
    let heap = BinaryHeap::try_from_array(values).unwrap();
    let sorted = heap.into_sorted_vec();
    for i in 1..4 {
        assert!(sorted.as_slice()[i - 1] <= sorted.as_slice()[i]);
    }
    same_bag(sorted.as_slice(), &values);
}
