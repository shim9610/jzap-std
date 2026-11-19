use super::*;
use crate::{Error, TryClone, TryCloneIn};
use core::{slice::SliceIndex, str::Utf8Error};
impl<P: AllocationPolicy> Box<str, P> {
    pub fn empty_in(policy: P) -> Self {
        let bytes = Box::<[u8], _>::empty_in(policy);
        // SAFETY: empty bytes form valid UTF-8.
        unsafe { Self::from_utf8_unchecked(bytes) }
    }
    pub fn try_from_str_in(source: &str, policy: P) -> Result<Self, Error> {
        let bytes = Box::<[u8], _>::try_copy_from_slice_in(source.as_bytes(), policy)?;
        // SAFETY: copied bytes of a valid str.
        Ok(unsafe { Self::from_utf8_unchecked(bytes) })
    }
    pub fn try_from_utf8(bytes: Box<[u8], P>) -> Result<Self, Rejected<Box<[u8], P>, Utf8Error>> {
        if let Err(error) = core::str::from_utf8(bytes.as_slice()) {
            return Err(Rejected {
                value: bytes,
                error,
            });
        }
        // SAFETY: validation above; no intervening modification.
        Ok(unsafe { Self::from_utf8_unchecked(bytes) })
    }
    /// # Safety
    /// All bytes must form valid UTF-8.
    pub unsafe fn from_utf8_unchecked(bytes: Box<[u8], P>) -> Self {
        let (ptr, policy) = Box::into_raw_parts(bytes);
        // SAFETY: slice and str pointers share byte metadata and allocation layout;
        // UTF-8 validity is supplied by the caller.
        unsafe { Self::from_raw_parts(NonNull::new_unchecked(ptr.as_ptr() as *mut str), policy) }
    }
    pub fn into_bytes(self) -> Box<[u8], P> {
        let (ptr, policy) = Self::into_raw_parts(self);
        // SAFETY: str bytes remain initialized and owned, with unchanged layout.
        unsafe { Box::from_raw_parts(NonNull::new_unchecked(ptr.as_ptr() as *mut [u8]), policy) }
    }
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }
    pub fn as_mut_str(&mut self) -> &mut str {
        self.as_mut()
    }
    pub fn len(&self) -> usize {
        self.as_str().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get<I: SliceIndex<str>>(&self, index: I) -> Result<&I::Output, Error> {
        self.as_str().get(index).ok_or(Error::Bounds)
    }
    pub fn get_mut<I: SliceIndex<str>>(&mut self, index: I) -> Result<&mut I::Output, Error> {
        self.as_mut_str().get_mut(index).ok_or(Error::Bounds)
    }
}
impl Box<str> {
    pub fn empty() -> Self {
        Self::empty_in(Unmetered)
    }
    pub fn try_from_str(source: &str) -> Result<Self, Error> {
        Self::try_from_str_in(source, Unmetered)
    }
    pub fn into_std_string(self) -> alloc::string::String {
        // SAFETY: this owner's UTF-8 invariant is preserved by into_bytes.
        unsafe { alloc::string::String::from_utf8_unchecked(self.into_bytes().into_std_vec()) }
    }
}
impl Default for Box<str> {
    fn default() -> Self {
        Self::empty()
    }
}
impl<P: AllocationPolicy> TryClone for Box<str, P> {
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        if self.len() == source.len() {
            // SAFETY: disjoint owners, equal byte lengths, complete valid UTF-8 source.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    source.as_str().as_ptr(),
                    self.ptr.as_ptr() as *mut u8,
                    self.len(),
                )
            };
            return Ok(());
        }
        let replacement = Self::try_from_str_in(source.as_str(), self.policy.try_clone()?)?;
        *self = replacement;
        Ok(())
    }
    fn try_clone(&self) -> Result<Self, Error> {
        Self::try_from_str_in(self.as_str(), self.policy.try_clone()?)
    }
}
impl<'a, P: AllocationPolicy> TryCloneIn<'a> for Box<str, P> {
    type Output = Box<str, Budget<'a>>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        Box::<str, _>::try_from_str_in(self.as_str(), budget)
    }
}

impl<P: AllocationPolicy> Box<str, P> {
    pub fn try_from_std_string_in(
        value: alloc::string::String,
        policy: P,
    ) -> Result<Self, Rejected<alloc::string::String>> {
        match Box::<[u8], _>::try_from_std_vec_in(value.into_bytes(), policy) {
            // SAFETY: contents originated in a valid String and were not changed.
            Ok(bytes) => Ok(unsafe { Self::from_utf8_unchecked(bytes) }),
            Err(rejected) => Err(Rejected {
                value: unsafe { alloc::string::String::from_utf8_unchecked(rejected.value) },
                error: rejected.error,
            }),
        }
    }
}
impl crate::TryDefault for Box<str> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::empty())
    }
}
