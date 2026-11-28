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
pub(crate) const fn logical_capacity(size: usize, stored: usize, limit: usize) -> usize 
//@ req true;
//@ ens result == if size == 0 { limit } else { stored };
//@ on_unwind_ens false;
{
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
/*@
pred<T> <Capacity<T>>.own(t, c) = std::alloc::is_valid_layout(c.count.0 * std::mem::size_of::<T>(), std::mem::align_of::<T>()) == true;

lem Capacity_drop<T>()
    req Capacity_own::<T>(?t, ?c);
    ens <arithmetic::checked::Checked<usize>>.own(t, c.count);
{
    open Capacity_own::<T>(t, c);
    close usize_own(t, c.count.0);
    close arithmetic::checked::Checked_own::<usize>(t, c.count);
}

lem Capacity_own_mono<T0, T1>()
    req type_interp::<T0>() &*& type_interp::<T1>() &*& Capacity_own::<T0>(?t, ?v) &*& is_subtype_of::<T0, T1>() == true;
    ens type_interp::<T0>() &*& type_interp::<T1>() &*& Capacity_own::<T1>(t, Capacity::<T1> { count: upcast(v.count) });
{
    assume(false); // variance obligation; VeriFast's own RawVec proof does the same (verifast issue #610)
}

// 0 <= m % a <= m for m >= 0, a > 0.
lem mod_le(m: i32, a: i32)
    req 0 <= m &*& 0 < a;
    ens 0 <= m % a &*& m % a <= m;
{
    div_rem_nonneg(m, a);
    mul_mono_l(0, m / a, a);
}
@*/
impl<T> Copy for Capacity<T> {}
impl<T> Clone for Capacity<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Capacity<T> {
    #[inline]
    pub(crate) const fn empty(limit: usize) -> Self 
    //@ req true;
    //@ ens result.count.0 == if std::mem::size_of::<T>() == 0 { limit } else { 0 };
    //@ on_unwind_ens false;
    //@ safety_proof { let outcome = call(); match outcome { returning(r) => { std::alloc::is_valid_layout_size_of_align_of::<T>(); mod_le(isize::MAX, std::mem::align_of::<T>()); close Capacity_own::<T>(_t, r); } unwinding => {} } }
    {
        Self {
            count: Checked::new(logical_capacity(mem::size_of::<T>(), 0, limit)),
            element: PhantomData,
        }
    }
    #[inline]
    pub(crate) fn try_new(count: usize) -> Result<(Self, Layout), Error> 
    //@ req thread_token(?t);
    /*@ ens thread_token(t) &*& match result {
            Result::Ok(p) => p.0.count.0 == count &*& std::alloc::is_valid_layout(count * std::mem::size_of::<T>(), std::mem::align_of::<T>()) == true &*& p.1.size() == count * std::mem::size_of::<T>() &*& p.1.align() == std::mem::align_of::<T>(),
            Result::Err(e) => <error::Error>.own(t, e)
        }; @*/
    //@ on_unwind_ens false;
    /*@
    safety_proof {
        let outcome = call();
        match outcome {
            returning(result) => {
                match result {
                    Result::Ok(p) => {
                        close Capacity_own::<T>(_t, p.0);
                        std::alloc::close_Layout_own(_t, p.1);
                        close_tuple_2_own::<Capacity<T>, std::alloc::Layout>(_t, p);
                    }
                    Result::Err(e) => {}
                }
                close <std::result::Result<(Capacity<T>, std::alloc::Layout), error::Error>>.own(_t, result);
            }
            unwinding => {}
        }
    }
    @*/
    {
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
    pub(crate) unsafe fn from_validated_raw(count: usize) -> Self 
    //@ req thread_token(?t) &*& std::alloc::is_valid_layout(count * std::mem::size_of::<T>(), std::mem::align_of::<T>()) == true;
    //@ ens thread_token(t) &*& result.count.0 == count &*& <Capacity<T>>.own(t, result);
    //@ on_unwind_ens false;
    {
        //@ close Capacity_own::<T>(t, Capacity::<T> { count: arithmetic::checked::Checked::<usize> { 0: count } });
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
    pub(crate) fn layout(self) -> Layout 
    //@ req <Capacity<T>>.own(?t, self);
    //@ ens result.size() == self.count.0 * std::mem::size_of::<T>() &*& self.count.0 * std::mem::size_of::<T>() <= isize::MAX &*& result.align() == std::mem::align_of::<T>();
    //@ on_unwind_ens false;
    //@ safety_proof { let outcome = call(); match outcome { returning(r) => { std::alloc::close_Layout_own(_t, r); } unwinding => {} } }
    {
        // Invariant established by every safe constructor, or by the explicit
        // unsafe raw-storage adapter. Copy cannot alter the count or element type.
        let bytes = wrapping_mul(self.count.get(), mem::size_of::<T>());
        /*@
        {
            open Capacity_own::<T>(t, self);
            mod_le(isize::MAX, std::mem::align_of::<T>());
        }
        @*/
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
