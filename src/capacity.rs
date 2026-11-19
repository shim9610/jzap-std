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

pub(crate) struct Capacity<T> {
    count: Checked<usize>,
    element: PhantomData<fn() -> T>,
}
impl<T> Copy for Capacity<T> {}
impl<T> Clone for Capacity<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Capacity<T> {
    #[inline]
    pub(crate) const fn empty(limit: usize) -> Self {
        Self {
            count: Checked::new(logical_capacity(mem::size_of::<T>(), 0, limit)),
            element: PhantomData,
        }
    }
    #[inline]
    pub(crate) fn try_new(count: usize) -> Result<(Self, Layout), Error> {
        let layout = crate::layout::array::<T>(count)?;
        Ok((
            Self {
                count: Checked::new(count),
                element: PhantomData,
            },
            layout,
        ))
    }
    #[inline]
    pub(crate) fn for_slice(values: &[T], limit: usize) -> Self {
        // A valid Rust slice already satisfies its array byte-layout bounds.
        let count = logical_capacity(mem::size_of::<T>(), values.len(), limit);
        Self {
            count: Checked::new(count),
            element: PhantomData,
        }
    }
    /// # Safety
    /// count must describe a valid array layout for T, including isize bounds.
    #[inline]
    pub(crate) unsafe fn from_validated_raw(count: usize) -> Self {
        Self {
            count: Checked::new(count),
            element: PhantomData,
        }
    }
    #[inline]
    pub(crate) fn get(self) -> usize {
        self.count.get()
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
