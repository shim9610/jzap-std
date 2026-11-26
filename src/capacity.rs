//! Array-layout validity encoded without an extra stored layout or handle word.
use crate::arithmetic::bounded::wrapping_mul;
use crate::{Checked, Error};
use core::{alloc::Layout, marker::PhantomData, mem};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Interpret storage counts without introducing bytes for zero-sized elements.
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == if size == 0 { limit } else { stored },
))]
pub(crate) const fn logical_capacity(size: usize, stored: usize, limit: usize) -> usize {
    if size == 0 { limit } else { stored }
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires length <= stored,
    ensures match out {
        Ok(capacity) => length <= capacity <= limit
            && capacity == if size == 0 { limit } else { stored },
        Err(Error::Capacity) => length > limit || (size != 0 && stored > limit),
        _ => false,
    },
))]
pub(crate) fn adopted_capacity(
    size: usize,
    length: usize,
    stored: usize,
    limit: usize,
) -> Result<usize, Error> {
    let capacity = logical_capacity(size, stored, limit);
    if length > limit || capacity > limit {
        return Err(Error::Capacity);
    }
    Ok(capacity)
}

// The count is private to this module. Every constructor below either checks
// the array byte bound or is the named trusted entry, so a `Capacity<T>` of a
// sized element always describes at most `isize::MAX` bytes.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct Capacity<T> {
    count: Checked<usize>,
    element: PhantomData<T>,
}
impl<T> Copy for Capacity<T> {}
impl<T> Clone for Capacity<T> {
    fn clone(&self) -> Self {
        *self
    }
}
#[cfg(verus_keep_ghost)]
verus! {
/// An array of `count` elements of `size` bytes fits the address space.
pub(crate) open spec fn byte_valid(count: usize, size: usize) -> bool {
    (count as int) * (size as int) <= isize::MAX as int
}
impl<T> Capacity<T> {
    pub(crate) closed spec fn view(self) -> usize { self.count@ }
    #[verifier::type_invariant]
    spec fn wf(self) -> bool {
        vstd::layout::size_of::<T>() != 0
            ==> byte_valid(self.count@, vstd::layout::size_of::<T>() as usize)
    }
    /// The invariant of every value of this type.
    pub(crate) proof fn bytes_fit(tracked &self)
        ensures vstd::layout::size_of::<T>() != 0
            ==> byte_valid(self@, vstd::layout::size_of::<T>() as usize),
    {
        use_type_invariant(self);
    }
}
}
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl<T> Capacity<T> {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out@ == if vstd::layout::size_of::<T>() == 0 { limit } else { 0usize },
    ))]
    pub(crate) const fn empty(limit: usize) -> Self {
        Self {
            count: Checked::new(logical_capacity(mem::size_of::<T>(), 0, limit)),
            element: PhantomData,
        }
    }
    /// The checked constructor: refuses a count whose array exceeds `isize::MAX` bytes.
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(capacity) => capacity@ == count,
            Err(Error::Layout) => !byte_valid(count, vstd::layout::size_of::<T>() as usize),
            _ => false,
        },
    ))]
    fn admitted(count: usize) -> Result<Self, Error> {
        let _bytes = crate::layout::numeric::array_bytes(count, mem::size_of::<T>())?;
        Ok(Self {
            count: Checked::new(count),
            element: PhantomData,
        })
    }
    /// # Safety
    /// count must describe a valid array layout for T, including isize bounds.
    ///
    /// The trusted entry of this type: the caller's contract is the invariant.
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires vstd::layout::size_of::<T>() != 0
            ==> byte_valid(count, vstd::layout::size_of::<T>() as usize),
        ensures out@ == count,
    ))]
    pub(crate) unsafe fn from_validated_raw(count: usize) -> Self {
        Self {
            count: Checked::new(count),
            element: PhantomData,
        }
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self@,))]
    pub(crate) fn get(self) -> usize {
        self.count.get()
    }
}
impl<T> Capacity<T> {
    #[inline]
    pub(crate) fn try_new(count: usize) -> Result<(Self, Layout), Error> {
        let layout = crate::layout::array::<T>(count)?;
        // The same byte admission as the layout, checked by the constructor.
        Ok((Self::admitted(count)?, layout))
    }
    #[inline]
    pub(crate) fn for_slice(values: &[T], limit: usize) -> Self {
        let count = logical_capacity(mem::size_of::<T>(), values.len(), limit);
        // SAFETY: a valid Rust slice already satisfies its array byte-layout
        // bounds, and a zero-sized element has no byte bound.
        unsafe { Self::from_validated_raw(count) }
    }
    #[inline]
    pub(crate) fn layout(self) -> Layout {
        // Invariant established by every safe constructor, or by the explicit
        // unsafe raw-storage adapter. Copy cannot alter the count or element type.
        let bytes = wrapping_mul(self.count.get(), mem::size_of::<T>());
        // SAFETY: array-layout validity also establishes alignment rounding bounds.
        unsafe { Layout::from_size_align_unchecked(bytes, mem::align_of::<T>()) }
    }
}
impl<T> core::ops::Sub<Checked<usize>> for Capacity<T> {
    type Output = Result<Checked<usize>, crate::ArithmeticError>;
    #[inline]
    fn sub(self, rhs: Checked<usize>) -> Self::Output {
        self.count - rhs
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;
    #[kani::proof]
    #[kani::solver(z3)]
    fn constructors_and_reconstructed_layout_preserve_each_byte_bound() {
        let count: usize = kani::any();
        macro_rules! one {
            ($ty:ty) => {{
                let empty = Capacity::<$ty>::empty(count);
                assert!(empty.layout().size() == 0);
                if let Ok((capacity, layout)) = Capacity::<$ty>::try_new(count) {
                    assert!(capacity.get() == count);
                    assert!(capacity.layout() == layout);
                    assert!(capacity.layout().size() <= isize::MAX as usize);
                }
            }};
        }
        one!(u8);
        one!(u64);
        one!([u8; 3]);
        one!([u64; 3]);
        one!(());
        #[repr(align(128))]
        struct Aligned(u8);
        one!(Aligned);
    }
}
