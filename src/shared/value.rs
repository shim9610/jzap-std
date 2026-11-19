//! Sealed payload shapes and transactional cloning into final shared storage.
use super::*;
mod sealed {
    pub trait Sealed {}
    impl<T> Sealed for T {}
    impl<T> Sealed for [T] {}
    impl Sealed for str {}
    impl Sealed for core::ffi::CStr {}
    #[cfg(feature = "std")]
    impl Sealed for std::ffi::OsStr {}
    #[cfg(feature = "std")]
    impl Sealed for std::path::Path {}
}
#[doc(hidden)]
pub trait Shape: sealed::Sealed {
    fn compatible(&self) -> bool {
        true
    }
    /// # Safety
    /// base is a header initialized by Allocation with this value's layout.
    unsafe fn control<C: Counter, P: SharedPolicy>(
        &self,
        base: NonNull<u8>,
    ) -> NonNull<Control<C, P, Self>>;
}
impl<T> Shape for T {
    unsafe fn control<C: Counter, P: SharedPolicy>(
        &self,
        base: NonNull<u8>,
    ) -> NonNull<Control<C, P, T>> {
        base.cast()
    }
}
impl<T> Shape for [T] {
    unsafe fn control<C: Counter, P: SharedPolicy>(
        &self,
        base: NonNull<u8>,
    ) -> NonNull<Control<C, P, [T]>> {
        let p = NonNull::slice_from_raw_parts(base.cast::<T>(), self.len());
        // SAFETY: identical slice-length metadata, correct extended allocation.
        unsafe { NonNull::new_unchecked(p.as_ptr() as *mut Control<C, P, [T]>) }
    }
}
impl Shape for str {
    unsafe fn control<C: Counter, P: SharedPolicy>(
        &self,
        base: NonNull<u8>,
    ) -> NonNull<Control<C, P, str>> {
        let p = NonNull::slice_from_raw_parts(base, self.len());
        // SAFETY: str and byte-slice metadata are the same length.
        unsafe { NonNull::new_unchecked(p.as_ptr() as *mut Control<C, P, str>) }
    }
}
#[doc(hidden)]
pub trait CloneValue: Shape {
    /// # Safety
    /// destination is exclusive uninitialized storage of Self's exact layout.
    /// On Err no initialized values remain in destination.
    unsafe fn clone_into(&self, destination: *mut Self) -> Result<(), Error>;
}
impl<T: TryClone> CloneValue for T {
    unsafe fn clone_into(&self, destination: *mut T) -> Result<(), Error> {
        // SAFETY: inherited exclusive final-storage contract.
        let mut slot = unsafe { crate::init::InitSlot::new(NonNull::new_unchecked(destination)) };
        self.try_clone_into(&mut slot)?;
        slot.finish()
    }
}
impl<T: TryClone> CloneValue for [T] {
    unsafe fn clone_into(&self, destination: *mut Self) -> Result<(), Error> {
        // SAFETY: inherited exclusive final-storage contract for self.len() elements.
        let ptr = unsafe { NonNull::new_unchecked(destination.cast::<T>()) };
        let mut initialize = |index: usize, slot: &mut crate::init::InitSlot<'_, T>| {
            // SAFETY: the prefix driver supplies only indices below self.len().
            unsafe { self.get_unchecked(index) }.try_clone_into(slot)
        };
        // SAFETY: as above; a completed prefix is released on failure.
        unsafe { crate::init::prefix::initialize(ptr, self.len(), &mut initialize) }.map_err(|e| {
            match e {
                InitError::Storage(e) | InitError::Initialize(e) => e,
            }
        })
    }
}
impl CloneValue for str {
    unsafe fn clone_into(&self, destination: *mut Self) -> Result<(), Error> {
        // SAFETY: exact disjoint destination storage, and source is valid UTF-8.
        unsafe {
            core::ptr::copy_nonoverlapping(self.as_ptr(), destination.cast::<u8>(), self.len());
        }
        Ok(())
    }
}
impl<T: ?Sized + CloneValue, C: Counter, P: SharedPolicy> Strong<T, C, P> {
    pub fn try_clone_payload_with<Q: SharedPolicy>(
        this: &Self,
        policy: Q,
    ) -> Result<Strong<T, C, Q>, Error> {
        if !this.as_ref().compatible() {
            return Err(Error::Layout);
        }
        let a = Allocation::<C, Q>::new(Layout::for_value(this.as_ref()), policy, 1, false)?;
        // SAFETY: exact shape/layout and exclusively owned destination.
        let ptr = unsafe { this.as_ref().control::<C, Q>(a.ptr) };
        unsafe {
            this.as_ref().clone_into(data(ptr))?;
        }
        Ok(a.finish(ptr))
    }
    #[inline(always)]
    pub fn try_make_mut(this: &mut Self) -> Result<&mut T, Error> {
        let (strong, weak) = unsafe { counts(this.ptr) };
        // Avoid a locked uniqueness probe when a visible alias already
        // requires replacement. The final uniqueness protocol still rechecks
        // both counters before exposing a mutable reference.
        if strong.load() == 1 && weak.load() == 1 && C::unique(strong, weak) {
            // SAFETY: uniqueness protocol excludes all other access to T.
            return Ok(unsafe { &mut *data(this.ptr) });
        }
        if !this.as_ref().compatible() {
            return Err(Error::Layout);
        }
        let old = this.ptr;
        let layout = Layout::for_value(this.as_ref());
        let a =
            Allocation::<C, P>::new(layout, Self::allocation_policy(this).try_clone()?, 1, false)?;
        let ptr = unsafe { this.as_ref().control::<C, P>(a.ptr) };
        if unsafe { counts(old).0.take_last() } {
            // SAFETY: the old payload is still initialized and its full layout
            // fits the low bits of the retired count word.
            unsafe {
                retire(old);
            }
            // SAFETY: exclusive retirement won over weak upgrades. Reserve
            // succeeded first; moving T cannot fail, and old keeps only weaks.
            unsafe {
                core::ptr::copy_nonoverlapping(data(old).cast::<u8>(), a.data(), layout.size());
            }
            this.ptr = ptr;
            core::mem::forget(a);
            drop(WeakRelease(old));
        } else {
            // SAFETY: our strong credit still keeps source initialized. Failure
            // cleans partial destination before the allocation guard runs.
            unsafe {
                this.as_ref().clone_into(data(ptr))?;
            }
            *this = a.finish(ptr);
        }
        // SAFETY: replacement allocation has exactly one strong/implicit weak.
        Ok(unsafe { &mut *data(this.ptr) })
    }
}
impl<T, C: Counter, P: SharedPolicy> Strong<MaybeUninit<T>, C, P> {
    /// # Safety
    /// The payload is fully initialized as a valid T and aliases respect T.
    pub unsafe fn assume_init(this: Self) -> Strong<T, C, P> {
        let ptr = Self::into_raw(this).cast::<Control<C, P, T>>();
        // SAFETY: same layout/receipt, initialization guaranteed by caller.
        unsafe { Strong::from_raw(ptr) }
    }
}
impl<T, C: Counter> Strong<[T], C> {
    pub fn try_new_uninit_slice(len: usize) -> Result<Strong<[MaybeUninit<T>], C>, Error> {
        Self::try_new_uninit_slice_in(len, Unmetered)
    }
    pub fn try_new_zeroed_slice(len: usize) -> Result<Strong<[MaybeUninit<T>], C>, Error> {
        Self::try_new_zeroed_slice_in(len, Unmetered)
    }
    pub fn try_from_slice(source: &[T]) -> Result<Self, Error>
    where
        T: TryClone,
    {
        Self::try_from_slice_in(source, Unmetered)
    }
    pub fn try_copy_from_slice(source: &[T]) -> Result<Self, Error>
    where
        T: Copy,
    {
        Self::try_copy_from_slice_in(source, Unmetered)
    }
    pub fn try_from_iter(source: impl IntoIterator<Item = T>) -> Result<Self, Error> {
        Self::try_from_iter_in(source, Unmetered)
    }
}
impl<T, C: Counter, P: SharedPolicy> Strong<[T], C, P> {
    fn uninit_slice(
        len: usize,
        policy: P,
        zeroed: bool,
    ) -> Result<Strong<[MaybeUninit<T>], C, P>, Error> {
        let layout = crate::layout::array::<T>(len)?;
        let a = Allocation::<C, P>::new(layout, policy, 1, zeroed)?;
        let raw = NonNull::slice_from_raw_parts(a.ptr.cast::<MaybeUninit<T>>(), len);
        // SAFETY: exact slice metadata and checked header/payload layout.
        let ptr =
            unsafe { NonNull::new_unchecked(raw.as_ptr() as *mut Control<C, P, [MaybeUninit<T>]>) };
        Ok(a.finish(ptr))
    }
    pub fn try_new_uninit_slice_in(
        len: usize,
        policy: P,
    ) -> Result<Strong<[MaybeUninit<T>], C, P>, Error> {
        Self::uninit_slice(len, policy, false)
    }
    pub fn try_new_zeroed_slice_in(
        len: usize,
        policy: P,
    ) -> Result<Strong<[MaybeUninit<T>], C, P>, Error> {
        Self::uninit_slice(len, policy, true)
    }
    pub fn try_init_slice_in<E>(
        len: usize,
        policy: P,
        mut initialize: impl FnMut(usize, &mut crate::init::InitSlot<'_, T>) -> Result<(), E>,
    ) -> Result<Self, InitError<E>> {
        let owner = Self::try_new_uninit_slice_in(len, policy).map_err(InitError::Storage)?;
        // SAFETY: the payload of a live allocation is non-null, aligned storage
        // for len uninitialized elements owned by `owner` until completion.
        let ptr = unsafe { NonNull::new_unchecked(data(owner.ptr).cast::<T>()) };
        // SAFETY: as above; a completed prefix is released before `owner`.
        unsafe { crate::init::prefix::initialize(ptr, len, &mut initialize) }?;
        // SAFETY: every element has recorded initialization, including ZSTs.
        Ok(unsafe { Strong::<[MaybeUninit<T>], C, P>::assume_init(owner) })
    }
    pub fn try_from_slice_in(source: &[T], policy: P) -> Result<Self, Error>
    where
        T: TryClone,
    {
        let a = Allocation::<C, P>::new(Layout::for_value(source), policy, 1, false)?;
        let ptr = unsafe { source.control::<C, P>(a.ptr) };
        unsafe {
            source.clone_into(data(ptr))?;
        }
        Ok(a.finish(ptr))
    }
    pub fn try_copy_from_slice_in(source: &[T], policy: P) -> Result<Self, Error>
    where
        T: Copy,
    {
        let a = Allocation::<C, P>::new(Layout::for_value(source), policy, 1, false)?;
        let ptr = unsafe { source.control::<C, P>(a.ptr) };
        // SAFETY: same checked layout and disjoint initialized Copy source.
        unsafe {
            core::ptr::copy_nonoverlapping(source.as_ptr(), a.data().cast::<T>(), source.len());
        }
        Ok(a.finish(ptr))
    }
    pub fn try_from_vec_in<L: crate::vec::LengthPolicy>(
        mut source: crate::vec::Vec<T, P, L>,
    ) -> Result<Self, Rejected<crate::vec::Vec<T, P, L>>> {
        let policy = match source.allocation_policy().try_clone() {
            Ok(policy) => policy,
            Err(error) => {
                return Err(Rejected {
                    value: source,
                    error,
                });
            }
        };
        let a =
            match Allocation::<C, P>::new(Layout::for_value(source.as_slice()), policy, 1, false) {
                Ok(a) => a,
                Err(error) => {
                    return Err(Rejected {
                        value: source,
                        error,
                    });
                }
            };
        let ptr = unsafe { source.as_slice().control::<C, P>(a.ptr) };
        // SAFETY: source relinquishes its complete initialized prefix after the
        // move; its empty storage owner still releases its own allocation/charge.
        unsafe {
            core::ptr::copy_nonoverlapping(source.as_ptr(), a.data().cast::<T>(), source.len());
            source.try_set_len(0).map_err(|error| Rejected {
                value: source,
                error,
            })?;
        }
        Ok(a.finish(ptr))
    }
    pub fn try_from_fallible_iter_in<E>(
        source: impl crate::TryIntoIterator<Item = T, Error = E>,
        policy: P,
    ) -> Result<Self, InitError<E>> {
        let values =
            crate::vec::Vec::try_from_fallible_iter_in(source, policy, crate::vec::Unlimited)?;
        Self::try_from_vec_in(values).map_err(|e| InitError::Storage(e.error))
    }
    pub fn try_from_iter_in(source: impl IntoIterator<Item = T>, policy: P) -> Result<Self, Error> {
        Self::try_from_fallible_iter_in(crate::iter::from_std(source), policy).map_err(
            |e| match e {
                InitError::Storage(e) => e,
                InitError::Initialize(n) => match n {},
            },
        )
    }
    pub fn as_slice(&self) -> &[T] {
        self.as_ref()
    }
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }
    pub fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        self.as_ref().get(index)
    }
}
impl<T, C: Counter, P: SharedPolicy> Strong<[MaybeUninit<T>], C, P> {
    /// # Safety
    /// Every element represents a valid initialized T; aliases respect T.
    pub unsafe fn assume_init(this: Self) -> Strong<[T], C, P> {
        let raw = Self::into_raw(this);
        let ptr = unsafe { NonNull::new_unchecked(raw.as_ptr() as *mut Control<C, P, [T]>) };
        unsafe { Strong::from_raw(ptr) }
    }
}
impl<C: Counter> Strong<str, C> {
    pub fn try_from_str(source: &str) -> Result<Self, Error> {
        Self::try_from_str_in(source, Unmetered)
    }
}
impl<C: Counter, P: SharedPolicy> Strong<str, C, P> {
    pub fn try_from_str_in(source: &str, policy: P) -> Result<Self, Error> {
        let a = Allocation::<C, P>::new(Layout::for_value(source), policy, 1, false)?;
        let ptr = unsafe { source.control::<C, P>(a.ptr) };
        unsafe {
            source.clone_into(data(ptr))?;
        }
        Ok(a.finish(ptr))
    }
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }
    pub fn as_bytes(&self) -> &[u8] {
        self.as_ref().as_bytes()
    }
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }
    pub fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }
}

