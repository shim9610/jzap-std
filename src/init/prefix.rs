//! Shared physical prefix ownership for array and slice initialization.
use super::InitSlot;
#[cfg(kani)]
use crate::Error;
use crate::{Checked, error::InitError};
use core::ptr::NonNull;
struct Prefix<T> {
    ptr: NonNull<T>,
    state: Checked<usize>,
}
impl<T> Drop for Prefix<T> {
    fn drop(&mut self) {
        if !core::mem::needs_drop::<T>() {
            return;
        }
        // SAFETY: only completed slots have advanced the private state.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.ptr.as_ptr(),
                self.state.get(),
            ));
        }
    }
}
#[cfg_attr(kani, inline(never))]
#[cfg_attr(not(kani), inline(always))]
unsafe fn initialize_one<T, E>(
    ptr: NonNull<T>,
    index: usize,
    initialize: &mut impl FnMut(usize, &mut InitSlot<'_, T>) -> Result<(), E>,
) -> Result<(), InitError<E>> {
    // SAFETY: the private caller supplies an admitted index in exclusive storage.
    let pointer = unsafe { NonNull::new_unchecked(ptr.as_ptr().add(index)) };
    // SAFETY: this element has not yet transferred to the completed prefix.
    let mut slot = unsafe { InitSlot::new(pointer) };
    initialize(index, &mut slot).map_err(InitError::Initialize)?;
    slot.finish().map_err(InitError::Storage)
}
/// Initialize exactly `length` elements, releasing a completed prefix on failure.
/// The callback stays borrowed so its destructor runs in the original owner scope.
///
/// # Safety
/// `ptr` must exclusively own aligned, writable, initially uninitialized storage
/// for `length` elements of T. The allocation and all callback-borrowed resources
/// must remain live until this call returns. On success ownership of every value
/// returns to the caller; on error no initialized value remains in the storage.
#[inline]
pub(crate) unsafe fn initialize<T, E>(
    ptr: NonNull<T>,
    length: usize,
    initialize: &mut impl FnMut(usize, &mut InitSlot<'_, T>) -> Result<(), E>,
) -> Result<(), InitError<E>> {
    let mut prefix = Prefix {
        ptr,
        state: Checked::new(0),
    };
    // SAFETY: the driver admits only indices below length, advancing after finish.
    super::driver::run(&mut prefix.state, length, |index| unsafe {
        initialize_one(ptr, index, initialize)
    })?;
    prefix.state = Checked::new(0);
    Ok(())
}
#[cfg(kani)]
#[path = "prefix_proofs.rs"]
mod proofs;
