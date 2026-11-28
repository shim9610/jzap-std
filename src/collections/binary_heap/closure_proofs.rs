//! Closure harnesses of the BinaryHeap wrapper (issue #16, unit 7; docs/premise-proofs.md).
//!
//! BinaryHeap has one word, `data: Vec<T, P, L>`; its invariant is the Vec
//! invariant of that field (scripts/owner_closure.py, `WRAPPERS`). Each harness
//! takes its heap from `closure_heap`, whose only assumption is the Vec
//! invariant read through the reviewed getters, runs actual heap methods at a
//! zero-sized element (every length is a heap of `()`) and asserts the Vec
//! invariant and the heap's guard state on every return.
use super::*;
use crate::vec::LengthLimit;
use ::core::{mem, ptr::NonNull};

/// An arbitrary heap of zero-sized elements under the Vec invariant
/// `len <= cap <= limit`, read through the reviewed one-expression getters.
fn closure_heap<P: AllocationPolicy, L: LengthPolicy>(policy: P, limit: L) -> BinaryHeap<(), P, L> {
    // SAFETY: zero-sized elements need no storage; the words are restricted below.
    let owner = unsafe {
        Vec::from_raw_parts(NonNull::dangling(), kani::any(), kani::any(), policy, limit)
    };
    kani::assume(owner.len() <= owner.capacity() && owner.capacity() <= owner.max_len());
    BinaryHeap { data: owner }
}

/// The exit check: the Vec invariant of the heap's field, under the same limit.
fn closure_exit<P: AllocationPolicy, L: LengthPolicy>(heap: &BinaryHeap<(), P, L>, limit: usize) {
    assert!(heap.len() <= heap.capacity() && heap.capacity() <= heap.max_len());
    assert!(heap.max_len() == limit);
}

fn closure_valid<T, P: AllocationPolicy, L: LengthPolicy>(heap: &BinaryHeap<T, P, L>) -> bool {
    heap.len() <= heap.capacity() && heap.capacity() <= heap.max_len()
}

/// The six struct literals of BinaryHeap: `new`, `new_in`, `try_with_capacity`,
/// `try_with_capacity_in`, `from_vec` (through `try_from_array`) and `try_clone`.
#[kani::proof]
fn closure_constructors_establish_the_invariant() {
    let heap = BinaryHeap::<()>::new();
    assert!(closure_valid(&heap) && heap.is_empty());
    let heap = BinaryHeap::<(), _>::new_in(Unmetered);
    assert!(closure_valid(&heap) && heap.is_empty());
    let heap = <BinaryHeap<()> as Default>::default();
    assert!(closure_valid(&heap) && heap.is_empty());
    let heap = <BinaryHeap<()> as crate::TryDefault>::try_default();
    assert!(matches!(heap, Ok(ref h) if closure_valid(h) && h.is_empty()));
    let requested: usize = kani::any();
    if let Ok(heap) = BinaryHeap::<()>::try_with_capacity(requested) {
        assert!(closure_valid(&heap) && heap.is_empty() && heap.capacity() >= requested);
    }
    if let Ok(heap) = BinaryHeap::<(), _>::try_with_capacity_in(requested, Unmetered) {
        assert!(closure_valid(&heap) && heap.is_empty() && heap.capacity() >= requested);
    }
    let heap = BinaryHeap::try_from_array([(); 3]);
    assert!(matches!(heap, Ok(ref h) if closure_valid(h) && h.len() == 3));
    let heap = BinaryHeap::try_from_array_in([(); 3], Unmetered);
    assert!(matches!(heap, Ok(ref h) if closure_valid(h) && h.len() == 3));
    // The clone of an arbitrary heap under the Vec invariant.
    let source = closure_heap(Unmetered, Unlimited);
    let copy = source.try_clone();
    if let Ok(ref copy) = copy {
        assert!(closure_valid(copy) && copy.len() == source.len());
    }
    kani::cover!(copy.is_ok() && source.len() > 0);
    closure_exit(&source, usize::MAX);
    let data = source.into_vec();
    assert!(data.len() <= data.capacity() && data.capacity() <= data.max_len());
}