impl<T, C: Counter, P: SharedPolicy> Strong<T, C, P> {
    /// Deep-copy newly owned payload storage into the target local budget.
    /// Shared handles nested inside T retain their original allocation budget.
    pub fn try_deep_clone_in<'a>(
        this: &Self,
        budget: Budget<'a>,
    ) -> Result<Strong<T::Output, C, Budget<'a>>, Error>
    where
        T: TryCloneIn<'a>,
    {
        Strong::try_init_in(budget, |slot: &mut crate::init::InitSlot<'_, T::Output>| {
            this.as_ref().try_clone_into_in(budget, slot)
        })
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}
impl<T, C: Counter, P: SharedPolicy> Strong<[T], C, P> {
    pub fn try_deep_clone_in<'a>(
        this: &Self,
        budget: Budget<'a>,
    ) -> Result<Strong<[T::Output], C, Budget<'a>>, Error>
    where
        T: TryCloneIn<'a>,
    {
        Strong::try_init_slice_in(this.len(), budget, |i, slot| {
            this.as_slice()
                .get(i)
                .ok_or(Error::Bounds)?
                .try_clone_into_in(budget, slot)
        })
        .map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}
impl<C: Counter, P: SharedPolicy> Strong<str, C, P> {
    pub fn try_deep_clone_in<'a>(
        this: &Self,
        budget: Budget<'a>,
    ) -> Result<Strong<str, C, Budget<'a>>, Error> {
        Strong::try_from_str_in(this.as_str(), budget)
    }
}

