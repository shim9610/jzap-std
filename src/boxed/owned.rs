//! Explicit ownership and borrowing; no implicit allocation.
use super::*;
use core::{
    borrow::{Borrow, BorrowMut},
    mem::{ManuallyDrop, MaybeUninit},
};

impl<T: ?Sized, P: AllocationPolicy> Box<T, P> {
    pub fn allocation_layout(this: &Self) -> Layout {
        Layout::for_value(this.as_ref())
    }
    /// Return the pointer and its accounting policy without deallocation.
    /// Keeping only the pointer deliberately leaks its budget charge.
    pub fn into_raw_parts(this: Self) -> (NonNull<T>, P) {
        let this = ManuallyDrop::new(this);
        // SAFETY: this owner is suppressed; its policy is moved exactly once.
        (this.ptr, unsafe { core::ptr::read(&this.policy) })
    }
    /// Reconstitute an owner without allocating.
    ///
    /// # Safety
    /// `ptr` must uniquely own initialized T in a global allocation with exactly
    /// Layout::for_value(T), or aligned non-null storage for a zero-sized T.
    /// Its metadata must be valid. `policy` must own the existing charge for the
    /// full allocation; no other owner may drop, deallocate or release it.
    pub unsafe fn from_raw_parts(ptr: NonNull<T>, policy: P) -> Self {
        Self {
            ptr,
            policy,
            marker: PhantomData,
        }
    }
    pub fn as_ptr(this: &Self) -> *const T {
        this.ptr.as_ptr()
    }
    pub fn as_mut_ptr(this: &mut Self) -> *mut T {
        this.ptr.as_ptr()
    }
    pub fn policy(this: &Self) -> &P {
        &this.policy
    }
    /// Deliberately leak both the allocation and its accounting charge.
    pub fn leak<'a>(this: Self) -> &'a mut T
    where
        T: 'a,
        P: 'a,
    {
        let (mut ptr, policy) = Self::into_raw_parts(this);
        core::mem::forget(policy);
        // SAFETY: owner consumed, no other access remains; lifetime is bounded by T/P.
        unsafe { ptr.as_mut() }
    }
}
impl<T: ?Sized, P: AllocationPolicy> AsRef<T> for Box<T, P> {
    fn as_ref(&self) -> &T {
        // SAFETY: the owner holds a fully initialized T for the borrow duration.
        unsafe { self.ptr.as_ref() }
    }
}
impl<T: ?Sized, P: AllocationPolicy> AsMut<T> for Box<T, P> {
    fn as_mut(&mut self) -> &mut T {
        // SAFETY: exclusive owner borrow.
        unsafe { self.ptr.as_mut() }
    }
}
impl<T: ?Sized, P: AllocationPolicy> Borrow<T> for Box<T, P> {
    fn borrow(&self) -> &T {
        self.as_ref()
    }
}
impl<T: ?Sized, P: AllocationPolicy> BorrowMut<T> for Box<T, P> {
    fn borrow_mut(&mut self) -> &mut T {
        self.as_mut()
    }
}
impl<T, P: AllocationPolicy> Box<T, P> {
    pub fn into_inner(this: Self) -> T {
        let (ptr, policy) = Self::into_raw_parts(this);
        let _storage = AllocationGuard(ptr.cast(), Layout::new::<T>(), policy.budget());
        // SAFETY: uniquely owned initialized T; only storage is released afterwards.
        unsafe { ptr.as_ptr().read() }
    }
    pub fn into_boxed_slice(this: Self) -> Box<[T], P> {
        let (ptr, policy) = Self::into_raw_parts(this);
        // SAFETY: a one-element slice has the same layout and owned initialized bytes.
        unsafe { Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr, 1), policy) }
    }
    pub fn take(this: Self) -> (T, Box<MaybeUninit<T>, P>) {
        let (ptr, policy) = Self::into_raw_parts(this);
        // SAFETY: read transfers T; allocation/charge are retained as uninitialized storage.
        unsafe { (ptr.as_ptr().read(), Box::from_raw_parts(ptr.cast(), policy)) }
    }
    pub fn try_new_uninit_in(policy: P) -> Result<Box<MaybeUninit<T>, P>, crate::Error> {
        let ptr = storage::allocate(Layout::new::<T>(), policy.budget())?;
        // SAFETY: MaybeUninit<T> permits uninitialized bytes, and has T's layout.
        Ok(unsafe { Box::from_raw_parts(ptr.cast(), policy) })
    }
    pub fn try_new_zeroed_in(policy: P) -> Result<Box<MaybeUninit<T>, P>, crate::Error> {
        let ptr = storage::allocate_zeroed(Layout::new::<T>(), policy.budget())?;
        // SAFETY: zeroed bytes are valid for MaybeUninit<T>, even if not valid for T.
        Ok(unsafe { Box::from_raw_parts(ptr.cast(), policy) })
    }
}
impl<T> Box<T> {
    pub fn try_new_uninit() -> Result<Box<MaybeUninit<T>>, crate::Error> {
        Self::try_new_uninit_in(Unmetered)
    }
    pub fn try_new_zeroed() -> Result<Box<MaybeUninit<T>>, crate::Error> {
        Self::try_new_zeroed_in(Unmetered)
    }
}
impl<T, P: AllocationPolicy> Box<MaybeUninit<T>, P> {
    /// # Safety
    /// The stored bytes must represent a fully initialized, valid T.
    pub unsafe fn assume_init(this: Self) -> Box<T, P> {
        let (ptr, policy) = Self::into_raw_parts(this);
        // SAFETY: inherited initialization contract; unchanged allocation and charge.
        unsafe { Box::from_raw_parts(ptr.cast(), policy) }
    }
    pub fn write(this: Self, value: T) -> Box<T, P> {
        let (ptr, policy) = Self::into_raw_parts(this);
        // SAFETY: the uniquely owned allocation is suitable for T, initialized once.
        unsafe {
            ptr.cast::<T>().as_ptr().write(value);
            Box::from_raw_parts(ptr.cast(), policy)
        }
    }
}
impl<T: ?Sized> Box<T> {
    pub fn into_raw(this: Self) -> *mut T {
        Self::into_raw_parts(this).0.as_ptr()
    }
    /// # Safety
    /// Same storage and ownership conditions as from_raw_parts, without a budget charge.
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        // SAFETY: caller guarantees a non-null valid owning pointer.
        unsafe { Self::from_raw_parts(NonNull::new_unchecked(ptr), Unmetered) }
    }
    /// Explicitly transfer to the standard owner's API, without allocation.
    pub fn into_std(this: Self) -> alloc::boxed::Box<T> {
        // SAFETY: identical global allocation layout; ownership transferred once.
        unsafe { alloc::boxed::Box::from_raw(Self::into_raw(this)) }
    }
    /// Adopt a standard global allocation without allocating or copying its payload.
    pub fn from_std(this: alloc::boxed::Box<T>) -> Self {
        // SAFETY: standard owner supplies initialized, uniquely owned global storage.
        unsafe { Self::from_raw(alloc::boxed::Box::into_raw(this)) }
    }
}

