//! Full machine-word cursor transitions on real editing implementations.
use super::*;

fn units(len: usize) -> Vec<()> {
    // SAFETY: initialized units require no storage or budget charge. The ZST
    // capacity is exactly Unlimited::max_len(), including len == usize::MAX.
    unsafe { Vec::from_raw_parts(NonNull::dangling(), len, usize::MAX, Unmetered, Unlimited) }
}

#[kani::proof]
fn extraction_drop_restores_every_numeric_cursor_exactly() {
    let original: usize = kani::any();
    let read: usize = kani::any();
    let write: usize = kani::any();
    let end: usize = kani::any();
    kani::assume(write <= read && read <= end && end <= original);
    let mut owner = units(0);
    drop(ExtractIf {
        owner: &mut owner,
        filter: (),
        state: Extraction::from_cursor_state(write, read, end, original).unwrap(),
    });
    assert!(owner.len() == original - (read - write));
    assert!(owner.capacity() == usize::MAX);
}

#[kani::proof]
#[kani::unwind(8)]
fn extraction_drop_preserves_nonzero_prefix_and_unvisited_values() {
    let original = kani::any::<usize>();
    let read = kani::any::<usize>();
    let write = kani::any::<usize>();
    let end = kani::any::<usize>();
    kani::assume(write <= read && read <= end && end <= original && original <= 4);
    let values = [11u8, 23, 37, 53];
    let Ok(mut owner) = Vec::try_from_array(values) else {
        return;
    };
    // SAFETY: all four bytes are initialized. The following guard owns their
    // logical retained prefix and unvisited suffix; u8 has no destructor.
    unsafe {
        owner.try_set_len(0).unwrap();
    }
    drop(ExtractIf {
        owner: &mut owner,
        filter: (),
        state: Extraction::from_cursor_state(write, read, end, original).unwrap(),
    });
    assert!(owner.len() == write + (original - read));
    for i in 0..write {
        assert!(owner.as_slice()[i] == values[i]);
    }
    for i in 0..original - read {
        assert!(owner.as_slice()[write + i] == values[read + i]);
    }
}

#[kani::proof]
#[kani::solver(z3)]
fn dedup_all_lengths_with_loop_contracts() {
    let len: usize = kani::any();
    let mut v = units(len);
    let _: Result<(), ()> = v.try_dedup_by(|_, _| {
        if kani::any() {
            Err(())
        } else {
            Ok(kani::any())
        }
    });
    assert!(v.len() <= len);
    assert!(len == 0 || v.len() != 0);
}

#[kani::proof]
#[kani::solver(z3)]
// Bounds verifier-internal write-set bookkeeping; product loops use contracts.
#[kani::unwind(12)]
fn extraction_preserves_every_valid_cursor_state() {
    let original: usize = kani::any();
    let read: usize = kani::any();
    let write: usize = kani::any();
    let end: usize = kani::any();
    kani::assume(write <= read && read <= end && end <= original);
    let mut owner = units(0);
    let mut it = ExtractIf {
        owner: &mut owner,
        filter: |_: &mut ()| -> Result<bool, ()> {
            if kani::any() {
                Err(())
            } else {
                Ok(kani::any())
            }
        },
        state: Extraction::from_cursor_state(write, read, end, original).unwrap(),
    };
    let result = it.next();
    match result {
        None => assert!(it.state.read() == end),
        Some(Ok(())) => assert!(
            read < it.state.read() && it.state.written() < it.state.read() && it.state.end() == end
        ),
        Some(Err(())) => {
            assert!(it.state.read() < end && it.state.end() == it.state.read());
            assert!(it.next().is_none());
        }
    }
    assert!(
        it.state.written() <= it.state.read()
            && it.state.read() <= it.state.end()
            && it.state.end() <= it.state.total()
    );
    assert!(it.state.total() == original);
    assert!(it.size_hint().1 == Some(it.state.end() - it.state.read()));
    drop(it);
    assert!(owner.len.get() <= original);
}

#[kani::proof]
#[kani::solver(z3)]
fn drain_preserves_every_valid_cursor_state() {
    let start: usize = kani::any();
    let front: usize = kani::any();
    let back: usize = kani::any();
    let end: usize = kani::any();
    let original: usize = kani::any();
    kani::assume(start <= front && front <= back && back <= end && end <= original);
    let mut owner = units(start);
    let mut it = Drain {
        owner: &mut owner,
        span: crate::span::Span::try_new(front, back).unwrap(),
        tail_start: end,
        tail_len: original - end,
    };
    let _ = it.next();
    let _ = it.next_back();
    assert!(it.span.front() <= it.span.back() && it.span.back() <= it.tail_start);
    assert!(it.len() == it.span.back() - it.span.front());
    drop(it);
    assert!(owner.len.get() == start + (original - end));
    assert!(owner.len.get() <= original);
}

