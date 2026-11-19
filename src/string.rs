//! Explicitly fallible UTF-8 ownership, with optional byte limits and budgets.
//!
//! Byte offsets must identify character boundaries. Explicit str borrows retain
//! core's contracts; no implicit Deref/Index or allocating Clone is provided.
use crate::budget::{AllocationPolicy, Budget, Unmetered};
use crate::vec::{LengthLimit, LengthPolicy, Unlimited, Vec};
use crate::{Error, Rejected, TryClone, TryCloneIn};
use core::ops::RangeBounds;

mod convert;
#[cfg(kani)]
mod cursor_proofs;
pub(crate) mod edit;
mod encoding;
pub(crate) mod numeric;
#[cfg(kani)]
mod proofs;
mod traits;
pub use convert::{FormatError, Utf8Lossy, Utf16Error};
pub use edit::Drain;

#[repr(transparent)]
pub struct String<P: AllocationPolicy = Unmetered, L: LengthPolicy = Unlimited> {
    bytes: Vec<u8, P, L>,
}
/// A UTF-8 owner with a fixed maximum byte length.
pub type BoundedString<P = Unmetered> = String<P, LengthLimit>;

impl String {
    pub const fn new() -> Self {
        Self { bytes: Vec::new() }
    }
    pub fn with_limit(max_bytes: usize) -> BoundedString {
        String::with_limit_in(max_bytes, Unmetered)
    }
    pub fn try_with_capacity(capacity: usize) -> Result<Self, Error> {
        Self::try_with_capacity_in(capacity, Unmetered)
    }
    pub fn try_with_capacity_and_limit(
        capacity: usize,
        max_bytes: usize,
    ) -> Result<BoundedString, Error> {
        Self::try_with_capacity_and_limit_in(capacity, max_bytes, Unmetered)
    }
    pub fn try_from_str(source: &str) -> Result<Self, Error> {
        Self::try_from_str_in(source, Unmetered, Unlimited)
    }
}
impl<P: AllocationPolicy> String<P> {
    pub fn new_in(policy: P) -> Self {
        Self {
            bytes: Vec::new_in(policy),
        }
    }
    pub fn with_limit_in(max_bytes: usize, policy: P) -> BoundedString<P> {
        String {
            bytes: Vec::with_limit_in(max_bytes, policy),
        }
    }
    pub fn try_with_capacity_in(capacity: usize, policy: P) -> Result<Self, Error> {
        Ok(Self {
            bytes: Vec::try_with_capacity_in(capacity, policy)?,
        })
    }
    pub fn try_with_capacity_and_limit_in(
        capacity: usize,
        max_bytes: usize,
        policy: P,
    ) -> Result<BoundedString<P>, Error> {
        Ok(String {
            bytes: Vec::try_with_capacity_and_limit_in(capacity, max_bytes, policy)?,
        })
    }
}
impl<P: AllocationPolicy, L: LengthPolicy> String<P, L> {
    pub fn try_from_str_in(source: &str, policy: P, limit: L) -> Result<Self, Error> {
        Ok(Self {
            bytes: Vec::try_from_slice_in(source.as_bytes(), policy, limit)?,
        })
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: constructors validate UTF-8; all safe edits preserve complete
        // scalars. Raw byte mutation is an explicit unsafe boundary.
        unsafe { core::str::from_utf8_unchecked(self.bytes.as_slice()) }
    }
    #[inline]
    pub fn as_mut_str(&mut self) -> &mut str {
        // SAFETY: same invariant; safe str mutations preserve UTF-8.
        unsafe { core::str::from_utf8_unchecked_mut(self.bytes.as_mut_slice()) }
    }
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.bytes.capacity()
    }
    #[inline]
    pub fn max_len(&self) -> usize {
        self.bytes.max_len()
    }
    pub fn remaining_limit(&self) -> usize {
        self.bytes.remaining_limit()
    }
    pub fn allocation_policy(&self) -> &P {
        self.bytes.allocation_policy()
    }
    pub fn length_policy(&self) -> L {
        self.bytes.length_policy()
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr()
    }
    pub fn is_char_boundary(&self, at: usize) -> bool {
        self.as_str().is_char_boundary(at)
    }
    pub fn get(&self, range: impl RangeBounds<usize>) -> Result<&str, Error> {
        let (start, end) = self.range(range)?;
        self.as_str().get(start..end).ok_or(Error::Bounds)
    }
    pub fn get_mut(&mut self, range: impl RangeBounds<usize>) -> Result<&mut str, Error> {
        let (start, end) = self.range(range)?;
        self.as_mut_str().get_mut(start..end).ok_or(Error::Bounds)
    }
    pub fn chars(&self) -> core::str::Chars<'_> {
        self.as_str().chars()
    }
    pub fn char_indices(&self) -> core::str::CharIndices<'_> {
        self.as_str().char_indices()
    }
    pub fn bytes(&self) -> core::str::Bytes<'_> {
        self.as_str().bytes()
    }
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), Error> {
        self.bytes.try_reserve(additional)
    }
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), Error> {
        self.bytes.try_reserve_exact(additional)
    }
    pub fn try_shrink_to(&mut self, minimum: usize) -> Result<(), Error> {
        self.bytes.try_shrink_to(minimum)
    }
    pub fn try_shrink_to_fit(&mut self) -> Result<(), Error> {
        self.bytes.try_shrink_to_fit()
    }
    #[inline]
    pub fn try_push_str(&mut self, text: &str) -> Result<(), Error> {
        self.bytes.try_extend_copy(text.as_bytes())
    }
    #[inline(always)]
    pub fn try_push(&mut self, ch: char) -> Result<(), Error> {
        let old = self.len();
        let plan = edit::Edit::new(old, old, old, ch.len_utf8(), usize::MAX)?;
        // Existing capacity already obeys the byte ceiling and budget. Growing
        // storage checks both policies before the first write.
        if plan.len > self.capacity() {
            self.bytes.try_reserve(plan.additional)?;
        }
        // SAFETY: the checked edit plan and reservation provide the suffix;
        // the all-scalar encoder proof establishes exactly ch.len_utf8() valid
        // bytes before length publication.
        unsafe {
            encoding::write_scalar(ch, self.bytes.as_mut_ptr().add(plan.start));
            self.bytes.try_set_len(plan.len)
        }
    }
    pub fn clear(&mut self) {
        self.bytes.clear();
    }
    fn boundary(&self, at: usize) -> Result<(), Error> {
        if at > self.len() {
            return Err(Error::Bounds);
        }
        if !self.is_char_boundary(at) {
            return Err(Error::CharBoundary);
        }
        Ok(())
    }
    fn range(&self, range: impl RangeBounds<usize>) -> Result<(usize, usize), Error> {
        let (start, end) = crate::vec::edit::checked_range(range, self.len())?;
        self.boundary(start)?;
        self.boundary(end)?;
        Ok((start, end))
    }
    pub fn truncate(&mut self, at: usize) -> Result<(), Error> {
        if at >= self.len() {
            return Ok(());
        }
        self.boundary(at)?;
        self.bytes.truncate(at);
        Ok(())
    }
    pub fn pop(&mut self) -> Option<char> {
        let mut chars = self.as_str().chars();
        let ch = chars.next_back()?;
        self.bytes.truncate(chars.as_str().len());
        Some(ch)
    }
    pub fn try_split_off(&mut self, at: usize) -> Result<Self, Error> {
        self.boundary(at)?;
        Ok(Self {
            bytes: self.bytes.try_split_off(at)?,
        })
    }
    pub fn try_append<Q: AllocationPolicy, M: LengthPolicy>(
        &mut self,
        other: &mut String<Q, M>,
    ) -> Result<(), Error> {
        self.bytes.try_append(&mut other.bytes)
    }
    pub fn try_extend_from_within(&mut self, range: impl RangeBounds<usize>) -> Result<(), Error> {
        let (start, end) = self.range(range)?;
        self.bytes.try_extend_copy_from_within(start..end)
    }
    pub fn into_bytes(self) -> Vec<u8, P, L> {
        self.bytes
    }
}
