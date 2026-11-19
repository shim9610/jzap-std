//! The only product allocation boundary; no OOM handler or infallible allocator API.
use crate::{Error, budget::Accounting};
use core::{alloc::Layout, ptr::NonNull};
#[inline]
pub(crate) fn allocate(layout: Layout, budget: Accounting<'_>) -> Result<NonNull<u8>, Error> {
    allocate_impl(layout, budget, false)
}
#[inline]
pub(crate) fn allocate_zeroed(
    layout: Layout,
    budget: Accounting<'_>,
) -> Result<NonNull<u8>, Error> {
    allocate_impl(layout, budget, true)
}
#[inline]
fn allocate_impl(
    layout: Layout,
    budget: Accounting<'_>,
    zeroed: bool,
) -> Result<NonNull<u8>, Error> {
    let reservation = budget.reserve(layout.size())?;
    let ptr = if layout.size() == 0 {
        // This aligned non-null address is never dereferenced for bytes of a ZST.
        NonNull::without_provenance(
            core::num::NonZeroUsize::new(layout.align()).ok_or(Error::Layout)?,
        )
    } else {
        // SAFETY: nonzero, valid Layout. Null is returned to the caller as an error.
        NonNull::new(unsafe {
            if zeroed {
                alloc::alloc::alloc_zeroed(layout)
            } else {
                alloc::alloc::alloc(layout)
            }
        })
        .ok_or(Error::Allocation {
            size: layout.size(),
            align: layout.align(),
        })?
    };
    if let Some(r) = reservation {
        r.commit().retain();
    }
    Ok(ptr)
}
/// SAFETY: ptr is exclusively owned and was allocated with this exact layout/budget.
#[inline]
pub(crate) unsafe fn deallocate(ptr: NonNull<u8>, layout: Layout, budget: Accounting<'_>) {
    if layout.size() != 0 {
        // SAFETY: inherited allocation ownership and exact layout from caller.
        unsafe {
            alloc::alloc::dealloc(ptr.as_ptr(), layout);
        }
    }
    budget.release(layout.size());
}

/// Resize a global allocation of unchanged alignment, reserving the entire new
/// request while the old charge is still held. Failure leaves the old owner intact.
/// # Safety
/// ptr uniquely owns exactly old; no references into the allocation may survive.
#[inline]
pub(crate) unsafe fn reallocate(
    ptr: NonNull<u8>,
    old: Layout,
    new: Layout,
    budget: Accounting<'_>,
) -> Result<NonNull<u8>, Error> {
    if old.align() != new.align() {
        return Err(Error::Layout);
    }
    if old.size() == new.size() {
        return Ok(ptr);
    }
    if old.size() == 0 || new.size() == 0 {
        let result = allocate(new, budget)?;
        // SAFETY: no initialized bytes require preservation in either zero-size case.
        unsafe { deallocate(ptr, old, budget) };
        return Ok(result);
    }
    let reservation = budget.reserve(new.size())?;
    // SAFETY: both nonzero layouts are valid, alignment is unchanged; exclusive ownership.
    let result = NonNull::new(unsafe { alloc::alloc::realloc(ptr.as_ptr(), old, new.size()) })
        .ok_or(Error::Allocation {
            size: new.size(),
            align: new.align(),
        })?;
    if let Some(reservation) = reservation {
        reservation.commit().retain();
    }
    budget.release(old.size());
    Ok(result)
}
