//! Safe initialization tracking. Callback success never substitutes for a write.
use crate::{Error, Rejected};
use core::{marker::PhantomData, ptr::NonNull};
#[cfg(feature = "alloc")]
pub(crate) mod collection;
pub(crate) mod driver;
pub(crate) mod prefix;
#[cfg(kani)]
#[path = "init/proofs.rs"]
mod proofs;
pub trait TryInit<T> {
    type Error;
    fn try_init(self, slot: &mut InitSlot<'_, T>) -> Result<(), Self::Error>;
}
impl<T, E, F: FnOnce(&mut InitSlot<'_, T>) -> Result<(), E>> TryInit<T> for F {
    type Error = E;
    fn try_init(self, slot: &mut InitSlot<'_, T>) -> Result<(), E> {
        self(slot)
    }
}
pub struct InitSlot<'a, T> {
    ptr: NonNull<T>,
    initialized: bool,
    marker: PhantomData<&'a mut T>,
}
impl<T> InitSlot<'_, T> {
    /// SAFETY: ptr owns suitably aligned writable storage for T, initially uninitialized,
    /// and this guard exclusively borrows it until drop or successful disarm.
    pub(crate) unsafe fn new(ptr: NonNull<T>) -> Self {
        Self {
            ptr,
            initialized: false,
            marker: PhantomData,
        }
    }
    pub fn write(&mut self, value: T) -> Result<(), Rejected<T>> {
        if self.initialized {
            return Err(Rejected {
                value,
                error: Error::AlreadyInitialized,
            });
        }
        // SAFETY: guard exclusively owns the uninitialized T slot.
        unsafe { self.ptr.as_ptr().write(value) };
        self.initialized = true;
        Ok(())
    }
    pub fn get(&self) -> Option<&T> {
        if self.initialized {
            Some(unsafe { self.ptr.as_ref() })
        } else {
            None
        }
    } // SAFETY: initialized guard borrow.
    pub(crate) fn finish(mut self) -> Result<(), Error> {
        if !self.initialized {
            return Err(Error::Uninitialized);
        }
        self.initialized = false;
        Ok(())
    }
}
impl<T> Drop for InitSlot<'_, T> {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: only a recorded initialized value is dropped, exactly once.
            unsafe { self.ptr.as_ptr().drop_in_place() }
        }
    }
}

impl<T, const N: usize> InitSlot<'_, [T; N]> {
    /// Initialize elements in their final storage. No whole-array temporary.
    /// On failure the completed prefix and current initialized element are dropped;
    /// the parent remains unwritten and can be retried. Callback effects remain.
    pub fn try_init_array<E>(
        &mut self,
        mut initialize: impl FnMut(usize, &mut InitSlot<'_, T>) -> Result<(), E>,
    ) -> Result<(), crate::error::InitError<E>> {
        use crate::error::InitError;
        if self.initialized {
            return Err(InitError::Storage(Error::AlreadyInitialized));
        }
        // SAFETY: this unwritten slot exclusively borrows storage for all N elements.
        unsafe { prefix::initialize(self.ptr.cast::<T>(), N, &mut initialize) }?;
        // The helper has transferred all elements back to the parent.
        self.initialized = true;
        Ok(())
    }
}

impl<T> InitSlot<'_, T> {
    /// Borrow the address for explicit unsafe initialization. No initialized reference
    /// is produced. The caller must record completion before the slot can expose T.
    pub fn as_uninit_ptr(&mut self) -> Result<NonNull<T>, Error> {
        if self.initialized {
            return Err(Error::AlreadyInitialized);
        }
        Ok(self.ptr)
    }
    /// # Safety
    /// All bytes/fields required by T must now be initialized and valid. Ownership
    /// of every field must belong exclusively to this slot after this call.
    pub unsafe fn mark_initialized(&mut self) -> Result<(), Error> {
        if self.initialized {
            return Err(Error::AlreadyInitialized);
        }
        self.initialized = true;
        Ok(())
    }
    /// # Safety
    /// The pointer must be aligned, writable storage for T, initially uninitialized.
    /// It must be exclusively borrowed for the returned slot's lifetime. The caller
    /// retains storage ownership and must not deallocate it before the guard ends.
    pub unsafe fn from_raw<'a>(ptr: NonNull<T>) -> InitSlot<'a, T>
    where
        T: 'a,
    {
        // SAFETY: inherited exclusive storage contract.
        unsafe { InitSlot::new(ptr) }
    }
    #[doc(hidden)]
    /// # Safety
    /// The value's cleanup ownership must have been transferred to its parent.
    pub unsafe fn disarm_field(mut self) {
        self.initialized = false;
    }
}

