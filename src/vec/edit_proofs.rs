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
