//! In-place edits. Every hole is covered by a guard before user code can run.
use super::*;
use crate::compaction::{Compaction, Extraction};
use crate::{TryIntoIterator, TryIterator};
use core::ops::RangeBounds;
#[cfg(all(kani, jzap_loop_contracts))]
#[path = "edit_proofs.rs"]
mod panic_proofs;

#[cfg(kani)]
#[path = "compaction_proofs.rs"]
mod compaction_proofs;

#[cfg(kani)]
#[path = "extraction_proofs.rs"]
mod extraction_proofs;

#[cfg(kani)]
#[path = "staging_proofs.rs"]
mod staging_proofs;

pub(crate) fn checked_range(
    range: impl RangeBounds<usize>,
    len: usize,
) -> Result<(usize, usize), Error> {
    // Preserve callback order: a rejected lower endpoint never queries the upper.
    let start = super::bounds::lower(range.start_bound())?;
    let end = super::bounds::upper(range.end_bound(), len)?;
    crate::slice::geometry::range(start, end, len)
}

// Prefix [0,write) and suffix [read,original) own all live values. The gap
// contains no ownership. Used for error returns and destructor/callback unwind.
struct Compact<'a, T, P: AllocationPolicy, L: LengthPolicy> {
    owner: &'a mut Vec<T, P, L>,
    state: Compaction,
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for Compact<'_, T, P, L> {
    #[inline]
    fn drop(&mut self) {
        if self.state.read() == self.state.written() {
            return;
        }
        let remaining = self.state.remaining();
        // SAFETY: live suffix moves left into the gap; overlap is allowed.
        unsafe {
            core::ptr::copy(
                self.owner.ptr.as_ptr().add(self.state.read()),
                self.owner.ptr.as_ptr().add(self.state.written()),
                remaining,
            );
        }
        // The owner still has its original length; the restored length is
        // at most that, and the transition never lengthens.
        self.owner.extent.shorten_to(self.state.restored_len());
    }
}

// The owner's length lent to an initialization cursor. Whatever the cursor
// holds when the borrower returns or unwinds is published, clamped to the
// capacity, so the borrower cannot leave a length the capacity does not hold.
struct LentLength<'a, T, L: LengthPolicy> {
    extent: &'a mut crate::extent::Extent<T, L>,
    cursor: Checked<usize>,
}
impl<'a, T, L: LengthPolicy> LentLength<'a, T, L> {
    #[inline(always)]
    fn new(extent: &'a mut crate::extent::Extent<T, L>) -> Self {
        let cursor = extent.length();
        Self { extent, cursor }
    }
}
impl<T, L: LengthPolicy> Drop for LentLength<'_, T, L> {
    #[inline(always)]
    fn drop(&mut self) {
        self.extent.set_len_clamped(self.cursor.get());
    }
}

impl<T, P: AllocationPolicy, L: LengthPolicy> Vec<T, P, L> {
    /// Stable compaction. On callback error, earlier removals stay removed and
    /// the current/unvisited elements are retained. Callback mutations persist.
    pub fn try_retain_mut<E>(
        &mut self,
        mut keep: impl FnMut(&mut T) -> Result<bool, E>,
    ) -> Result<(), E> {
        let original = self.extent.len();
        let mut g = Compact {
            owner: self,
            state: Compaction::full(original),
        };
        let pointer = g.owner.ptr;
        super::compaction_driver::run::<false, E, _, _>(
            &mut g.state,
            original,
            |read, _write| {
                // SAFETY: the driver admits one initialized unread element.
                keep(unsafe { &mut *pointer.as_ptr().add(read) })
            },
            |read, write, retain| {
                // SAFETY: state advances before ownership transfer. A retained value
                // moves into an earlier hole; a removed value is dropped once.
                unsafe {
                    if retain {
                        core::ptr::copy_nonoverlapping(
                            pointer.as_ptr().add(read),
                            pointer.as_ptr().add(write),
                            1,
                        );
                    } else {
                        core::ptr::drop_in_place(pointer.as_ptr().add(read));
                    }
                }
            },
        )?;
        g.owner.extent.shorten_to(g.state.written());
        mem::forget(g);
        Ok(())
    }
    pub fn try_retain<E>(&mut self, mut keep: impl FnMut(&T) -> Result<bool, E>) -> Result<(), E> {
        self.try_retain_mut(move |v| keep(v))
    }
    pub fn retain_mut(&mut self, mut keep: impl FnMut(&mut T) -> bool) {
        let _: Result<(), core::convert::Infallible> = self.try_retain_mut(move |v| Ok(keep(v)));
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        self.retain_mut(move |v| keep(v));
    }

