use super::*;
use crate::Checked;
use core::{marker::PhantomData, ptr::NonNull};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Pure edit geometry. Every offset is checked before storage or bytes change.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct Edit {
    pub start: usize,
    pub end: usize,
    pub tail: usize,
    pub destination: usize,
    pub len: usize,
    pub additional: usize,
}
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl Edit {
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(plan) => start <= end <= old
                && plan.start == start && plan.end == end
                && plan.tail as int == old as int - end as int
                && plan.destination as int == start as int + inserted as int
                && plan.len as int == start as int + inserted as int + old as int - end as int
                && plan.len <= limit
                && plan.additional as int == if plan.len > old { plan.len as int - old as int } else { 0 },
            Err(Error::Bounds) => start > end || end > old,
            Err(Error::Capacity) => start <= end <= old
                && start as int + inserted as int + old as int - end as int > limit,
            _ => false,
        },
    ))]
    pub(crate) fn new(
        old: usize,
        start: usize,
        end: usize,
        inserted: usize,
        limit: usize,
    ) -> Result<Self, Error> {
        if start > end || end > old {
            return Err(Error::Bounds);
        }
        let tail = (Checked::new(old) - Checked::new(end))
            .map_err(
                #[cfg_attr(verus_keep_ghost, verus_spec(error: Error => ensures error == Error::Bounds,))]
                |_error| Error::Bounds,
            )?
            .get();
        let destination = (Checked::new(start) + Checked::new(inserted))
            .map_err(
                #[cfg_attr(verus_keep_ghost, verus_spec(error: Error => ensures error == Error::Capacity,))]
                |_error| Error::Capacity,
            )?
            .get();
        let len = crate::vec::required_len(destination, tail, limit)?;
        Ok(Self {
            start,
            end,
            tail,
            destination,
            len,
            additional: len.saturating_sub(old),
        })
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> String<P, L> {
    pub fn try_insert(&mut self, at: usize, ch: char) -> Result<(), Error> {
        self.try_insert_str(at, ch.encode_utf8(&mut [0; 4]))
    }
    pub fn try_insert_str(&mut self, at: usize, text: &str) -> Result<(), Error> {
        self.try_replace_range(at..at, text)
    }
    pub fn try_replace_range(
        &mut self,
        range: impl RangeBounds<usize>,
        text: &str,
    ) -> Result<(), Error> {
        let (start, end) = self.range(range)?;
        let plan = Edit::new(self.len(), start, end, text.len(), self.max_len())?;
        self.bytes.try_reserve(plan.additional)?;
        let ptr = self.bytes.as_mut_ptr();
        // SAFETY: Edit validates every offset and final length, reservation
        // provides final storage. The live tail may overlap its destination.
        // text is disjoint by the exclusive owner borrow. Both splice ends are
        // scalar boundaries and text is valid UTF-8. No callback follows reserve.
        unsafe {
            core::ptr::copy(ptr.add(plan.end), ptr.add(plan.destination), plan.tail);
            core::ptr::copy_nonoverlapping(text.as_ptr(), ptr.add(plan.start), text.len());
            self.bytes.try_set_len(plan.len)
        }
    }
    pub fn remove(&mut self, at: usize) -> Result<char, Error> {
        self.boundary(at)?;
        let ch = self
            .as_str()
            .get(at..)
            .and_then(|s| s.chars().next())
            .ok_or(Error::Bounds)?;
        let end = (Checked::new(at) + Checked::new(ch.len_utf8()))
            .map_err(|_| Error::Bounds)?
            .get();
        self.try_replace_range(at..end, "")?;
        Ok(ch)
    }
    /// Earlier removals persist on callback error; the current/unvisited suffix
    /// survives. A guard restores UTF-8 before unwinding can expose this owner.
    pub fn try_retain<E>(
        &mut self,
        mut keep: impl FnMut(char) -> Result<bool, E>,
    ) -> Result<(), E> {
        let original = self.len();
        let mut guard = Retain {
            owner: self,
            read: Checked::new(0),
            written: Checked::new(0),
            original: Checked::new(original),
        };
        while guard.read.get() < original {
            let Some(bytes) = guard.owner.as_bytes().get(guard.read.get()..) else {
                break;
            };
            // SAFETY: only the prefix before read is modified; read advances by
            // complete scalar widths. The unread suffix remains valid UTF-8.
            let Some(ch) = unsafe { core::str::from_utf8_unchecked(bytes) }
                .chars()
                .next()
            else {
                break;
            };
            let retain = keep(ch)?;
            let width = ch.len_utf8();
            if retain && guard.written != guard.read {
                // SAFETY: scalar lies in initialized unread suffix; written <=
                // read. Copying whole scalars preserves the compacted prefix.
                unsafe {
                    let ptr = guard.owner.bytes.as_mut_ptr();
                    let source = ptr.add(guard.read.get());
                    let destination = ptr.add(guard.written.get());
                    // Fixed scalar widths let LLVM emit short loads/stores.
                    match width {
                        1 => core::ptr::copy(source, destination, 1),
                        2 => core::ptr::copy(source, destination, 2),
                        3 => core::ptr::copy(source, destination, 3),
                        _ => core::ptr::copy(source, destination, 4),
                    }
                }
            }
            if retain {
                guard.written = Checked::new(super::numeric::advance(guard.written.get(), width));
            }
            guard.read = Checked::new(super::numeric::advance(guard.read.get(), width));
        }
        Ok(())
    }
    pub fn retain(&mut self, mut keep: impl FnMut(char) -> bool) {
        let _: Result<(), core::convert::Infallible> = self.try_retain(|ch| Ok(keep(ch)));
    }
    pub fn try_drain(&mut self, range: impl RangeBounds<usize>) -> Result<Drain<'_, P, L>, Error> {
        let (start, end) = self.range(range)?;
        let owner = NonNull::from(&mut *self);
        let chars = self.as_str().get(start..end).ok_or(Error::Bounds)?.chars();
        Ok(Drain {
            owner,
            chars,
            start,
            end,
            marker: PhantomData,
        })
    }
}
struct Retain<'a, P: AllocationPolicy, L: LengthPolicy> {
    owner: &'a mut String<P, L>,
    read: Checked<usize>,
    written: Checked<usize>,
    original: Checked<usize>,
}
impl<P: AllocationPolicy, L: LengthPolicy> Drop for Retain<'_, P, L> {
    fn drop(&mut self) {
        let (remaining, len) =
            super::numeric::restore(self.read.get(), self.written.get(), self.original.get());
        // SAFETY: read/written are scalar boundaries with written <= read <=
        // original. Moving the untouched suffix closes the gap without growth.
        unsafe {
            let ptr = self.owner.bytes.as_mut_ptr();
            core::ptr::copy(
                ptr.add(self.read.get()),
                ptr.add(self.written.get()),
                remaining,
            );
        }
        self.owner.bytes.truncate(len);
    }
}

