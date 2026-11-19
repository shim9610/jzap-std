//! Bounded collection into already admitted exclusive storage.
use crate::{Checked, TryIterator};
use core::ptr::NonNull;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum Stop<E> {
    End,
    Source(E),
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match result {
        Ok(()) => out == Ok(true),
        Err(Stop::End) => out == Ok(false),
        Err(Stop::Source(error)) => out == Err(error),
    },
))]
#[inline]
pub(crate) fn finished<E>(result: Result<(), Stop<E>>) -> Result<bool, E> {
    match result {
        Ok(()) => Ok(true),
        Err(Stop::End) => Ok(false),
        Err(Stop::Source(error)) => Err(error),
    }
}

#[cfg_attr(kani, inline(never))]
#[cfg_attr(not(kani), inline(always))]
unsafe fn write_next<T, I: TryIterator<Item = T>>(
    ptr: NonNull<T>,
    index: usize,
    iter: &mut I,
) -> Result<(), Stop<I::Error>> {
    let value = iter.try_next().map_err(Stop::Source)?.ok_or(Stop::End)?;
    // SAFETY: inherited exclusive, live, unwritten element at the admitted index.
    unsafe { ptr.as_ptr().add(index).write(value) };
    Ok(())
}

// Keep the cursor local while the source cannot observe the borrowed owner.
// Publish the initialized prefix before normal return, source error, or unwind
// reaches the caller's owner/tail cleanup.
struct Commit<'a> {
    output: &'a mut Checked<usize>,
    current: &'a mut Checked<usize>,
}
impl Drop for Commit<'_> {
    #[inline]
    fn drop(&mut self) {
        *self.output = *self.current;
    }
}

/// # Safety
/// The caller owns valid storage through `end` and its initialized prefix is
/// exactly `state`. Require `state.get() <= end`. Storage must remain live and
/// exclusively borrowed throughout this call. Every completed step transfers
/// one source value to the caller's prefix; early end and source errors preserve
/// the completed prefix. The source remains borrowed in the caller's scope.
#[inline]
pub(crate) unsafe fn fill<T, I: TryIterator<Item = T>>(
    ptr: NonNull<T>,
    state: &mut Checked<usize>,
    end: usize,
    iter: &mut I,
) -> Result<bool, I::Error> {
    let mut current = *state;
    let progress = Commit {
        output: state,
        current: &mut current,
    };
    // SAFETY: the shared cursor admits only the unwritten indices below end and
    // advances after write_next transfers a valid source value to that slot.
    finished(super::driver::run(progress.current, end, |index| unsafe {
        write_next(ptr, index, iter)
    }))
}

#[cfg(kani)]
#[path = "collection_proofs.rs"]
mod proofs;