    /// Compare (later, previous retained), in std's argument order. Error keeps
    /// the current/unvisited suffix and any mutations to compared values.
    pub fn try_dedup_by<E>(
        &mut self,
        mut same: impl FnMut(&mut T, &mut T) -> Result<bool, E>,
    ) -> Result<(), E> {
        let original = self.extent.len();
        if original < 2 {
            return Ok(());
        }
        let mut g = Compact {
            owner: self,
            state: Compaction::first_kept(original),
        };
        let pointer = g.owner.ptr;
        super::compaction_driver::run::<false, E, _, _>(
            &mut g.state,
            original,
            |read, write| {
                let previous = crate::arithmetic::bounded::wrapping_sub(write, 1);
                // SAFETY: first-kept and the driver preserve write >= 1 and previous < read.
                same(unsafe { &mut *pointer.as_ptr().add(read) }, unsafe {
                    &mut *pointer.as_ptr().add(previous)
                })
                .map(|duplicate| !duplicate)
            },
            |read, write, retain| {
                // SAFETY: state advances before ownership transfer. A retained value
                // moves into an earlier hole; a removed value is dropped once.
                unsafe {
                    if retain {
                        core::ptr::copy_nonoverlapping(
                            pointer.as_ptr().add(read),
                            pointer.as_ptr().add(write),
                            1,
                        );
                    } else {
                        core::ptr::drop_in_place(pointer.as_ptr().add(read));
                    }
                }
            },
        )?;
        g.owner.extent.shorten_to(g.state.written());
        mem::forget(g);
        Ok(())
    }
    pub fn dedup_by(&mut self, mut same: impl FnMut(&mut T, &mut T) -> bool) {
        let _: Result<(), core::convert::Infallible> = self.try_dedup_by(|a, b| Ok(same(a, b)));
    }
    pub fn try_dedup_by_key<K: PartialEq, E>(
        &mut self,
        mut key: impl FnMut(&mut T) -> Result<K, E>,
    ) -> Result<(), E> {
        self.try_dedup_by(|a, b| Ok(key(a)? == key(b)?))
    }
    pub fn dedup_by_key<K: PartialEq>(&mut self, mut key: impl FnMut(&mut T) -> K) {
        self.dedup_by(|a, b| key(a) == key(b));
    }
    pub fn dedup(&mut self)
    where
        T: PartialEq,
    {
        self.dedup_by(|a, b| a == b);
    }

    /// Validate before changing length. Dropping the iterator removes remaining
    /// selected values and closes the gap, without allocating or advancing an iterator.
    /// Forgetting it can leak the selected values and tail, like std::Drain.
    pub fn try_drain(
        &mut self,
        range: impl RangeBounds<usize>,
    ) -> Result<Drain<'_, T, P, L>, Error> {
        let (start, end) = checked_range(range, self.extent.len())?;
        let span = crate::span::Span::try_new(start, end)?;
        let tail_len = (self.extent.length() - Checked::new(end))
            .map_err(|_| Error::Bounds)?
            .get();
        self.extent.shorten_to(start);
        Ok(Drain {
            owner: self,
            span,
            tail_start: end,
            tail_len,
        })
    }
    /// Lazy fallible extraction. An error is yielded once, then iteration ends;
    /// the failing and unvisited values are retained. Early Drop does not invoke
    /// the predicate again. Forgetting the iterator may leak retained values.
    #[inline]
    pub fn try_extract_if<E, F: FnMut(&mut T) -> Result<bool, E>>(
        &mut self,
        range: impl RangeBounds<usize>,
        filter: F,
    ) -> Result<ExtractIf<'_, T, F, P, L>, Error> {
        let (start, end) = checked_range(range, self.extent.len())?;
        let state = Extraction::try_range(start, end, self.extent.len())?;
        self.extent.shorten_to(0);
        Ok(ExtractIf {
            owner: self,
            filter,
            state,
        })
    }
    /// Removed items are lazy, replacement is explicit: call `finish` to insert.
    /// Drop alone removes the range and discards the unconsumed replacement.
    pub fn try_splice<I: IntoIterator<Item = T>>(
        &mut self,
        range: impl RangeBounds<usize>,
        replacement: I,
    ) -> Result<SpliceIn<'_, crate::iter::FromStd<I::IntoIter>, P, L>, Error> {
        // Convert before creating holes: an IntoIterator panic must not start removal.
        let replacement = crate::iter::from_std(replacement);
        Ok(Splice {
            drain: self.try_drain(range)?,
            replacement,
        })
    }
    /// Fallible conversion happens before removal. Source errors during insertion
    /// are returned by `Splice::try_finish`; Drop never requests replacement items.
    pub fn try_splice_fallible<I: TryIntoIterator<Item = T>>(
        &mut self,
        range: impl RangeBounds<usize>,
        replacement: I,
    ) -> Result<SpliceIn<'_, I::IntoIter, P, L>, InitError<I::Error>> {
        let replacement = replacement.try_into_iter().map_err(InitError::Initialize)?;
        Ok(Splice {
            drain: self.try_drain(range)?,
            replacement,
        })
    }
}

