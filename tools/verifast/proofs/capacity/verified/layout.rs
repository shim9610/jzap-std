//! Checked allocation size calculations, independent of an allocator.
use crate::Error;
use core::alloc::Layout;
pub(crate) mod numeric;
#[cfg(kani)]
mod proofs;
/// Reject multiplication/alignment/isize bounds before calling an allocator.
pub fn array<T>(count: usize) -> Result<Layout, Error> 
//@ req thread_token(?t);
//@ ens thread_token(t) &*& match result { Result::Ok(l) => std::alloc::is_valid_layout(count * std::mem::size_of::<T>(), std::mem::align_of::<T>()) == true &*& l.size() == count * std::mem::size_of::<T>() &*& l.align() == std::mem::align_of::<T>(), Result::Err(e) => <error::Error>.own(t, e) };
//@ on_unwind_ens false;
/*@
safety_proof {
    let outcome = call();
    match outcome {
        returning(result) => {
            match result {
                Result::Ok(l) => { std::alloc::close_Layout_own(_t, l); }
                Result::Err(e) => {}
            }
            close <std::result::Result<std::alloc::Layout, error::Error>>.own(_t, result);
        }
        unwinding => {}
    }
}
@*/
{
    let size = core::mem::size_of::<T>();
    // A type's size is a multiple of its alignment, so fitting isize also
    // guarantees that rounding this array size to alignment cannot overflow.
    // This early bound folds the following checked operations into the same
    // release threshold as Layout::array, without its unchecked multiplication.
    let bytes = numeric::array_bytes(count, size)?;
    match Layout::from_size_align(bytes, core::mem::align_of::<T>()) {
        Ok(layout) => Ok(layout),
        Err(_error) => {
            //@ close <error::Error>.own(t, error::Error::Layout);
            Err(Error::Layout)
        }
    }
}