#[kani::proof]
#[kani::solver(z3)]
fn retain_all_lengths_with_loop_contracts() {
    let len: usize = kani::any();
    let mut owner = units(len);
    let _: Result<(), ()> = owner.try_retain_mut(|_| {
        if kani::any() {
            Err(())
        } else {
            Ok(kani::any())
        }
    });
    assert!(owner.len() <= len);
}

#[kani::proof]
#[kani::unwind(12)]
fn splice_fill_preserves_every_admitted_word_hole_and_tail() {
    struct Source;
    impl crate::TryIterator for Source {
        type Item = ();
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<()>, u8> {
            if kani::any() {
                Err(23)
            } else {
                Ok(if kani::any() { Some(()) } else { None })
            }
        }
    }
    let original: usize = kani::any();
    let start: usize = kani::any();
    let end: usize = kani::any();
    kani::assume(start <= end && end <= original);
    let mut owner = units(start);
    let mut splice = Splice {
        drain: Drain {
            owner: &mut owner,
            span: crate::span::Span::try_new(start, start).unwrap(),
            tail_start: end,
            tail_len: original - end,
        },
        replacement: Source,
    };
    let result = splice.fill();
    let committed = splice.drain.owner.len();
    assert!(start <= committed && committed <= end);
    assert!(splice.drain.tail_start == end && splice.drain.tail_len == original - end);
    match result {
        Ok(true) => assert!(committed == end),
        Ok(false) => assert!(committed < end),
        Err(InitError::Initialize(23)) => assert!(committed < end),
        _ => assert!(false),
    }
    drop(splice);
    assert!(owner.len() == committed + (original - end));
}

// The empty-tail branch of `Splice::try_finish` pushes through the owner and
// never moves `tail_start`. The Drain drop then runs `restore_tail` with the
// length above `tail_start` and an empty tail. Every admitted word length and
// zero, one or two replacement values; the cover shows the state is reached.
#[kani::proof]
#[kani::unwind(5)]
fn empty_tail_splice_publishes_past_the_tail_start() {
    struct Source(u8);
    impl crate::TryIterator for Source {
        type Item = ();
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<()>, u8> {
            if self.0 == 0 {
                return Ok(None);
            }
            self.0 -= 1;
            Ok(Some(()))
        }
    }
    let len: usize = kani::any();
    let count: u8 = kani::any();
    kani::assume(count <= 2);
    let mut owner = units(len);
    let Ok(drain) = owner.try_drain(len..len) else {
        unreachable!()
    };
    let splice = Splice {
        drain,
        replacement: Source(count),
    };
    let (tail_start, tail_len) = (splice.drain.tail_start, splice.drain.tail_len);
    assert!(tail_start == len && tail_len == 0);
    let result = splice.try_finish();
    let published = owner.len.get();
    assert!(owner.cap.get() == usize::MAX);
    assert!(tail_start <= published);
    match result {
        Ok(()) => assert!(published - tail_start == count as usize),
        Err(_) => assert!(published == usize::MAX && published - tail_start < count as usize),
    }
    kani::cover!(result.is_ok() && published > tail_start);
    kani::cover!(result.is_err());
}

// Closure harnesses (issue #16, unit 1 step 5). The owner comes from
// `closure_owner` (only `closure_invariant` assumed); a guard built directly
// assumes only its pair invariant with that owner.
use super::super::query_proofs::{closure_bound, closure_exit, closure_invariant, closure_owner};

/// A callback or source reply chosen freshly on every call.
fn closure_reply<T>(value: T) -> Result<T, u8> {
    if kani::any() { Err(23) } else { Ok(value) }
}
/// A source without a bound: every call ends, fails or yields a value.
struct ClosureSource;
impl TryIterator for ClosureSource {
    type Item = ();
    type Error = u8;
    fn try_next(&mut self) -> Result<Option<()>, u8> {
        closure_reply(if kani::any() { Some(()) } else { None })
    }
    // Any lower bound, or an error: a hint is never trusted.
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), u8> {
        closure_reply((kani::any(), None))
    }
}