pub struct Drain<'a, T, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    owner: &'a mut Vec<T, P, L>,
    span: crate::span::Span,
    tail_start: usize,
    tail_len: usize,
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Drain<'_, T, P, L> {
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: front..back is precisely the not-yet-yielded range.
        unsafe {
            core::slice::from_raw_parts(
                self.owner.ptr.as_ptr().add(self.span.front()),
                self.span.len(),
            )
        }
    }
    fn clear_remaining(&mut self) {
        let start = self.span.front();
        let count = self.span.len();
        self.span.clear();
        if !mem::needs_drop::<T>() {
            return;
        }
        // SAFETY: ownership removed before user destructors; slice drop cleans
        // the rest on unwind, and Drain's tail guard restores the vector.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.owner.ptr.as_ptr().add(start),
                count,
            ));
        }
    }
    fn restore_tail(&mut self) {
        // SAFETY: gap before the retained tail is uninitialized, overlap allowed.
        unsafe {
            core::ptr::copy(
                self.owner.ptr.as_ptr().add(self.tail_start),
                self.owner.ptr.as_ptr().add(self.owner.extent.len()),
                self.tail_len,
            );
        }
        // A drop cannot return a refusal: the transition clamps to the capacity.
        self.owner
            .extent
            .set_len_clamped(crate::arithmetic::bounded::wrapping_add(
                self.owner.extent.len(),
                self.tail_len,
            ));
        self.tail_len = 0;
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Iterator for Drain<'_, T, P, L> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        let i = self.span.next()?;
        // SAFETY: span removes exactly one initialized index before transfer.
        Some(unsafe { self.owner.ptr.as_ptr().add(i).read() })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.span.len();
        (n, Some(n))
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> DoubleEndedIterator for Drain<'_, T, P, L> {
    fn next_back(&mut self) -> Option<T> {
        let i = self.span.next_back()?;
        // SAFETY: span removes exactly one initialized index before transfer.
        Some(unsafe { self.owner.ptr.as_ptr().add(i).read() })
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> ExactSizeIterator for Drain<'_, T, P, L> {
    #[inline]
    fn len(&self) -> usize {
        self.size_hint().0
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> core::iter::FusedIterator for Drain<'_, T, P, L> {}
impl<T, P: AllocationPolicy, L: LengthPolicy> AsRef<[T]> for Drain<'_, T, P, L> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}
impl<T: core::fmt::Debug, P: AllocationPolicy, L: LengthPolicy> core::fmt::Debug
    for Drain<'_, T, P, L>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Drain").field(&self.as_slice()).finish()
    }
}
impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for Drain<'_, T, P, L> {
    #[inline]
    fn drop(&mut self) {
        struct Tail<'a, 'b, T, P: AllocationPolicy, L: LengthPolicy>(&'a mut Drain<'b, T, P, L>);
        impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for Tail<'_, '_, T, P, L> {
            #[inline]
            fn drop(&mut self) {
                self.0.restore_tail();
            }
        }
        let guard = Tail(self);
        guard.0.clear_remaining();
    }
}

