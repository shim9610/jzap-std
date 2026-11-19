//! Explicit fallible equivalents of standard shared-owner conversions.
use super::*;
use value::{CloneValue, Shape};

/// Preserve source metadata while replacing only the data-pointer component.
/// Reference: portable-atomic-util 0.2.8 utils::ptr::with_metadata_of.
/// # Safety
/// The pinned Rust ABI stores a wide pointer's data component first. The matrix
/// exercises this on supported targets. base is the exact allocated control
/// layout for source, whose metadata remains valid. No payload reference forms.
unsafe fn control_from_value<T: ?Sized, C: Counter, P: SharedPolicy>(
    source: &T,
    base: NonNull<u8>,
) -> Result<NonNull<Control<C, P, T>>, Error> {
    let original =
        (core::ptr::from_ref(source) as *mut Control<C, P, T>).with_addr(base.as_ptr().addr() ^ 1);
    let mut result = original;
    // SAFETY: a pointer's storage covers its thin data component. Write a
    // pointer, not an integer address, to retain the new allocation's provenance.
    unsafe {
        core::ptr::addr_of_mut!(result)
            .cast::<*mut u8>()
            .write(base.as_ptr());
    }
    if result.addr() != base.as_ptr().addr()
        || !core::ptr::eq(result.with_addr(original.addr()), original)
    {
        return Err(Error::Layout);
    }
    NonNull::new(result).ok_or(Error::Layout)
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> Strong<T, C, P> {
    pub fn try_from_box_in(
        value: crate::boxed::Box<T, P>,
    ) -> Result<Self, Rejected<crate::boxed::Box<T, P>>> {
        let policy = match crate::boxed::Box::policy(&value).try_clone() {
            Ok(policy) => policy,
            Err(error) => {
                return Err(Rejected { value, error });
            }
        };
        Self::try_from_box_with(value, policy)
    }
    pub fn try_from_box_with<Q: SharedPolicy>(
        value: crate::boxed::Box<T, Q>,
        policy: P,
    ) -> Result<Self, Rejected<crate::boxed::Box<T, Q>>> {
        let layout = Layout::for_value(value.as_ref());
        let a = match Allocation::<C, P>::new(layout, policy, 1, false) {
            Ok(a) => a,
            Err(error) => return Err(Rejected { value, error }),
        };
        // SAFETY: initialized source supplies metadata; checked extended storage
        // supplies the new pointer provenance. Failure preserves the source.
        let ptr = match unsafe { control_from_value::<T, C, P>(value.as_ref(), a.ptr) } {
            Ok(p) => p,
            Err(error) => return Err(Rejected { value, error }),
        };
        let (source, source_policy) = crate::boxed::Box::into_raw_parts(value);
        // SAFETY: complete disjoint payload move; old allocation is released
        // without dropping the values now owned by the new control block.
        unsafe {
            core::ptr::copy_nonoverlapping(source.as_ptr().cast::<u8>(), a.data(), layout.size());
            crate::storage::deallocate(source.cast(), layout, source_policy.budget());
        }
        Ok(a.finish(ptr))
    }
    pub fn try_from_std_box_in(
        value: alloc::boxed::Box<T>,
        policy: P,
    ) -> Result<Self, Rejected<alloc::boxed::Box<T>>> {
        Self::try_from_box_with(crate::boxed::Box::from_std(value), policy).map_err(|e| Rejected {
            value: crate::boxed::Box::into_std(e.value),
            error: e.error,
        })
    }
}
impl<T: ?Sized, C: Counter> Strong<T, C> {
    pub fn try_from_std_box(
        value: alloc::boxed::Box<T>,
    ) -> Result<Self, Rejected<alloc::boxed::Box<T>>> {
        Self::try_from_std_box_in(value, Unmetered)
    }
}
impl<T, C: Counter, P: SharedPolicy> Strong<[T], C, P> {
    pub fn try_into_array<const N: usize>(this: Self) -> Result<Strong<[T; N], C, P>, Self> {
        if this.len() != N {
            return Err(this);
        }
        let ptr = Self::into_raw(this).cast::<Control<C, P, [T; N]>>();
        // SAFETY: checked length and identical array/slice payload layout.
        Ok(unsafe { Strong::from_raw(ptr) })
    }
    pub fn try_from_array_in<const N: usize>(
        value: [T; N],
        policy: P,
    ) -> Result<Self, Rejected<[T; N]>> {
        Strong::<[T; N], C, P>::try_new_in(value, policy).map(Strong::into_slice)
    }
    pub fn try_from_std_vec_in(
        mut value: alloc::vec::Vec<T>,
        policy: P,
    ) -> Result<Self, Rejected<alloc::vec::Vec<T>>> {
        let a = match Allocation::<C, P>::new(Layout::for_value(value.as_slice()), policy, 1, false)
        {
            Ok(a) => a,
            Err(error) => return Err(Rejected { value, error }),
        };
        let ptr = unsafe { value.as_slice().control::<C, P>(a.ptr) };
        // SAFETY: exact disjoint initialized prefix; source relinquishes its
        // elements before its standard storage allocation is destroyed.
        unsafe {
            core::ptr::copy_nonoverlapping(value.as_ptr(), a.data().cast::<T>(), value.len());
            value.set_len(0);
        }
        Ok(a.finish(ptr))
    }
}
impl<T, const N: usize, C: Counter, P: SharedPolicy> Strong<[T; N], C, P> {
    pub fn into_slice(this: Self) -> Strong<[T], C, P> {
        let raw = Self::into_raw(this);
        let pointer: *mut Control<C, P, [T]> = raw.as_ptr();
        // SAFETY: compiler-checked array unsizing preserves layout and credits.
        unsafe { Strong::from_raw(NonNull::new_unchecked(pointer)) }
    }
}
impl<T, C: Counter> Strong<[T], C> {
    pub fn try_from_array<const N: usize>(value: [T; N]) -> Result<Self, Rejected<[T; N]>> {
        Self::try_from_array_in(value, Unmetered)
    }
    pub fn try_from_std_vec(
        value: alloc::vec::Vec<T>,
    ) -> Result<Self, Rejected<alloc::vec::Vec<T>>> {
        Self::try_from_std_vec_in(value, Unmetered)
    }
}
impl<C: Counter, P: SharedPolicy> Strong<str, C, P> {
    pub fn into_bytes(this: Self) -> Strong<[u8], C, P> {
        let raw = Self::into_raw(this);
        // SAFETY: str and byte-slice metadata/layout match. Shared/weak aliases
        // continue to exclude mutable access to the original bytes.
        unsafe {
            Strong::from_raw(NonNull::new_unchecked(
                raw.as_ptr() as *mut Control<C, P, [u8]>
            ))
        }
    }
    pub fn try_from_std_string_in(
        value: alloc::string::String,
        policy: P,
    ) -> Result<Self, Rejected<alloc::string::String>> {
        match Self::try_from_str_in(value.as_str(), policy) {
            Ok(a) => Ok(a),
            Err(error) => Err(Rejected { value, error }),
        }
    }
    pub fn try_from_string_in<L: crate::vec::LengthPolicy>(
        value: crate::string::String<P, L>,
    ) -> Result<Self, Rejected<crate::string::String<P, L>>> {
        let policy = match value.allocation_policy().try_clone() {
            Ok(policy) => policy,
            Err(error) => {
                return Err(Rejected { value, error });
            }
        };
        match Self::try_from_str_in(value.as_str(), policy) {
            Ok(a) => Ok(a),
            Err(error) => Err(Rejected { value, error }),
        }
    }
}
impl<C: Counter> Strong<str, C> {
    pub fn try_from_std_string(
        value: alloc::string::String,
    ) -> Result<Self, Rejected<alloc::string::String>> {
        Self::try_from_std_string_in(value, Unmetered)
    }
}
impl<T: ?Sized + CloneValue, C: Counter, P: SharedPolicy> Strong<T, C, P> {
    pub fn try_from_ref_in(source: &T, policy: P) -> Result<Self, Error> {
        if !source.compatible() {
            return Err(Error::Layout);
        }
        let a = Allocation::<C, P>::new(Layout::for_value(source), policy, 1, false)?;
        let ptr = unsafe { source.control::<C, P>(a.ptr) };
        unsafe {
            source.clone_into(data(ptr))?;
        }
        Ok(a.finish(ptr))
    }
    pub fn try_from_cow_in<'a>(
        value: alloc::borrow::Cow<'a, T>,
        policy: P,
    ) -> Result<Self, Rejected<alloc::borrow::Cow<'a, T>>>
    where
        T: alloc::borrow::ToOwned,
    {
        match Self::try_from_ref_in(value.as_ref(), policy) {
            Ok(a) => Ok(a),
            Err(error) => Err(Rejected { value, error }),
        }
    }
}
impl<T: ?Sized + CloneValue, C: Counter> Strong<T, C> {
    pub fn try_from_ref(source: &T) -> Result<Self, Error> {
        Self::try_from_ref_in(source, Unmetered)
    }
    pub fn try_from_cow<'a>(
        value: alloc::borrow::Cow<'a, T>,
    ) -> Result<Self, Rejected<alloc::borrow::Cow<'a, T>>>
    where
        T: alloc::borrow::ToOwned,
    {
        Self::try_from_cow_in(value, Unmetered)
    }
}
impl<T, C: Counter> crate::TryDefault for Strong<[T], C> {
    fn try_default() -> Result<Self, Error> {
        let empty = Self::try_new_uninit_slice(0)?;
        Ok(unsafe { Strong::<[MaybeUninit<T>], C>::assume_init(empty) })
    }
}
impl<C: Counter> crate::TryDefault for Strong<str, C> {
    fn try_default() -> Result<Self, Error> {
        Self::try_from_str("")
    }
}
impl<T: crate::TryDefault, C: Counter> crate::TryDefault for Pinned<T, C> {
    fn try_default() -> Result<Self, Error> {
        Strong::try_pin_init_in(Unmetered, T::try_default_into).map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}
impl<T: ?Sized + core::error::Error, C: Counter, P: SharedPolicy> core::error::Error
    for Strong<T, C, P>
{
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        self.as_ref().source()
    }
}