fn closure_retain_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    // A callback error returns through `Compact::drop`.
    let _ = owner.try_retain_mut(|_| closure_reply(kani::any::<bool>()));
    closure_exit(&owner, max);
    assert!(owner.len() <= len);
}
#[kani::proof]
#[kani::solver(z3)]
fn closure_retain_preserves_the_invariant() {
    closure_retain_case(LengthLimit::new(kani::any()));
    closure_retain_case(Unlimited);
}

fn closure_dedup_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let _ = owner.try_dedup_by(|_, _| closure_reply(kani::any::<bool>()));
    closure_exit(&owner, max);
    assert!(owner.len() <= len && (len == 0 || owner.len() != 0));
}
#[kani::proof]
#[kani::solver(z3)]
fn closure_dedup_preserves_the_invariant() {
    closure_dedup_case(LengthLimit::new(kani::any()));
    closure_dedup_case(Unlimited);
}

/// The pair a Drain keeps with its owner (docs/premise-proofs.md, step 1):
/// `tail_start + tail_len <= cap && (len <= tail_start || tail_len == 0)`.
fn closure_drain_invariant<T, P: AllocationPolicy, L: LengthPolicy>(
    drain: &Drain<'_, T, P, L>,
) -> bool {
    match drain.tail_start.checked_add(drain.tail_len) {
        Some(end) => {
            end <= drain.owner.cap.get()
                && (drain.owner.len.get() <= drain.tail_start || drain.tail_len == 0)
        }
        None => false,
    }
}
/// An arbitrary Drain over an owner: any ordered unread range and any tail
/// words that satisfy `closure_drain_invariant`, and nothing else.
fn closure_drain<L: LengthPolicy>(
    owner: &mut Vec<(), Unmetered, L>,
) -> Drain<'_, (), Unmetered, L> {
    let span = crate::span::Span::try_new(kani::any(), kani::any());
    kani::assume(span.is_ok());
    let drain = Drain {
        owner,
        span: span.unwrap(),
        tail_start: kani::any(),
        tail_len: kani::any(),
    };
    kani::assume(closure_drain_invariant(&drain));
    drain
}

/// Runs the constructor and returns the count the guard's drop removes.
fn closure_drained<L: LengthPolicy>(owner: &mut Vec<(), Unmetered, L>, len: usize) -> usize {
    match owner.try_drain((closure_bound(), closure_bound())) {
        Ok(drain) => {
            assert!(closure_invariant(drain.owner) && closure_drain_invariant(&drain));
            let start = drain.owner.len.get();
            assert!(start == drain.span.front() && drain.span.back() == drain.tail_start);
            assert!(drain.tail_start + drain.tail_len == len);
            drain.span.len()
        }
        Err(_) => 0,
    }
}
fn closure_drain_constructor_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let removed = closure_drained(&mut owner, len);
    closure_exit(&owner, max);
    assert!(owner.len() == len - removed);
}
#[kani::proof]
fn closure_drain_constructor_establishes_the_pair_invariant() {
    closure_drain_constructor_case(LengthLimit::new(kani::any()));
    closure_drain_constructor_case(Unlimited);
}

fn closure_drain_drop_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let mut drain = closure_drain(&mut owner);
    let tail_len = drain.tail_len;
    let _ = drain.next();
    let _ = drain.next_back();
    assert!(closure_invariant(drain.owner) && closure_drain_invariant(&drain));
    drop(drain);
    closure_exit(&owner, max);
    assert!(owner.len() == len + tail_len);
}
#[kani::proof]
fn closure_drain_drop_restores_the_invariant_from_every_pair_state() {
    closure_drain_drop_case(LengthLimit::new(kani::any()));
    closure_drain_drop_case(Unlimited);
}

#[cfg(jzap_init_loop_contracts)]
fn closure_splice_fill_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let drain = closure_drain(&mut owner);
    let (tail_start, tail_len) = (drain.tail_start, drain.tail_len);
    let mut splice = Splice {
        drain,
        replacement: ClosureSource,
    };
    let result = splice.fill();
    assert!(closure_invariant(splice.drain.owner) && closure_drain_invariant(&splice.drain));
    assert!(splice.drain.tail_start == tail_start && splice.drain.tail_len == tail_len);
    let committed = splice.drain.owner.len.get();
    assert!(len <= committed);
    // The phase fact `move_tail` and `stage_excess` need: a full hole.
    if let Ok(true) = result {
        assert!(committed == tail_start);
    }
    if len > tail_start {
        assert!(matches!(result, Err(InitError::Storage(Error::Bounds))) && committed == len);
    }
    drop(splice);
    closure_exit(&owner, max);
    assert!(owner.len() == committed + tail_len);
}
// The unwind bound is for verifier-internal bookkeeping; the product loop runs
// under its contract.
#[cfg(jzap_init_loop_contracts)]
#[kani::proof]
#[kani::unwind(12)]
fn closure_splice_fill_preserves_the_owner_and_pair_invariants() {
    closure_splice_fill_case(LengthLimit::new(kani::any()));
    closure_splice_fill_case(Unlimited);
}