/// Initialize every named field in its final slot. An exhaustive pattern checks
/// for missing/duplicate fields; field guards clean up on error or unwind.
/// The initializer expressions run outside unsafe blocks.
#[macro_export]
macro_rules! try_init_struct {
    ($slot:expr, $ty:path { $($field:ident : $initialize:expr),* $(,)? }) => {{
        // This closure is only type checked, never called on uninitialized memory.
        let _ = |__value: $ty| { let $ty { $($field: _,)* } = __value; };
        (|| {
            let __parent: &mut $crate::init::InitSlot<'_, $ty> = $slot;
            let __pointer = __parent.as_uninit_ptr().map_err($crate::error::InitError::Storage)?;
            // Pattern bindings are introduced only AFTER all initializer expressions
            // have run, so caller identifiers cannot access earlier field guards.
            let ($($field,)*) = ($({
                // SAFETY: compiler checks the field; parent is uninitialized,
                // exclusive, and its lifetime encloses every field guard.
                let mut __field = unsafe {
                    let __ptr = core::ptr::addr_of_mut!((*__pointer.as_ptr()).$field);
                    if !__ptr.is_aligned() {
                        return Err($crate::error::InitError::Storage($crate::Error::Layout));
                    }
                    $crate::init::InitSlot::from_raw(core::ptr::NonNull::new_unchecked(__ptr))
                };
                $crate::init::TryInit::try_init($initialize, &mut __field)
                    .map_err($crate::error::InitError::Initialize)?;
                if __field.get().is_none() {
                    return Err($crate::error::InitError::Storage($crate::Error::Uninitialized));
                }
                __field
            },)*);
            // SAFETY: exhaustive field check and each field's write record establish
            // validity; no caller code runs between transfer and disarming guards.
            unsafe { __parent.mark_initialized() }.map_err($crate::error::InitError::Storage)?;
            $(unsafe { $field.disarm_field() };)*
            Ok(())
        })()
    }};
}

/// The safe initializer cannot duplicate fields or smuggle an unsafe caller
/// expression into the implementation's unsafe block.
/// ```compile_fail
/// use jzap::{boxed::Box, budget::Unmetered, init::InitSlot, Error};
/// struct V { a: u8 }
/// let _ = Box::<V>::try_init_in(Unmetered, |slot: &mut InitSlot<'_,V>| {
///     jzap::try_init_struct!(slot,V {
///         a: |s: &mut InitSlot<'_,u8>| s.write(1).map_err(|e|e.error),
///         a: |s: &mut InitSlot<'_,u8>| s.write(2).map_err(|e|e.error),
///     })
/// });
/// ```
/// ```compile_fail
/// use jzap::{boxed::Box, budget::Unmetered, init::InitSlot, Error};
/// struct V { a: u8 }
/// unsafe fn initializer(_: &mut InitSlot<'_,u8>) -> Result<(),Error> { Ok(()) }
/// let _ = Box::<V>::try_init_in(Unmetered, |slot: &mut InitSlot<'_,V>| {
///     jzap::try_init_struct!(slot,V { a: |s: &mut InitSlot<'_,u8>| initializer(s) })
/// });
/// ```
#[allow(dead_code)]
fn struct_macro_contracts() {}