pub struct ExtractIf<'a, T, F, P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    owner: &'a mut Vec<T, P, L>,
    filter: F,
    state: Extraction,
}
impl<T, F, P: AllocationPolicy, L: LengthPolicy> ExtractIf<'_, T, F, P, L> {
    #[cold]
    fn fail<E>(&mut self, error: E) -> Option<Result<T, E>> {
        self.state.stop();
        Some(Err(error))
    }
}
impl<T, E, F: FnMut(&mut T) -> Result<bool, E>, P: AllocationPolicy, L: LengthPolicy> Iterator
    for ExtractIf<'_, T, F, P, L>
{
    type Item = Result<T, E>;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let pointer = self.owner.ptr;
        let filter = &mut self.filter;
        let result = super::compaction_driver::extract_next::<E, _, _>(
            &mut self.state,
            |read| {
                // SAFETY: admitted initialized unread element.
                filter(unsafe { &mut *pointer.as_ptr().add(read) })
            },
            |read, write| {
                // SAFETY: the driver admits an initialized element and earlier disjoint hole.
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        pointer.as_ptr().add(read),
                        pointer.as_ptr().add(write),
                        1,
                    );
                }
            },
        );
        match result {
            Ok(Some(read)) => Some(Ok(unsafe { core::ptr::read(pointer.as_ptr().add(read)) })),
            Ok(None) => None,
            Err(error) => self.fail(error),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.state.remaining()))
    }
}
impl<T, E, F: FnMut(&mut T) -> Result<bool, E>, P: AllocationPolicy, L: LengthPolicy>
    core::iter::FusedIterator for ExtractIf<'_, T, F, P, L>
{
}
impl<T, F, P: AllocationPolicy, L: LengthPolicy> Drop for ExtractIf<'_, T, F, P, L> {
    #[inline]
    fn drop(&mut self) {
        if self.state.written() != self.state.read() {
            // SAFETY: write <= read <= original; compact the unvisited suffix
            // into the holes without dropping either moved ownership twice.
            unsafe {
                core::ptr::copy(
                    self.owner.ptr.as_ptr().add(self.state.read()),
                    self.owner.ptr.as_ptr().add(self.state.written()),
                    self.state.unvisited_len(),
                );
            }
        }
        // A drop cannot return a refusal: the transition clamps to the capacity.
        self.owner.extent.set_len_clamped(self.state.restored_len());
    }
}

#[must_use = "call finish to insert replacement values and receive allocation failures"]
pub struct Splice<
    'a,
    T,
    I: TryIterator<Item = T>,
    P: AllocationPolicy = Unmetered,
    L: LengthPolicy = Unlimited,
> {
    drain: Drain<'a, T, P, L>,
    replacement: I,
}
/// A splice whose element type is the replacement iterator's item type.
pub type SpliceIn<'a, I, P = Unmetered, L = Unlimited> =
    Splice<'a, <I as TryIterator>::Item, I, P, L>;
