//! Fallible owning allocation. Construction rejection returns the original value.
use crate::{
    Rejected,
    budget::{Accounting, AllocationPolicy, Budget, Unmetered},
    error::InitError,
    storage,
};
use core::{
    alloc::Layout,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};
pub struct Box<T: ?Sized, P: AllocationPolicy = Unmetered> {
    ptr: NonNull<T>,
    policy: P,
    marker: PhantomData<T>,
}
// Owns only the allocation. Declare before an initializer slot, or before
// invoking T::drop, so storage is released after T even during unwinding.
struct AllocationGuard<'a>(NonNull<u8>, Layout, Accounting<'a>);
impl Drop for AllocationGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: exclusively owned allocation; values are cleaned by the caller.
        unsafe { storage::deallocate(self.0, self.1, self.2) }
    }
}
impl<T, P: AllocationPolicy> Box<T, P> {
    #[inline]
    pub fn try_new_in(value: T, policy: P) -> Result<Self, Rejected<T>> {
        let ptr = match storage::allocate(Layout::new::<T>(), policy.budget()) {
            Ok(p) => p.cast::<T>(),
            Err(error) => return Err(Rejected { value, error }),
        };
        // SAFETY: exact aligned storage, exclusively owned, initialized once.
        unsafe {
            ptr.as_ptr().write(value);
        }
        Ok(Self {
            ptr,
            policy,
            marker: PhantomData,
        })
    }
    /// Reserve storage before calling initialize. This is by-value initialization.
    #[inline]
    pub fn try_new_with_in<E>(
        policy: P,
        initialize: impl FnOnce() -> Result<T, E>,
    ) -> Result<Self, InitError<E>> {
        Self::try_init_in(policy, |slot: &mut crate::init::InitSlot<'_, T>| {
            let value = initialize().map_err(InitError::Initialize)?;
            slot.write(value).map_err(|e| InitError::Storage(e.error))
        })
        .map_err(|error| match error {
            InitError::Storage(error) => InitError::Storage(error),
            InitError::Initialize(error) => error,
        })
    }
    #[inline]
    pub fn try_init_in<I: crate::init::TryInit<T>>(
        policy: P,
        initialize: I,
    ) -> Result<Self, InitError<I::Error>> {
        let ptr = storage::allocate(Layout::new::<T>(), policy.budget())
            .map_err(InitError::Storage)?
            .cast::<T>();
        let guard = AllocationGuard(ptr.cast(), Layout::new::<T>(), policy.budget());
        // SAFETY: allocated exact storage, exclusively borrowed by slot.
        let mut slot = unsafe { crate::init::InitSlot::new(ptr) };
        initialize
            .try_init(&mut slot)
            .map_err(InitError::Initialize)?;
        slot.finish().map_err(InitError::Storage)?;
        core::mem::forget(guard);
        Ok(Self {
            ptr,
            policy,
            marker: PhantomData,
        })
    }
}
impl<T, P: AllocationPolicy> Deref for Box<T, P> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    } // SAFETY: initialized, borrowed owner.
}
impl<T, P: AllocationPolicy> DerefMut for Box<T, P> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    } // SAFETY: exclusive owner borrow.
}
impl<T: ?Sized, P: AllocationPolicy> Drop for Box<T, P> {
    fn drop(&mut self) {
        let layout = Layout::for_value(self.as_ref());
        let _allocation = AllocationGuard(self.ptr.cast(), layout, self.policy.budget());
        if core::mem::needs_drop::<T>() {
            // SAFETY: unique initialized T and matching allocation; T is dropped once.
            unsafe {
                self.ptr.as_ptr().drop_in_place();
            }
        }
    }
}

impl<T: crate::TryClone, P: AllocationPolicy> crate::TryClone for Box<T, P> {
    fn try_clone_from(&mut self, source: &Self) -> Result<(), crate::Error> {
        if let Some(result) = T::try_copy_slice_from(
            core::slice::from_ref(source.as_ref()),
            core::slice::from_mut(self.as_mut()),
        ) {
            return result;
        }
        // Bound library-created stack staging while reusing the allocation for
        // ordinary small values. Large values retain destination initialization
        // in a temporary owner; arbitrary user callbacks retain their contracts.
        if core::mem::size_of::<T>() <= 256 {
            return self.as_mut().try_clone_from(source.as_ref());
        }
        let replacement = Self::try_init_in(
            self.policy.try_clone()?,
            |slot: &mut crate::init::InitSlot<'_, T>| source.as_ref().try_clone_into(slot),
        )
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })?;
        *self = replacement;
        Ok(())
    }
    fn try_clone(&self) -> Result<Self, crate::Error> {
        // Bitwise-clonable values need no initializer slot: allocate, copy and
        // return directly, so the success path does not merge a slot/init
        // error into the result (8 instructions per clone in the u64 loop).
        if T::copy_witness().is_some() {
            let policy = self.policy.try_clone()?;
            let ptr = storage::allocate(Layout::new::<T>(), policy.budget())?.cast::<T>();
            // SAFETY: the private witness permits bitwise duplication of the
            // initialized source; the fresh exact allocation is disjoint and
            // exclusively owned, and becomes the new Box's single value.
            unsafe { ptr.as_ptr().write(self.ptr.as_ptr().read()) };
            return Ok(Self {
                ptr,
                policy,
                marker: PhantomData,
            });
        }
        Self::try_init_in(
            self.policy.try_clone()?,
            |slot: &mut crate::init::InitSlot<'_, T>| (**self).try_clone_into(slot),
        )
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}
impl<'a, T: crate::TryCloneIn<'a>, P: AllocationPolicy> crate::TryCloneIn<'a> for Box<T, P> {
    type Output = Box<T::Output, Budget<'a>>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, crate::Error> {
        Box::try_init_in(budget, |slot: &mut crate::init::InitSlot<'_, T::Output>| {
            (**self).try_clone_into_in(budget, slot)
        })
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}

