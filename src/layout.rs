//! Checked allocation size calculations, independent of an allocator.
use crate::Error;
use core::alloc::Layout;
pub(crate) mod numeric;
#[cfg(kani)]
mod proofs;
/// Reject multiplication/alignment/isize bounds before calling an allocator.
pub fn array<T>(count: usize) -> Result<Layout, Error> {
    let size = core::mem::size_of::<T>();
    // A type's size is a multiple of its alignment, so fitting isize also
    // guarantees that rounding this array size to alignment cannot overflow.
    // This early bound folds the following checked operations into the same
    // release threshold as Layout::array, without its unchecked multiplication.
    let bytes = numeric::array_bytes(count, size)?;
    Layout::from_size_align(bytes, core::mem::align_of::<T>()).map_err(|_| Error::Layout)
}
