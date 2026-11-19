//! Actual writes/normalization; word indices and bounded nonzero ownership are separate.
use super::*;

#[kani::proof]
fn local_cursor_publication_preserves_every_word() {
    let before: usize = kani::any();
    let after: usize = kani::any();
    let mut output = Checked::new(before);
    {
        let mut current = output;
        let progress = Commit {
            output: &mut output,
            current: &mut current,
        };
        *progress.current = Checked::new(after);
        assert!(progress.output.get() == before);
    }
    assert!(output.get() == after);
}

#[kani::proof]
fn local_fill_publishes_every_admitted_word_prefix() {
    struct Source(u8);
    impl TryIterator for Source {
        type Item = ();
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<()>, u8> {
            match self.0 {
                0 => Ok(Some(())),
                1 => Ok(None),
                _ => Err(23),
            }
        }
    }
    let start: usize = kani::any();
    let end: usize = kani::any();
    let mode: u8 = kani::any();
    kani::assume(start <= end && mode < 3);
    let mut state = Checked::new(start);
    // A dangling, aligned pointer supports every numeric extent for a ZST.
    let result = unsafe {
        fill(
            NonNull::<()>::dangling(),
            &mut state,
            end,
            &mut Source(mode),
        )
    };
    assert!(start <= state.get() && state.get() <= end);
    match result {
        Ok(true) => assert!(state.get() == end),
        Ok(false) => assert!(mode == 1 && state.get() < end),
        Err(error) => assert!(error == 23 && mode == 2 && state.get() < end),
    }
}

#[kani::proof]
fn every_word_leaf_offset_and_source_reply() {
    struct Source {
        mode: u8,
        calls: u8,
    }
    impl TryIterator for Source {
        type Item = ();
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<()>, u8> {
            self.calls += 1;
            match self.mode {
                0 => Ok(None),
                1 => Ok(Some(())),
                _ => Err(23),
            }
        }
    }
    let index: usize = kani::any();
    let mode: u8 = kani::any();
    kani::assume(mode < 3);
    let mut source = Source { mode, calls: 0 };
    let mut backing = 0u8;
    let result =
        unsafe { write_next(NonNull::from(&mut backing).cast::<()>(), index, &mut source) };
    assert!(source.calls == 1 && backing == 0);
    let result = finished(result);
    match mode {
        0 => assert!(result == Ok(false)),
        1 => assert!(result == Ok(true)),
        _ => assert!(result == Err(23)),
    }
}

#[kani::proof]
#[kani::unwind(6)]
fn typed_spans_preserve_the_prefix_and_exact_source_consumption() {
    use core::{cell::Cell, mem::MaybeUninit};
    struct Value<'a> {
        id: usize,
        dropped: &'a Cell<u8>,
    }
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            assert!(self.id < 3);
            let bit = 1u8 << self.id;
            assert!(self.dropped.get() & bit == 0);
            self.dropped.set(self.dropped.get() | bit);
        }
    }
    struct Source<'a> {
        index: usize,
        stop: usize,
        error: bool,
        calls: usize,
        dropped: &'a Cell<u8>,
    }
    impl<'a> TryIterator for Source<'a> {
        type Item = Value<'a>;
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<Value<'a>>, u8> {
            self.calls += 1;
            if self.index == self.stop {
                return if self.error { Err(23) } else { Ok(None) };
            }
            let value = Value {
                id: self.index,
                dropped: self.dropped,
            };
            self.index += 1;
            Ok(Some(value))
        }
    }
    let start: usize = kani::any();
    let end: usize = kani::any();
    kani::assume(start <= end && end <= 3);
    let stop: usize = kani::any();
    let error: bool = kani::any();
    let dropped = Cell::new(0);
    let mut storage: [MaybeUninit<Value<'_>>; 3] = [const { MaybeUninit::uninit() }; 3];
    let ptr = NonNull::from(&mut storage).cast::<Value<'_>>();
    for i in 0..start {
        unsafe {
            ptr.as_ptr().add(i).write(Value {
                id: i,
                dropped: &dropped,
            });
        }
    }
    let mut state = Checked::new(start);
    let mut source = Source {
        index: start,
        stop,
        error,
        calls: 0,
        dropped: &dropped,
    };
    let result = unsafe { fill(ptr, &mut state, end, &mut source) };
    let stopped = start <= stop && stop < end;
    let expected = if stopped { stop } else { end };
    assert!(state.get() == expected && dropped.get() == 0);
    if stopped {
        assert!(result == if error { Err(23) } else { Ok(false) });
    } else {
        assert!(result == Ok(true));
    }
    assert!(source.calls == expected - start + usize::from(stopped));
    assert!(source.index == expected);
    for i in 0..expected {
        assert!(unsafe { &*ptr.as_ptr().add(i) }.id == i);
    }
    unsafe {
        core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(ptr.as_ptr(), expected));
    }
    assert!(dropped.get() == (1u8 << expected) - 1);
}