impl<T, P: AllocationPolicy, const N: usize> Box<[T; N], P> {
    /// Reserve the entire array first, then initialize one destination element at a time.
    pub fn try_init_array_in<E>(
        policy: P,
        initialize: impl FnMut(usize, &mut crate::init::InitSlot<'_, T>) -> Result<(), E>,
    ) -> Result<Self, InitError<E>> {
        Self::try_init_in(policy, |slot: &mut crate::init::InitSlot<'_, [T; N]>| {
            slot.try_init_array(initialize)
        })
        .map_err(|e| match e {
            InitError::Storage(e) => InitError::Storage(e),
            InitError::Initialize(e) => e,
        })
    }
}

impl<T> Box<T, Unmetered> {
    /// Explicitly unmetered allocation; allocator failure returns the input.
    #[inline]
    pub fn try_new(value: T) -> Result<Self, Rejected<T>> {
        Self::try_new_in(value, Unmetered)
    }
}

/// Ownership and budget lifetimes cannot be bypassed by safe code.
///
/// ```compile_fail
/// use jzap::{boxed::Box, budget::{Budget, MemoryBudget}};
/// let value;
/// { let budget = MemoryBudget::new(100); value = Box::try_new_in(1u64, Budget::Limited(&budget)).ok().unwrap(); }
/// drop(value);
/// ```
/// ```compile_fail
/// use jzap::boxed::Box;
/// let value = Box::try_new(1u64).ok().unwrap();
/// let _: Box<u64> = value.clone();
/// ```
/// ```compile_fail
/// use jzap::{boxed::Box, budget::Unmetered, init::InitSlot};
/// let mut escaped = None;
/// let _ = Box::try_init_in(Unmetered, |slot: &mut InitSlot<'_, u64>| {
///     escaped = Some(slot);
///     Ok::<_, ()>(())
/// });
/// ```
mod ownership_contracts {}

// SAFETY: Box uniquely owns T; transferring it also transfers the policy. A
// borrowed Cell-based budget is not Send, so this cannot transfer local accounting.
unsafe impl<T: ?Sized + Send, P: AllocationPolicy + Send> Send for Box<T, P> {}
// SAFETY: shared access exposes only &T and immutable policy access. Mutating the
// payload requires &mut Box; a Cell-based policy is not Sync.
unsafe impl<T: ?Sized + Sync, P: AllocationPolicy + Sync> Sync for Box<T, P> {}

/// A local budget cannot be sent across threads through an owning pointer.
/// ```compile_fail
/// use jzap::{boxed::Box, budget::Budget};
/// fn assert_send<T: Send>() {}
/// assert_send::<Box<u64, Budget<'static>>>();
/// ```
mod local_budget_contract {}

/// Independent initializer slots cannot exchange ownership through safe code.
/// ```compile_fail
/// use jzap::{boxed::Box, budget::Unmetered, init::InitSlot};
/// let _ = Box::<u64>::try_init_in(Unmetered, |outer: &mut InitSlot<'_, u64>| -> Result<(), ()> {
///     let _ = Box::<u64>::try_init_in(Unmetered, |inner: &mut InitSlot<'_, u64>| -> Result<(), ()> {
///         core::mem::swap(outer, inner);
///         Ok(())
///     });
///     Ok(())
/// });
/// ```
mod separate_slot_contract {}

mod owned;
mod slice;
mod string;
pub use slice::IntoIter;
mod pinned;
mod traits;
pub use pinned::Pinned;
mod call;
#[doc(hidden)]
pub use call::{CallRelease, StandardCallOwner};

/// Strict dynamic owners do not implicitly expose indexing or escape pinning.
/// ```compile_fail
/// use jzap::boxed::Box;
/// let slice = Box::<[u8]>::try_copy_from_slice(&[1]).unwrap();
/// let _ = slice[3];
/// ```
/// ```compile_fail
/// use jzap::{boxed::Box, init::InitSlot};
/// struct S { a: u64, b: u64 }
/// let _ = Box::<S>::try_init_in(jzap::budget::Unmetered, |slot: &mut InitSlot<'_, S>| {
///     jzap::try_init_struct!(slot, S { a: |f: &mut InitSlot<'_, u64>| f.write(1).map_err(|e| e.error) })
/// });
/// ```
/// ```compile_fail
/// use jzap::boxed::Box;
/// let boxed = Box::try_pin(std::marker::PhantomPinned).ok().unwrap();
/// let _ = boxed.into_inner();
/// ```
/// ```compile_fail
/// use jzap::boxed::Box;
/// fn escape<'a>(text: &'a str) -> Box<dyn std::fmt::Display + 'static> {
///     jzap::box_unsize!(Box::try_new(text).unwrap() => dyn std::fmt::Display + 'static)
/// }
/// ```
mod dynamic_contracts {}
mod text;

mod buffer;
pub(crate) mod buffer_numeric;
mod foreign;

pub use foreign::CStrError;