/// The pair an ExtractIf keeps with its owner: a valid cursor whose total is
/// at most the capacity.
fn closure_extract_invariant<T, F, P: AllocationPolicy, L: LengthPolicy>(
    extract: &ExtractIf<'_, T, F, P, L>,
) -> bool {
    extract.state.valid() && extract.state.total() <= extract.owner.cap.get()
}

fn closure_extract_constructor_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let range = (closure_bound(), closure_bound());
    match owner.try_extract_if(range, |_: &mut ()| closure_reply(kani::any::<bool>())) {
        Ok(extract) => {
            assert!(closure_invariant(extract.owner) && closure_extract_invariant(&extract));
            assert!(extract.owner.len.get() == 0 && extract.state.total() == len);
            // Nothing was visited: the drop restores the whole length.
            drop(extract);
        }
        Err(_) => {}
    }
    assert!(owner.len() == len);
    closure_exit(&owner, max);
}
#[kani::proof]
fn closure_extract_constructor_establishes_the_pair_invariant() {
    closure_extract_constructor_case(LengthLimit::new(kani::any()));
    closure_extract_constructor_case(Unlimited);
}

/// An arbitrary ExtractIf over an owner: any cursor state that
/// `Extraction::from_cursor_state` admits and that satisfies
/// `closure_extract_invariant`, and nothing else.
fn closure_extract<L: LengthPolicy>(
    owner: &mut Vec<(), Unmetered, L>,
) -> ExtractIf<'_, (), (), Unmetered, L> {
    let state = Extraction::from_cursor_state(kani::any(), kani::any(), kani::any(), kani::any());
    kani::assume(state.is_ok());
    let extract = ExtractIf {
        owner,
        filter: (),
        state: state.unwrap(),
    };
    kani::assume(closure_extract_invariant(&extract));
    extract
}

fn closure_extract_drop_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let max = owner.max_len();
    let extract = closure_extract(&mut owner);
    let restored = extract.state.restored_len();
    drop(extract);
    closure_exit(&owner, max);
    assert!(owner.len() == restored);
}
#[kani::proof]
fn closure_extract_drop_restores_the_invariant_from_every_pair_state() {
    closure_extract_drop_case(LengthLimit::new(kani::any()));
    closure_extract_drop_case(Unlimited);
}

#[kani::proof]
fn closure_stage_push_preserves_the_invariant() {
    let mut owner = closure_owner::<(), _, _>(Unmetered, LengthLimit::new(kani::any()));
    let (len, max) = (owner.len(), owner.max_len());
    let result = stage_push(&mut owner, ());
    closure_exit(&owner, max);
    match result {
        Ok(capacity) => assert!(owner.len() == len + 1 && capacity == owner.capacity()),
        Err(_) => assert!(owner.len() == len),
    }
}

#[cfg(jzap_init_loop_contracts)]
#[kani::proof]
#[kani::unwind(12)]
fn closure_stage_batch_preserves_the_invariant() {
    let mut owner = closure_owner::<(), _, _>(Unmetered, LengthLimit::new(kani::any()));
    let (len, max) = (owner.len(), owner.max_len());
    let result = stage_batch(&mut owner, &mut ClosureSource, ());
    closure_exit(&owner, max);
    assert!(len <= owner.len());
    if let Ok(Some(())) = result {
        // The batch is full: the next value needs growth.
        assert!(owner.len() == owner.capacity());
    }
}