impl Shape for core::ffi::CStr {
    fn compatible(&self) -> bool {
        let bytes = self.to_bytes_with_nul();
        Layout::for_value(self) == Layout::for_value(bytes)
            && (self as *const Self as *const [u8]).len() == bytes.len()
            && (self as *const Self).cast::<u8>() == bytes.as_ptr()
    }
    unsafe fn control<C: Counter, P: SharedPolicy>(
        &self,
        base: NonNull<u8>,
    ) -> NonNull<Control<C, P, Self>> {
        let p = NonNull::slice_from_raw_parts(base, self.to_bytes_with_nul().len());
        // SAFETY: caller checks layout, data address and byte-length metadata.
        unsafe { NonNull::new_unchecked(p.as_ptr() as *mut Control<C, P, Self>) }
    }
}
impl CloneValue for core::ffi::CStr {
    unsafe fn clone_into(&self, dst: *mut Self) -> Result<(), Error> {
        // SAFETY: exact copy of a complete validated null-terminated representation.
        unsafe {
            core::ptr::copy_nonoverlapping(
                (self as *const Self).cast::<u8>(),
                dst.cast::<u8>(),
                Layout::for_value(self).size(),
            );
        }
        Ok(())
    }
}
impl<C: Counter> crate::TryDefault for Strong<core::ffi::CStr, C> {
    fn try_default() -> Result<Self, Error> {
        Self::try_from_ref(c"")
    }
}
#[cfg(feature = "std")]
macro_rules! native_shape {
    ($ty:ty,$bytes:expr) => {
        impl Shape for $ty {
            fn compatible(&self) -> bool {
                let bytes = ($bytes)(self);
                Layout::for_value(self) == Layout::for_value(bytes)
                    && (self as *const Self as *const [u8]).len() == bytes.len()
                    && (self as *const Self).cast::<u8>() == bytes.as_ptr()
            }
            unsafe fn control<C: Counter, P: SharedPolicy>(
                &self,
                base: NonNull<u8>,
            ) -> NonNull<Control<C, P, Self>> {
                let p = NonNull::slice_from_raw_parts(base, ($bytes)(self).len());
                // SAFETY: caller checks native layout/data/metadata compatibility first.
                unsafe { NonNull::new_unchecked(p.as_ptr() as *mut Control<C, P, Self>) }
            }
        }
        impl CloneValue for $ty {
            unsafe fn clone_into(&self, dst: *mut Self) -> Result<(), Error> {
                // SAFETY: complete native representation copied within the same runtime.
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        (self as *const Self).cast::<u8>(),
                        dst.cast::<u8>(),
                        Layout::for_value(self).size(),
                    );
                }
                Ok(())
            }
        }
    };
}
#[cfg(feature = "std")]
fn os_bytes(s: &std::ffi::OsStr) -> &[u8] {
    s.as_encoded_bytes()
}
#[cfg(feature = "std")]
fn path_bytes(s: &std::path::Path) -> &[u8] {
    s.as_os_str().as_encoded_bytes()
}
#[cfg(feature = "std")]
native_shape!(std::ffi::OsStr, os_bytes);
#[cfg(feature = "std")]
native_shape!(std::path::Path, path_bytes);
#[cfg(feature = "std")]
impl<C: Counter> crate::TryDefault for Strong<std::ffi::OsStr, C> {
    fn try_default() -> Result<Self, Error> {
        Self::try_from_ref(std::ffi::OsStr::new(""))
    }
}
#[cfg(feature = "std")]
impl<C: Counter> crate::TryDefault for Strong<std::path::Path, C> {
    fn try_default() -> Result<Self, Error> {
        Self::try_from_ref(std::path::Path::new(""))
    }
}
