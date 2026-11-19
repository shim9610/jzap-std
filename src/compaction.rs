//! Closed numeric state for stable in-place compaction and suffix restoration.
use crate::Checked;
use crate::arithmetic::bounded::{wrapping_add, wrapping_sub};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[derive(Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct Compaction {
    read: Checked<usize>,
    write: Checked<usize>,
    original: Checked<usize>,
}
#[cfg(verus_keep_ghost)]
verus! {
pub(crate) struct CompactionView {
    pub read: usize, pub write: usize, pub original: usize,
}
impl Compaction {
    pub(crate) closed spec fn view(self) -> CompactionView {
        CompactionView { read: self.read@, write: self.write@, original: self.original@ }
    }
    pub(crate) open spec fn wf(self) -> bool {
        self@.write <= self@.read <= self@.original
    }
    /// Construct an arbitrary valid numeric proof state without a runtime API.
    pub(crate) fn proof_from_state(read: usize, write: usize, original: usize) -> (s: Self)
        requires write <= read <= original,
        ensures s.wf(), s@.read == read, s@.write == write, s@.original == original,
    {
        Self { read: Checked::new(read), write: Checked::new(write), original: Checked::new(original) }
    }
}
}
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl Compaction {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.wf(), out@.read == 0, out@.write == 0, out@.original == original,
    ))]
    #[inline]
    pub(crate) const fn full(original: usize) -> Self {
        Self {
            read: Checked::new(0),
            write: Checked::new(0),
            original: Checked::new(original),
        }
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.wf(), out@.read == out@.write,
            out@.read == if original == 0 { 0usize } else { 1usize }, out@.original == original,
    ))]
    pub(crate) fn first_kept(original: usize) -> Self {
        let first = original.min(1);
        Self {
            read: Checked::new(first),
            write: Checked::new(first),
            original: Checked::new(original),
        }
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(s) => s.wf() && s@.read == start && s@.write == start && s@.original == original,
            Err(_) => start > original,
        },
    ))]
    #[cfg_attr(not(any(kani, verus_keep_ghost)), allow(dead_code))]
    pub(crate) fn try_prefix(start: usize, original: usize) -> Result<Self, crate::Error> {
        if start > original {
            return Err(crate::Error::Bounds);
        }
        Ok(Self {
            read: Checked::new(start),
            write: Checked::new(start),
            original: Checked::new(original),
        })
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.wf(),
        ensures match out {
            Some(index) => self@.read < self@.original && index == self@.read,
            None => self@.read == self@.original,
        },
    ))]
    pub(crate) fn peek(&self) -> Option<usize> {
        if self.read.get() < self.original.get() {
            Some(self.read.get())
        } else {
            None
        }
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self@.read,))]
    pub(crate) fn read(&self) -> usize {
        self.read.get()
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self@.write,))]
    pub(crate) fn written(&self) -> usize {
        self.write.get()
    }
    #[cfg(kani)]
    pub(crate) fn valid_for(&self, original: usize) -> bool {
        self.write.get() <= self.read.get()
            && self.read.get() <= self.original.get()
            && self.original.get() == original
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.wf(), self@.write > 0,
        ensures out as int == self@.write as int - 1,
    ))]
    #[cfg_attr(not(any(kani, verus_keep_ghost)), allow(dead_code))]
    pub(crate) fn previous_kept(&self) -> usize {
        // Kept for the state-contract adapters and arbitrary-state Rust checks.
        // The caller establishes write >= 1 before requesting a predecessor.
        wrapping_sub(self.write.get(), 1)
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.wf(),
        ensures out as int == self@.original as int - self@.read as int, out <= self@.original,
    ))]
    pub(crate) fn remaining(&self) -> usize {
        // Closed invariant: write <= read <= original.
        wrapping_sub(self.original.get(), self.read.get())
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.wf(),
        ensures out as int == self@.original as int - (self@.read as int - self@.write as int),
            out <= self@.original,
    ))]
    pub(crate) fn restored_len(&self) -> usize {
        // The restored suffix and retained prefix cannot exceed original.
        wrapping_add(self.write.get(), self.remaining())
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).wf(),
        ensures final(self).wf(), final(self)@.original == old(self)@.original,
            out == (old(self)@.read < old(self)@.original),
            final(self)@.read as int == old(self)@.read as int + if out { 1int } else { 0int },
            final(self)@.write as int == old(self)@.write as int + if out { 1int } else { 0int },
    ))]
    pub(crate) fn keep_next(&mut self) -> bool {
        if self.read.get() >= self.original.get() {
            return false;
        }
        // In this closed type, write <= read < original proves both successors
        // fit. The source-extracted contracts verify this primitive arithmetic
        // for every valid state; constructors and all transitions close the invariant.
        let next_read = Checked::new(wrapping_add(self.read.get(), 1));
        let next_write = Checked::new(wrapping_add(self.write.get(), 1));
        self.read = next_read;
        self.write = next_write;
        true
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).wf(),
        ensures final(self).wf(), final(self)@.original == old(self)@.original,
            final(self)@.write == old(self)@.write,
            match out {
                Some(index) => index == old(self)@.read && old(self)@.read < old(self)@.original
                    && final(self)@.read as int == old(self)@.read as int + 1,
                None => old(self)@.read == old(self)@.original && final(self)@.read == old(self)@.read,
            },
    ))]
    pub(crate) fn remove_next(&mut self) -> Option<usize> {
        let read = self.peek()?;
        // peek established read < original, so this successor cannot overflow.
        let next = Checked::new(wrapping_add(self.read.get(), 1));
        self.read = next;
        Some(read)
    }
    /// Caller has already admitted a live element (read < original).
    /// Source-derived edit-loop contracts prove this at every call.
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires old(self).wf(), old(self)@.read < old(self)@.original,
        ensures final(self).wf(), final(self)@.original == old(self)@.original,
            final(self)@.read as int == old(self)@.read as int + 1,
            final(self)@.write as int == old(self)@.write as int + 1,
    ))]
    pub(crate) fn keep_live(&mut self) {
        let next_read = Checked::new(wrapping_add(self.read.get(), 1));
        let next_write = Checked::new(wrapping_add(self.write.get(), 1));
        self.read = next_read;
        self.write = next_write;
    }
    /// Caller has already admitted a live element; retirement precedes Drop.
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires old(self).wf(), old(self)@.read < old(self)@.original,
        ensures final(self).wf(), final(self)@.original == old(self)@.original,
            final(self)@.read as int == old(self)@.read as int + 1,
            final(self)@.write == old(self)@.write,
    ))]
    pub(crate) fn remove_live(&mut self) {
        let next = Checked::new(wrapping_add(self.read.get(), 1));
        self.read = next;
    }
}