/// Explicit, allocation-free unsizing (array to slice or concrete to trait object).
/// The compiler checks a raw-pointer coercion; arbitrary pointer casts are not used.
#[macro_export]
macro_rules! box_unsize {
    ($value:expr => $target:ty) => {{
        let (__ptr, __policy) = $crate::boxed::Box::into_raw_parts($value);
        let __coerced: *mut $target = __ptr.as_ptr();
        // SAFETY: compiler-checked unsizing retains data address, ownership, layout
        // and accounting. Metadata is provided by the coercion, not by user code.
        unsafe {
            $crate::boxed::Box::from_raw_parts(
                core::ptr::NonNull::new_unchecked(__coerced),
                __policy,
            )
        }
    }};
}

impl<T: crate::TryDefault, P: AllocationPolicy> Box<T, P> {
    pub fn try_default_in(policy: P) -> Result<Self, crate::Error> {
        Self::try_init_in(policy, T::try_default_into).map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}
impl<T: crate::TryDefault> crate::TryDefault for Box<T> {
    fn try_default() -> Result<Self, crate::Error> {
        Self::try_default_in(Unmetered)
    }
}
impl<T: ?Sized, P: AllocationPolicy> Box<T, P> {
    /// Adopt a global standard Box after accepting its full existing allocation
    /// into the target budget. Refusal returns the unchanged standard owner.
    pub fn try_from_std_in(
        value: alloc::boxed::Box<T>,
        policy: P,
    ) -> Result<Self, Rejected<alloc::boxed::Box<T>>> {
        let layout = Layout::for_value(value.as_ref());
        {
            let reservation = match policy.budget().reserve(layout.size()) {
                Ok(r) => r,
                Err(error) => return Err(Rejected { value, error }),
            };
            if let Some(r) = reservation {
                r.commit().retain();
            }
        }
        // SAFETY: transferred initialized global allocation with matching charge.
        Ok(unsafe {
            Self::from_raw_parts(
                NonNull::new_unchecked(alloc::boxed::Box::into_raw(value)),
                policy,
            )
        })
    }
}
