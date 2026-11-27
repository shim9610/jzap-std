//! Cursor induction step on the real owned iterator, for every machine word.
use super::*;

#[kani::proof]
#[kani::solver(z3)]
fn vec_iterator_preserves_every_valid_cursor_state() {
    let front: usize = kani::any();
    let back: usize = kani::any();
    kani::assume(front <= back);
    // SAFETY: an initialized unit range requires no storage or charge. These
    // are exactly the states reachable by consuming a unit Vec of length back.
    let owner =
        unsafe { Vec::from_raw_parts(NonNull::dangling(), 0, usize::MAX, Unmetered, Unlimited) };
    let mut it = IntoIter::<(), Unmetered, Unlimited> {
        owner,
        span: crate::span::Span::try_new(front, back).unwrap(),
    };
    let _ = it.as_slice();
    let _ = it.as_mut_slice();
    let _ = it.next();
    let _ = it.next_back();
    assert!(
        front <= it.span.front() && it.span.front() <= it.span.back() && it.span.back() <= back
    );
    assert!(it.len() == it.span.back() - it.span.front());
    assert!(it.owner.len.get() == 0);
    drop(it);
}

// Closure harness (issue #16, unit 1 step 5): the owned iterator takes an
// arbitrary owner that satisfies only `closure_invariant`.
fn closure_into_iter_case<L: LengthPolicy>(limit: L) {
    let owner = super::super::query_proofs::closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let it = owner.into_iter();
    super::super::query_proofs::closure_exit(&it.owner, max);
    assert!(it.owner.len.get() == 0);
    // The pair the iterator keeps with its owner: the unread range lies in the capacity.
    assert!(it.span.front() == 0 && it.span.back() == len);
    assert!(closure_iter_invariant(&it));
}
#[kani::proof]
fn closure_into_iter_preserves_the_invariant() {
    closure_into_iter_case(LengthLimit::new(kani::any()));
    closure_into_iter_case(Unlimited);
}

// Closure harnesses of the owned iterator (issue #16, unit 1 step 7).
use super::super::query_proofs::{closure_exit, closure_invariant, closure_owner};

/// The pair an IntoIter keeps with its owner: an ordered unread range inside
/// the capacity, and an empty published length.
fn closure_iter_invariant<T, P: AllocationPolicy, L: LengthPolicy>(it: &IntoIter<T, P, L>) -> bool {
    it.span.front() <= it.span.back()
        && it.span.back() <= it.owner.cap.get()
        && it.owner.len.get() == 0
}
/// An arbitrary iterator: an owner from `closure_owner` and any unread range
/// that satisfy `closure_iter_invariant`, and nothing else.
fn closure_iter<L: LengthPolicy>(limit: L) -> IntoIter<(), Unmetered, L> {
    let span = crate::span::Span::try_new(kani::any(), kani::any());
    kani::assume(span.is_ok());
    let it = IntoIter {
        owner: closure_owner::<(), _, _>(Unmetered, limit),
        span: span.unwrap(),
    };
    kani::assume(closure_iter_invariant(&it));
    it
}
fn closure_iter_exit<L: LengthPolicy>(it: &IntoIter<(), Unmetered, L>, limit: usize) {
    assert!(closure_iter_invariant(it));
    closure_exit(&it.owner, limit);
}

fn closure_iter_steps_case<L: LengthPolicy>(limit: L) {
    let mut it = closure_iter(limit);
    let (max, cap) = (it.owner.max_len(), it.owner.cap.get());
    let (front, back) = (it.span.front(), it.span.back());
    let remaining = back - front;
    assert!(it.as_slice().len() == remaining && it.as_mut_slice().len() == remaining);
    assert!(it.as_ref().len() == remaining && it.as_mut().len() == remaining);
    assert!(it.size_hint() == (remaining, Some(remaining)) && it.len() == remaining);
    closure_iter_exit(&it, max);
    let first = it.next();
    closure_iter_exit(&it, max);
    let last = it.next_back();
    closure_iter_exit(&it, max);
    assert!(front <= it.span.front() && it.span.back() <= back);
    assert!(remaining - it.len() == first.is_some() as usize + last.is_some() as usize);
    assert!(it.owner.cap.get() == cap);
    drop(it);
}
#[kani::proof]
fn closure_iter_steps_preserve_the_pair_invariant() {
    closure_iter_steps_case(LengthLimit::new(kani::any()));
    closure_iter_steps_case(Unlimited);
}