/// Characters in a selected range; Drop removes the complete range, including
/// unyielded characters. Forgetting the drain leaves the original text intact.
pub struct Drain<'a, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    owner: NonNull<String<P, L>>,
    chars: core::str::Chars<'a>,
    start: usize,
    end: usize,
    marker: PhantomData<&'a mut String<P, L>>,
}
// SAFETY: the lifetime marker exclusively borrows an owner; chars only reads its
// bytes. Moving/sharing the drain requires the owner's policies to allow it.
unsafe impl<P: AllocationPolicy + Send, L: LengthPolicy + Send> Send for Drain<'_, P, L> {}
unsafe impl<P: AllocationPolicy + Sync, L: LengthPolicy + Sync> Sync for Drain<'_, P, L> {}
impl<P: AllocationPolicy, L: LengthPolicy> Drain<'_, P, L> {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.chars.as_str()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> Iterator for Drain<'_, P, L> {
    type Item = char;
    #[inline]
    fn next(&mut self) -> Option<char> {
        self.chars.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chars.size_hint()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> DoubleEndedIterator for Drain<'_, P, L> {
    #[inline]
    fn next_back(&mut self) -> Option<char> {
        self.chars.next_back()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> core::iter::FusedIterator for Drain<'_, P, L> {}
impl<P: AllocationPolicy, L: LengthPolicy> crate::TryIterator for Drain<'_, P, L> {
    type Item = char;
    type Error = core::convert::Infallible;
    fn try_next(&mut self) -> Result<Option<char>, Self::Error> {
        Ok(self.next())
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), Self::Error> {
        Ok(self.size_hint())
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> crate::TryDoubleEndedIterator for Drain<'_, P, L> {
    fn try_next_back(&mut self) -> Result<Option<char>, Self::Error> {
        Ok(self.next_back())
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> crate::TryFusedIterator for Drain<'_, P, L> {}
impl<P: AllocationPolicy, L: LengthPolicy> core::fmt::Debug for Drain<'_, P, L> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Drain").field(&self.as_str()).finish()
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> Drop for Drain<'_, P, L> {
    fn drop(&mut self) {
        // End the immutable borrow before recovering exclusive access to bytes.
        self.chars = "".chars();
        // SAFETY: lifetime marker holds the sole mutable owner borrow; the original character
        // borrow ended above. No mutation has occurred since range validation.
        // The validated range is unchanged throughout the borrow. Delete
        // directly: Drop must neither revalidate text nor attempt allocation.
        unsafe {
            let owner = self.owner.as_mut();
            let (remaining, length) = super::numeric::drain(self.start, self.end, owner.len());
            let ptr = owner.bytes.as_mut_ptr();
            core::ptr::copy(ptr.add(self.end), ptr.add(self.start), remaining);
            owner.bytes.truncate(length);
        }
    }
}