impl<T, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy> Splice<'_, T, I, P, L> {
    fn move_tail(&mut self, additional: usize) -> Result<(), Error> {
        let required = required_len(self.drain.tail_len, additional, usize::MAX)?;
        let next_tail = (Checked::new(self.drain.tail_start) + Checked::new(additional))
            .map_err(|_| Error::Capacity)?;
        self.drain.owner.try_reserve(required)?;
        // SAFETY: original tail plus additional fits reserved storage. Move right,
        // allowing overlap; no references into the old storage survive reservation.
        unsafe {
            core::ptr::copy(
                self.drain.owner.ptr.as_ptr().add(self.drain.tail_start),
                self.drain.owner.ptr.as_ptr().add(next_tail.get()),
                self.drain.tail_len,
            );
        }
        self.drain.tail_start = next_tail.get();
        Ok(())
    }
    #[inline]
    fn fill(&mut self) -> Result<bool, InitError<I::Error>> {
        let start = self.drain.owner.extent.length();
        let _ = (Checked::new(self.drain.tail_start) - start).map_err(|_| Error::Bounds)?;
        let pointer = self.drain.owner.ptr;
        let mut length = LentLength::new(&mut self.drain.owner.extent);
        // SAFETY: the checked hole is uniquely owned and lies before the retained tail.
        unsafe {
            crate::init::collection::fill(
                pointer,
                &mut length.cursor,
                self.drain.tail_start,
                &mut self.replacement,
            )
        }
        .map_err(InitError::Initialize)
    }
    /// Complete replacement explicitly. Failure keeps inserted values and the
    /// original retained tail; consumed removed/replacement items are not restored.
    /// Iterator hints never cause a false capacity rejection. Unknown-length
    /// excess is staged once; each reserved batch moves the retained tail once.
    #[inline]
    pub fn try_finish(mut self) -> Result<(), InitError<I::Error>> {
        let initial_capacity = self.drain.owner.extent.cap();
        self.drain.clear_remaining();
        if self.drain.tail_len == 0 {
            // Each push is committed; guard still owns the (empty) retained tail.
            while let Some(value) = self.replacement.try_next().map_err(InitError::Initialize)? {
                self.drain.owner.try_push(value).map_err(|e| e.error)?;
            }
            return Ok(());
        }
        if !self.fill()? {
            return Ok(());
        }
        // An actual yielded value, not a hint, establishes that growth is needed.
        let Some(first) = self.replacement.try_next().map_err(InitError::Initialize)? else {
            return Ok(());
        };
        let available = ((Checked::new(self.drain.owner.max_len())
            - self.drain.owner.extent.length())
        .map_err(|_| Error::Capacity)?
            - Checked::new(self.drain.tail_len))
        .map_err(|_| Error::Capacity)?
        .get();
        if available == 0 {
            return Err(InitError::Storage(Error::Capacity));
        }
        // Like std, reserve a hinted batch to move the retained tail once for
        // exact-length iterators. Clamp untrusted hints to the logical ceiling.
        let lower = self
            .replacement
            .try_size_hint()
            .map_err(InitError::Initialize)?
            .0;
        let additional = if lower == 0 {
            // One actual item justifies this reservation. Fill the spare space
            // created by that first growth before allocating any staging buffer.
            let reserve = (Checked::new(self.drain.tail_len) + Checked::new(1))
                .map_err(|_| Error::Capacity)?;
            self.drain.owner.try_reserve(reserve.get())?;
            ((self.drain.owner.extent.capacity() - self.drain.owner.extent.length())
                .map_err(|_| Error::Capacity)?
                - Checked::new(self.drain.tail_len))
            .map_err(|_| Error::Capacity)?
            .get()
        } else {
            lower.saturating_add(1).min(available)
        };
        self.move_tail(additional)?;
        let next =
            (self.drain.owner.extent.length() + Checked::new(1)).map_err(|_| Error::Capacity)?;
        // The length is admitted before the value moves; a refusal drops it
        // like the refusals above. Nothing can unwind before the write.
        let index = self.drain.owner.extent.len();
        self.drain.owner.extent.set_len_within(next.get())?;
        // SAFETY: move_tail created at least one uninitialized slot, which
        // the admitted length now holds.
        unsafe {
            self.drain.owner.ptr.as_ptr().add(index).write(first);
        }
        if !self.fill()? {
            return Ok(());
        }
        let Some(first) = self.replacement.try_next().map_err(InitError::Initialize)? else {
            return Ok(());
        };
        self.stage_excess(first, initial_capacity)
    }
    // Keep unknown-length collection out of the common exact-length path.
    #[inline]
    fn stage_excess(
        &mut self,
        first: T,
        initial_capacity: usize,
    ) -> Result<(), InitError<I::Error>> {
        // Stage only excess beyond the drained hole. The staging limit accounts
        // for the original tail, even while it is outside owner.len.
        let available = ((Checked::new(self.drain.owner.max_len())
            - self.drain.owner.extent.length())
        .map_err(|_| Error::Capacity)?
            - Checked::new(self.drain.tail_len))
        .map_err(|_| Error::Capacity)?
        .get();
        let mut extra = Vec::<T, P>::with_limit_in(available, self.drain.owner.policy.try_clone()?);
        let (staged, _, _) = super::staging_driver::run(
            first,
            available,
            mem::size_of::<T>(),
            extra.capacity(),
            |_index, _cap, value| {
                let reply = stage_batch(&mut extra, &mut self.replacement, value);
                (reply, extra.extent.len(), extra.capacity())
            },
        );
        if !staged? {
            return Err(InitError::Storage(Error::Capacity));
        }
        let total = required_len(
            self.drain.owner.extent.len(),
            self.drain.tail_len,
            self.drain.owner.max_len(),
        )?;
        let needed = required_len(total, extra.extent.len(), self.drain.owner.max_len())?;
        // Reserve with the hidden tail included, without pretending holes are initialized.
        if needed > self.drain.owner.extent.cap() {
            self.drain.owner.grow(
                (Checked::new(needed) - self.drain.owner.extent.length())
                    .map_err(|_| Error::Capacity)?
                    .get(),
                self.drain.owner.extent.cap() != initial_capacity,
            )?;
        }
        let next_tail = (Checked::new(self.drain.tail_start) + extra.extent.length())
            .map_err(|_| Error::Capacity)?;
        let next_len = (self.drain.owner.extent.length() + extra.extent.length())
            .map_err(|_| Error::Capacity)?;
        // Admitted before the moves: a refusal leaves the tail in place and the
        // staged values owned by `extra`.
        let gap = self.drain.owner.extent.len();
        self.drain.owner.extent.set_len_within(next_len.get())?;
        // SAFETY: storage includes tail and extra; shift tail right once, then move
        // staging ownership into the gap. No user code between the state updates.
        unsafe {
            core::ptr::copy(
                self.drain.owner.ptr.as_ptr().add(self.drain.tail_start),
                self.drain.owner.ptr.as_ptr().add(next_tail.get()),
                self.drain.tail_len,
            );
            core::ptr::copy_nonoverlapping(
                extra.ptr.as_ptr(),
                self.drain.owner.ptr.as_ptr().add(gap),
                extra.extent.len(),
            );
        }
        self.drain.tail_start = next_tail.get();
        extra.extent.shorten_to(0);
        Ok(())
    }
}
impl<T, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy> Iterator
    for Splice<'_, T, I, P, L>
{
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<T> {
        self.drain.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.drain.size_hint()
    }
}
impl<T, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy> DoubleEndedIterator
    for Splice<'_, T, I, P, L>
{
    fn next_back(&mut self) -> Option<T> {
        self.drain.next_back()
    }
}
impl<T, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy> ExactSizeIterator
    for Splice<'_, T, I, P, L>
{
}
impl<T, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy> core::iter::FusedIterator
    for Splice<'_, T, I, P, L>
{
}