fn closure_iter_clone_case<L: LengthPolicy>(limit: L) {
    let it = closure_iter(limit);
    let max = it.owner.max_len();
    let remaining = it.len();
    let copy = it.try_clone();
    closure_iter_exit(&it, max);
    assert!(it.len() == remaining);
    // The unread range is within the capacity, so within the limit: no refusal.
    let Ok(copy) = copy else { unreachable!() };
    closure_iter_exit(&copy, max);
    assert!(copy.span.front() == 0 && copy.span.back() == remaining);
}
#[kani::proof]
fn closure_iter_clone_establishes_the_pair_invariant_of_the_copy() {
    closure_iter_clone_case(LengthLimit::new(kani::any()));
    closure_iter_clone_case(Unlimited);
}

// The whole-value replacement of `IntoIter::try_clone_from`: the replacement is
// built under the receiver's own limit and exchanged with it.
fn closure_iter_clone_from_case<L: LengthPolicy>(limit: L, source_limit: L) -> Result<(), Error> {
    let mut it = closure_iter(limit);
    let source = closure_iter(source_limit);
    let (max, source_max) = (it.owner.max_len(), source.owner.max_len());
    let (front, back) = (it.span.front(), it.span.back());
    let remaining = source.len();
    let result = it.try_clone_from(&source);
    closure_iter_exit(&it, max);
    closure_iter_exit(&source, source_max);
    assert!(source.len() == remaining);
    match result {
        Ok(()) => assert!(it.span.front() == 0 && it.span.back() == remaining),
        Err(_) => assert!(it.span.front() == front && it.span.back() == back),
    }
    kani::cover!(result.is_ok() && remaining > 0);
    result
}
#[kani::proof]
fn closure_iter_clone_from_replaces_the_whole_value_under_the_pair_invariant() {
    let limited =
        closure_iter_clone_from_case(LengthLimit::new(kani::any()), LengthLimit::new(kani::any()));
    // The source may hold more than the receiver's limit admits.
    kani::cover!(limited == Err(Error::Capacity));
    let _ = closure_iter_clone_from_case(Unlimited, Unlimited);
}

// What the two whole-value replacements do (`mem::swap(self, &mut replacement)`
// in `Vec::try_clone_from` and `IntoIter::try_clone_from`): two values that
// each hold their invariant exchange every word. The product drops the
// replaced value inside the method, where no harness can read it; this harness
// runs no product statement and states the exchange on the two types only.
#[kani::proof]
fn closure_whole_value_exchange_keeps_both_invariants() {
    let mut owner = closure_owner::<(), _, _>(Unmetered, LengthLimit::new(kani::any()));
    let mut other = closure_owner::<(), _, _>(Unmetered, LengthLimit::new(kani::any()));
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let (other_len, other_cap, other_max) = (other.len(), other.capacity(), other.max_len());
    mem::swap(&mut owner, &mut other);
    closure_exit(&owner, other_max);
    closure_exit(&other, max);
    assert!(owner.len() == other_len && owner.capacity() == other_cap);
    assert!(other.len() == len && other.capacity() == cap);
    let mut it = closure_iter(LengthLimit::new(kani::any()));
    let mut second = closure_iter(LengthLimit::new(kani::any()));
    let (it_max, second_max) = (it.owner.max_len(), second.owner.max_len());
    let (it_len, second_len) = (it.len(), second.len());
    mem::swap(&mut it, &mut second);
    closure_iter_exit(&it, second_max);
    closure_iter_exit(&second, it_max);
    assert!(it.len() == second_len && second.len() == it_len);
    assert!(closure_invariant(&it.owner) && closure_invariant(&second.owner));
}

#[kani::proof]
fn closure_iter_default_establishes_the_pair_invariant() {
    let it = IntoIter::<()>::default();
    closure_iter_exit(&it, usize::MAX);
    assert!(it.len() == 0);
}