// Closure harnesses of the splice (issue #16, unit 1 step 7). `move_tail` and
// `stage_excess` keep the Drain pair invariant only from the phase a completed
// `fill` leaves, `len == tail_start`. They run here from every pair state and
// assert the pair invariant for the states of that phase; nothing is assumed
// about the phase.
fn closure_move_tail_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, cap, max) = (owner.len(), owner.capacity(), owner.max_len());
    let drain = closure_drain(&mut owner);
    let (tail_start, tail_len) = (drain.tail_start, drain.tail_len);
    let mut splice = Splice {
        drain,
        replacement: ClosureSource,
    };
    let additional: usize = kani::any();
    let result = splice.move_tail(additional);
    assert!(closure_invariant(splice.drain.owner));
    assert!(splice.drain.owner.len.get() == len && splice.drain.tail_len == tail_len);
    match result {
        Ok(()) => {
            assert!(splice.drain.tail_start - tail_start == additional);
            // The reservation holds the length, the opened hole and the tail.
            let room = splice.drain.owner.cap.get() - len;
            assert!(room >= tail_len && room - tail_len >= additional);
        }
        Err(_) => {
            assert!(splice.drain.tail_start == tail_start && splice.drain.owner.cap.get() == cap)
        }
    }
    if len == tail_start {
        assert!(closure_drain_invariant(&splice.drain));
    }
    kani::cover!(result.is_ok() && len == tail_start && additional > 0 && tail_len > 0);
    kani::cover!(result.is_err());
    drop(splice);
    closure_exit(&owner, max);
    assert!(owner.len() == len + tail_len);
}
#[kani::proof]
fn closure_move_tail_preserves_the_owner_invariant_and_the_filled_pair() {
    closure_move_tail_case(LengthLimit::new(kani::any()));
    closure_move_tail_case(Unlimited);
}

#[cfg(jzap_init_loop_contracts)]
fn closure_stage_excess_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let drain = closure_drain(&mut owner);
    let (tail_start, tail_len) = (drain.tail_start, drain.tail_len);
    let mut splice = Splice {
        drain,
        replacement: ClosureSource,
    };
    let result = splice.stage_excess((), kani::any());
    assert!(closure_invariant(splice.drain.owner));
    assert!(splice.drain.tail_len == tail_len);
    let (committed, moved) = (splice.drain.owner.len.get(), splice.drain.tail_start);
    match result {
        // The staged values and the tail move by the same count.
        Ok(()) => assert!(committed > len && committed - len == moved - tail_start),
        Err(_) => assert!(committed == len && moved == tail_start),
    }
    if len == tail_start {
        assert!(closure_drain_invariant(&splice.drain) && committed == moved);
    }
    kani::cover!(result.is_ok() && len == tail_start && tail_len > 0);
    kani::cover!(matches!(result, Err(InitError::Initialize(23))));
    kani::cover!(matches!(result, Err(InitError::Storage(Error::Capacity))));
    drop(splice);
    closure_exit(&owner, max);
    assert!(owner.len() == committed + tail_len);
}
// The staging loop takes at most two turns at a zero-sized element: the
// staging owner starts with its whole limit as capacity. The cursor loop runs
// under its contract.
#[cfg(jzap_init_loop_contracts)]
#[kani::proof]
#[kani::unwind(4)]
fn closure_stage_excess_preserves_the_owner_invariant_and_the_filled_pair() {
    closure_stage_excess_case(LengthLimit::new(kani::any()));
}
#[cfg(jzap_init_loop_contracts)]
#[kani::proof]
#[kani::unwind(4)]
fn closure_stage_excess_preserves_the_owner_invariant_and_the_filled_pair_unlimited() {
    closure_stage_excess_case(Unlimited);
}

// The whole `try_finish` with a retained tail, from every pair state: the
// source ends, fails or yields without a bound, and every hint is arbitrary.
// The call consumes the guard, so the exit state is read after its drop.
#[cfg(jzap_init_loop_contracts)]
fn closure_splice_finish_case<L: LengthPolicy>(limit: L) {
    let mut owner = closure_owner::<(), _, _>(Unmetered, limit);
    let (len, max) = (owner.len(), owner.max_len());
    let drain = closure_drain(&mut owner);
    let tail_len = drain.tail_len;
    // Path selection: the empty-tail branch is a separate loop.
    kani::assume(tail_len != 0);
    let result = Splice {
        drain,
        replacement: ClosureSource,
    }
    .try_finish();
    closure_exit(&owner, max);
    // Nothing committed before the call and no tail value is lost.
    assert!(owner.len() >= tail_len && owner.len() - tail_len >= len);
    kani::cover!(result.is_ok() && owner.len() - tail_len > len);
    kani::cover!(matches!(result, Err(InitError::Initialize(23))));
    kani::cover!(matches!(result, Err(InitError::Storage(Error::Capacity))));
}
#[cfg(jzap_init_loop_contracts)]
#[kani::proof]
#[kani::unwind(4)]
fn closure_splice_finish_with_a_tail_preserves_the_invariant() {
    closure_splice_finish_case(LengthLimit::new(kani::any()));
}
#[cfg(jzap_init_loop_contracts)]
#[kani::proof]
#[kani::unwind(4)]
fn closure_splice_finish_with_a_tail_preserves_the_invariant_unlimited() {
    closure_splice_finish_case(Unlimited);
}