// The active scan is bounded by its range end, while total retains the original
// tail for cleanup. Four words, matching the former raw iterator counters.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct Extraction {
    read: Checked<usize>,
    deleted: Checked<usize>,
    end: Checked<usize>,
    total: Checked<usize>,
}
#[cfg(verus_keep_ghost)]
verus! {
pub(crate) struct ExtractionView {
    pub scan: CompactionView, pub total: usize, pub deleted: usize,
}
impl Extraction {
    pub(crate) closed spec fn view(self) -> ExtractionView {
        ExtractionView {
            scan: CompactionView {
                read: self.read@,
                write: ((self.read@ as int) - (self.deleted@ as int)) as usize,
                original: self.end@,
            },
            total: self.total@,
            deleted: self.deleted@,
        }
    }
    pub(crate) open spec fn wf(self) -> bool {
        self@.deleted <= self@.scan.read
            && self@.scan.write <= self@.scan.read <= self@.scan.original <= self@.total
    }
    pub(crate) fn proof_from_state(read: usize, write: usize, end: usize, total: usize) -> (s: Self)
        requires write <= read <= end <= total,
        ensures s.wf(), s@.scan.read == read, s@.scan.write == write,
            s@.scan.original == end, s@.total == total,
    {
        Self {
            read: Checked::new(read),
            deleted: Checked::new(read - write),
            end: Checked::new(end),
            total: Checked::new(total),
        }
    }
}
}
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl Extraction {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(s) => s.wf() && s@.scan.read == start && s@.scan.write == start
                && s@.scan.original == end && s@.total == total,
            Err(_) => start > end || end > total,
        },
    ))]
    #[inline]
    pub(crate) fn try_range(start: usize, end: usize, total: usize) -> Result<Self, crate::Error> {
        if start > end || end > total {
            return Err(crate::Error::Bounds);
        }
        Ok(Self {
            read: Checked::new(start),
            deleted: Checked::new(0),
            end: Checked::new(end),
            total: Checked::new(total),
        })
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self@.scan.read,))]
    pub(crate) fn read(&self) -> usize {
        self.read.get()
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.wf(),
        ensures out == self@.scan.write,
    ))]
    pub(crate) fn written(&self) -> usize {
        wrapping_sub(self.read.get(), self.deleted.get())
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self@.scan.original,))]
    pub(crate) fn end(&self) -> usize {
        self.end.get()
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.wf(),
        ensures out as int == self@.scan.original as int - self@.scan.read as int,
    ))]
    pub(crate) fn remaining(&self) -> usize {
        wrapping_sub(self.end.get(), self.read.get())
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.wf(),
        ensures out == (self@.scan.read == self@.scan.write),
    ))]
    pub(crate) fn untouched(&self) -> bool {
        self.deleted.get() == 0
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).wf(),
        ensures final(self).wf(), final(self)@.total == old(self)@.total,
            final(self)@.scan.original == old(self)@.scan.original,
            out == (old(self)@.scan.read < old(self)@.scan.original),
            final(self)@.scan.read as int == old(self)@.scan.read as int + if out { 1int } else { 0int },
            final(self)@.scan.write as int == old(self)@.scan.write as int + if out { 1int } else { 0int },
    ))]
    pub(crate) fn keep_next(&mut self) -> bool {
        if self.read.get() >= self.end.get() {
            return false;
        }
        // No element is removed, so only the scan index advances.
        self.read = Checked::new(wrapping_add(self.read.get(), 1));
        true
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).wf(),
        ensures final(self).wf(), final(self)@.total == old(self)@.total,
            final(self)@.scan.original == old(self)@.scan.original,
            final(self)@.scan.write == old(self)@.scan.write,
            match out {
                Some(index) => index == old(self)@.scan.read && old(self)@.scan.read < old(self)@.scan.original
                    && final(self)@.scan.read as int == old(self)@.scan.read as int + 1,
                None => old(self)@.scan.read == old(self)@.scan.original
                    && final(self)@.scan.read == old(self)@.scan.read,
            },
    ))]
    pub(crate) fn remove_next(&mut self) -> Option<usize> {
        let read = self.read.get();
        if read >= self.end.get() {
            return None;
        }
        // deleted <= read < end proves both successors fit every word width.
        self.read = Checked::new(wrapping_add(read, 1));
        self.deleted = Checked::new(wrapping_add(self.deleted.get(), 1));
        Some(read)
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires old(self).wf(),
        ensures final(self).wf(), final(self)@.total == old(self)@.total,
            final(self)@.scan.original == old(self)@.scan.read,
            final(self)@.scan.read == old(self)@.scan.read,
            final(self)@.scan.write == old(self)@.scan.write,
    ))]
    pub(crate) fn stop(&mut self) {
        self.end = self.read;
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.wf(),
        ensures out as int == self@.total as int - self@.scan.read as int,
    ))]
    pub(crate) fn unvisited_len(&self) -> usize {
        wrapping_sub(self.total.get(), self.read.get())
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.wf(),
        ensures out as int == self@.total as int - (self@.scan.read as int - self@.scan.write as int),
            out <= self@.total,
    ))]
    pub(crate) fn restored_len(&self) -> usize {
        wrapping_sub(self.total.get(), self.deleted.get())
    }
    #[cfg(kani)]
    pub(crate) fn from_cursor_state(
        write: usize,
        read: usize,
        end: usize,
        total: usize,
    ) -> Result<Self, crate::Error> {
        if write > read || read > end || end > total {
            return Err(crate::Error::Bounds);
        }
        Ok(Self {
            read: Checked::new(read),
            deleted: Checked::new(wrapping_sub(read, write)),
            end: Checked::new(end),
            total: Checked::new(total),
        })
    }
    #[cfg(kani)]
    pub(crate) fn valid(&self) -> bool {
        self.deleted.get() <= self.read.get()
            && self.read.get() <= self.end.get()
            && self.end.get() <= self.total.get()
    }
    #[cfg(kani)]
    pub(crate) fn total(&self) -> usize {
        self.total.get()
    }
    #[cfg(kani)]
    pub(crate) fn valid_progress(
        &self,
        end: usize,
        total: usize,
        minimum_read: usize,
        minimum_write: usize,
    ) -> bool {
        // Loop-invariant expressions must be side-effect free. The product
        // written() retains its bounded-subtraction assertion; this observer
        // expresses the same progress bound using checked arithmetic.
        let Some(write) = self.read.get().checked_sub(self.deleted.get()) else {
            return false;
        };
        self.valid()
            && self.end.get() == end
            && self.total.get() == total
            && minimum_read <= self.read.get()
            && minimum_write <= write
    }
}

#[cfg(kani)]
#[path = "compaction_proofs.rs"]
mod proofs;