impl<T: core::fmt::Debug, F, P: AllocationPolicy, L: LengthPolicy> core::fmt::Debug
    for ExtractIf<'_, T, F, P, L>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let peek = if self.state.read() < self.state.end() {
            // SAFETY: current element is initialized and not yet visited.
            Some(unsafe { &*self.owner.ptr.as_ptr().add(self.state.read()) })
        } else {
            None
        };
        f.debug_struct("ExtractIf")
            .field("peek", &peek)
            .finish_non_exhaustive()
    }
}
impl<T: core::fmt::Debug, I: TryIterator<Item = T>, P: AllocationPolicy, L: LengthPolicy>
    core::fmt::Debug for Splice<'_, T, I, P, L>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Splice")
            .field("drain", &self.drain)
            .finish_non_exhaustive()
    }
}

impl<
    T,
    I: TryIterator<Item = T, Error = core::convert::Infallible>,
    P: AllocationPolicy,
    L: LengthPolicy,
> Splice<'_, T, I, P, L>
{
    /// Complete an infallible-source replacement, returning storage errors.
    #[inline]
    pub fn finish(self) -> Result<(), Error> {
        self.try_finish().map_err(|error| match error {
            InitError::Storage(error) => error,
            InitError::Initialize(never) => match never {},
        })
    }
}

// Preserve allocation, write and publication in the actual owning Vec leaf.
#[inline]
fn stage_batch<T, I: TryIterator<Item = T>, P: AllocationPolicy>(
    extra: &mut Vec<T, P, LengthLimit>,
    replacement: &mut I,
    value: T,
) -> Result<Option<T>, InitError<I::Error>> {
    stage_push(extra, value).map_err(InitError::Storage)?;
    let end = extra.capacity();
    // SAFETY: the temporary owns the initialized prefix and exclusive spare
    // storage through its admitted capacity, including for zero-sized values.
    let pointer = extra.ptr;
    let mut length = LentLength::new(&mut extra.extent);
    let filled =
        unsafe { crate::init::collection::fill(pointer, &mut length.cursor, end, replacement) };
    drop(length);
    if !filled.map_err(InitError::Initialize)? {
        return Ok(None);
    }
    replacement.try_next().map_err(InitError::Initialize)
}

#[inline(always)]
fn stage_push<T, P: AllocationPolicy>(
    extra: &mut Vec<T, P, LengthLimit>,
    value: T,
) -> Result<usize, Error> {
    let next = (extra.extent.length() + Checked::new(1)).map_err(|_| Error::Capacity)?;
    extra.try_reserve(1)?;
    // The length is admitted before the value moves; a refusal drops it like
    // the refusals above. Nothing can unwind before the write.
    let index = extra.extent.len();
    extra.extent.set_len_within(next.get())?;
    // SAFETY: the admitted length holds the owner's unwritten next slot.
    unsafe { extra.ptr.as_ptr().add(index).write(value) };
    Ok(extra.capacity())
}