fn closure_push_pop_case<L: LengthPolicy>(limit: L) {
    let mut heap = closure_heap(Unmetered, limit);
    let max = heap.max_len();
    let len = heap.len();
    let result = heap.try_push(());
    closure_exit(&heap, max);
    assert!(heap.len() == if result.is_ok() { len + 1 } else { len });
    kani::cover!(result.is_ok() && len > 0);
    kani::cover!(result.is_err());
    let len = heap.len();
    let popped = heap.pop();
    closure_exit(&heap, max);
    assert!(popped.is_some() == (len > 0));
    assert!(heap.len() == if len > 0 { len - 1 } else { 0 });
    kani::cover!(popped.is_some() && len > 2);
}
#[kani::proof]
#[kani::unwind(70)]
fn closure_push_pop_preserve_the_invariant() {
    closure_push_pop_case(LengthLimit::new(kani::any()));
    closure_push_pop_case(Unlimited);
}

fn closure_peek_mut_case<L: LengthPolicy>(limit: L) {
    let mut heap = closure_heap(Unmetered, limit);
    let (max, len, cap) = (heap.max_len(), heap.len(), heap.capacity());
    let touch: bool = kani::any();
    let end: u8 = kani::any();
    {
        let Some(mut guard) = heap.peek_mut() else {
            assert!(len == 0);
            closure_exit(&heap, max);
            return;
        };
        assert!(len > 0 && guard.original_len.is_none() && guard.heap.len() == len);
        if touch {
            let _root: &mut () = &mut guard;
            // The pair state of the guard: the tail is hidden behind length 1
            // and the original length is restorable in the unchanged capacity.
            match guard.original_len {
                Some(original) => {
                    assert!(len > 1 && original.get() == len && guard.heap.len() == 1);
                    assert!(
                        original.get() <= guard.heap.capacity() && guard.heap.capacity() == cap
                    );
                }
                None => assert!(len == 1 && guard.heap.len() == 1),
            }
            assert!(closure_valid(&*guard.heap));
        }
        match end {
            0 => drop(guard),
            1 => {
                let _value: () = PeekMut::pop(guard);
                closure_exit(&heap, max);
                assert!(heap.len() == len - 1 && heap.capacity() == cap);
                kani::cover!(touch && len > 2);
                return;
            }
            _ => {
                mem::forget(guard);
                closure_exit(&heap, max);
                // Forgetting a mutated guard keeps only the root (documented leak).
                assert!(heap.len() == if touch { 1 } else { len });
                return;
            }
        }
    }
    closure_exit(&heap, max);
    assert!(heap.len() == len && heap.capacity() == cap);
    kani::cover!(touch && len > 2);
}
#[kani::proof]
#[kani::unwind(70)]
fn closure_peek_mut_restores_the_length() {
    closure_peek_mut_case(LengthLimit::new(kani::any()));
    closure_peek_mut_case(Unlimited);
}

fn closure_drain_case<L: LengthPolicy>(limit: L) {
    let mut heap = closure_heap(Unmetered, limit);
    let (max, len, cap) = (heap.max_len(), heap.len(), heap.capacity());
    let forget: bool = kani::any();
    {
        let mut drain = heap.drain();
        assert!(drain.len() == len);
        let mut taken = 0usize;
        if kani::any() {
            if drain.next().is_some() {
                taken += 1;
            }
        }
        if kani::any() {
            if drain.next_back().is_some() {
                taken += 1;
            }
        }
        assert!(drain.len() == len - taken);
        if forget {
            mem::forget(drain);
        }
    }
    closure_exit(&heap, max);
    assert!(heap.is_empty() && heap.capacity() == cap);
    kani::cover!(len > 2);
}
#[kani::proof]
fn closure_drain_retires_the_length() {
    closure_drain_case(LengthLimit::new(kani::any()));
    closure_drain_case(Unlimited);
}
