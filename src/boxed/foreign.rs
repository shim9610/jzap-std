//! C strings and feature-gated OS/path ownership. Checked borrowed views establish
//! layout, data address and metadata before an owning pointer is converted.
use super::*;
use crate::{Error, TryClone, TryCloneIn};
use core::ffi::{CStr, FromBytesWithNulError};

#[derive(Debug, PartialEq, Eq)]
pub enum CStrError {
    Encoding(FromBytesWithNulError),
    Layout,
}
fn compatible_c_str(view: &CStr, bytes: &[u8]) -> bool {
    Layout::for_value(view) == Layout::for_value(bytes)
        && (view as *const CStr as *const [u8]).len() == bytes.len()
        && (view as *const CStr).cast::<u8>() == bytes.as_ptr()
}
impl<P: AllocationPolicy> Box<CStr, P> {
    pub fn as_c_str(&self) -> &CStr {
        self.as_ref()
    }
    pub fn try_from_c_str_in(source: &CStr, policy: P) -> Result<Self, Error> {
        if !compatible_c_str(source, source.to_bytes_with_nul()) {
            return Err(Error::Layout);
        }
        let bytes = Box::<[u8], _>::try_copy_from_slice_in(source.to_bytes_with_nul(), policy)?;
        // SAFETY: exact copy of a validated CStr with checked layout and metadata.
        Ok(unsafe { Self::from_bytes_unchecked(bytes) })
    }
    pub fn try_from_bytes_with_nul(
        bytes: Box<[u8], P>,
    ) -> Result<Self, Rejected<Box<[u8], P>, CStrError>> {
        let error = match CStr::from_bytes_with_nul(bytes.as_slice()) {
            Err(e) => Some(CStrError::Encoding(e)),
            Ok(view) if !compatible_c_str(view, bytes.as_slice()) => Some(CStrError::Layout),
            Ok(_) => None,
        };
        if let Some(error) = error {
            return Err(Rejected {
                value: bytes,
                error,
            });
        }
        // SAFETY: validation checks encoding, layout, address and metadata above.
        Ok(unsafe { Self::from_bytes_unchecked(bytes) })
    }
    unsafe fn from_bytes_unchecked(bytes: Box<[u8], P>) -> Self {
        let (ptr, policy) = Box::into_raw_parts(bytes);
        // SAFETY: caller validated the public borrowed CStr view against byte storage.
        // Cast preserves its checked length metadata and the original mutable provenance.
        unsafe { Self::from_raw_parts(NonNull::new_unchecked(ptr.as_ptr() as *mut CStr), policy) }
    }
    pub fn try_into_bytes_with_nul(self) -> Result<Box<[u8], P>, Rejected<Self>> {
        let bytes = self.as_c_str().to_bytes_with_nul();
        if !compatible_c_str(self.as_c_str(), bytes) {
            return Err(Rejected {
                value: self,
                error: Error::Layout,
            });
        }
        let len = bytes.len();
        let (ptr, policy) = Self::into_raw_parts(self);
        // SAFETY: checked exact allocation layout and initialized byte view.
        Ok(unsafe {
            Box::from_raw_parts(NonNull::slice_from_raw_parts(ptr.cast::<u8>(), len), policy)
        })
    }
    #[cfg(not(no_global_oom_handling))]
    pub fn try_from_std_c_string_in(
        value: alloc::ffi::CString,
        policy: P,
    ) -> Result<Self, Rejected<alloc::ffi::CString>> {
        Self::try_from_std_in(value.into_boxed_c_str(), policy).map_err(|e| Rejected {
            value: e.value.into_c_string(),
            error: e.error,
        })
    }
}
impl Box<CStr> {
    pub fn try_from_c_str(source: &CStr) -> Result<Self, Error> {
        Self::try_from_c_str_in(source, Unmetered)
    }
    #[cfg(not(no_global_oom_handling))]
    pub fn into_std_c_string(self) -> alloc::ffi::CString {
        Self::into_std(self).into_c_string()
    }
}
impl crate::TryDefault for Box<CStr> {
    fn try_default() -> Result<Self, Error> {
        Self::try_from_c_str(c"")
    }
}
impl<P: AllocationPolicy> TryClone for Box<CStr, P> {
    fn try_clone(&self) -> Result<Self, Error> {
        Self::try_from_c_str_in(self.as_ref(), self.policy.try_clone()?)
    }
    fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
        let replacement = Self::try_from_c_str_in(source.as_ref(), self.policy.try_clone()?)?;
        *self = replacement;
        Ok(())
    }
}
impl<'a, P: AllocationPolicy> TryCloneIn<'a> for Box<CStr, P> {
    type Output = Box<CStr, Budget<'a>>;
    fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
        Box::<CStr, _>::try_from_c_str_in(self.as_ref(), budget)
    }
}

#[cfg(feature = "std")]
mod os {
    use super::*;
    use std::{
        ffi::{OsStr, OsString},
        path::{Path, PathBuf},
    };
    fn compatible_os_str(view: &OsStr) -> bool {
        let bytes = view.as_encoded_bytes();
        Layout::for_value(view) == Layout::for_value(bytes)
            && (view as *const OsStr as *const [u8]).len() == bytes.len()
            && (view as *const OsStr).cast::<u8>() == bytes.as_ptr()
    }
    fn compatible_path(view: &Path) -> bool {
        let os = view.as_os_str();
        Layout::for_value(view) == Layout::for_value(os)
            && (view as *const Path as *const [u8]).len()
                == (os as *const OsStr as *const [u8]).len()
            && (view as *const Path).cast::<u8>() == (os as *const OsStr).cast::<u8>()
    }
    impl<P: AllocationPolicy> Box<OsStr, P> {
        pub fn as_os_str(&self) -> &OsStr {
            self.as_ref()
        }
        unsafe fn from_bytes_unchecked(bytes: Box<[u8], P>) -> Self {
            let (ptr, policy) = Box::into_raw_parts(bytes);
            // SAFETY: caller checked native encoding, layout and metadata; retain mutable provenance.
            unsafe {
                Self::from_raw_parts(NonNull::new_unchecked(ptr.as_ptr() as *mut OsStr), policy)
            }
        }
        pub fn try_from_os_str_in(source: &OsStr, policy: P) -> Result<Self, Error> {
            if !compatible_os_str(source) {
                return Err(Error::Layout);
            }
            let bytes = Box::<[u8], _>::try_copy_from_slice_in(source.as_encoded_bytes(), policy)?;
            // SAFETY: a complete native encoded string was copied in this Rust runtime.
            Ok(unsafe { Self::from_bytes_unchecked(bytes) })
        }
        pub fn try_from_std_os_string_in(
            value: OsString,
            policy: P,
        ) -> Result<Self, Rejected<OsString>> {
            if !compatible_os_str(&value) {
                return Err(Rejected {
                    value,
                    error: Error::Layout,
                });
            }
            match Box::<[u8], _>::try_from_std_vec_in(value.into_encoded_bytes(), policy) {
                Ok(bytes) => Ok(unsafe { Self::from_bytes_unchecked(bytes) }),
                Err(e) => Err(Rejected {
                    value: unsafe { OsString::from_encoded_bytes_unchecked(e.value) },
                    error: e.error,
                }),
            }
        }
    }
    impl Box<OsStr> {
        pub fn try_from_os_str(source: &OsStr) -> Result<Self, Error> {
            Self::try_from_os_str_in(source, Unmetered)
        }
        pub fn into_std_os_string(self) -> OsString {
            Self::into_std(self).into_os_string()
        }
    }
    impl<P: AllocationPolicy> TryClone for Box<OsStr, P> {
        fn try_clone(&self) -> Result<Self, Error> {
            Self::try_from_os_str_in(self.as_ref(), self.policy.try_clone()?)
        }
        fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
            let replacement = Self::try_from_os_str_in(source.as_ref(), self.policy.try_clone()?)?;
            *self = replacement;
            Ok(())
        }
    }
    impl<'a, P: AllocationPolicy> TryCloneIn<'a> for Box<OsStr, P> {
        type Output = Box<OsStr, Budget<'a>>;
        fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
            Box::<OsStr, _>::try_from_os_str_in(self.as_ref(), budget)
        }
    }
    impl<P: AllocationPolicy> Box<Path, P> {
        pub fn as_path(&self) -> &Path {
            self.as_ref()
        }
        pub fn try_from_os_str(value: Box<OsStr, P>) -> Result<Self, Rejected<Box<OsStr, P>>> {
            if !compatible_path(Path::new(value.as_os_str())) {
                return Err(Rejected {
                    value,
                    error: Error::Layout,
                });
            }
            let (ptr, policy) = Box::into_raw_parts(value);
            // SAFETY: public view validated address, layout and metadata; preserve original provenance.
            Ok(unsafe {
                Self::from_raw_parts(NonNull::new_unchecked(ptr.as_ptr() as *mut Path), policy)
            })
        }
        pub fn try_into_os_str(self) -> Result<Box<OsStr, P>, Rejected<Self>> {
            if !compatible_path(self.as_path()) {
                return Err(Rejected {
                    value: self,
                    error: Error::Layout,
                });
            }
            let (ptr, policy) = Self::into_raw_parts(self);
            // SAFETY: inverse of checked layout and metadata preserving ownership conversion.
            Ok(unsafe {
                Box::from_raw_parts(NonNull::new_unchecked(ptr.as_ptr() as *mut OsStr), policy)
            })
        }
        pub fn try_from_path_in(source: &Path, policy: P) -> Result<Self, Error> {
            if !compatible_path(source) {
                return Err(Error::Layout);
            }
            let value = Box::<OsStr, _>::try_from_os_str_in(source.as_os_str(), policy)?;
            Self::try_from_os_str(value).map_err(|e| e.error)
        }
        pub fn try_from_std_path_buf_in(
            value: PathBuf,
            policy: P,
        ) -> Result<Self, Rejected<PathBuf>> {
            if !compatible_path(&value) {
                return Err(Rejected {
                    value,
                    error: Error::Layout,
                });
            }
            // PathBuf to a boxed path can shrink through infallible realloc. Instead
            // transfer to encoded bytes and use the fallible Vec shrink path.
            let value = Box::<OsStr, _>::try_from_std_os_string_in(value.into_os_string(), policy)
                .map_err(|e| Rejected {
                    value: PathBuf::from(e.value),
                    error: e.error,
                })?;
            let (ptr, policy) = Box::into_raw_parts(value);
            // SAFETY: checked native Path/OsStr representation before ownership transfer.
            Ok(unsafe {
                Self::from_raw_parts(NonNull::new_unchecked(ptr.as_ptr() as *mut Path), policy)
            })
        }
    }
    impl Box<Path> {
        pub fn try_from_path(source: &Path) -> Result<Self, Error> {
            Self::try_from_path_in(source, Unmetered)
        }
        pub fn into_std_path_buf(self) -> PathBuf {
            Self::into_std(self).into_path_buf()
        }
    }
    impl<P: AllocationPolicy> TryClone for Box<Path, P> {
        fn try_clone(&self) -> Result<Self, Error> {
            Self::try_from_path_in(self.as_ref(), self.policy.try_clone()?)
        }
        fn try_clone_from(&mut self, source: &Self) -> Result<(), Error> {
            let replacement = Self::try_from_path_in(source.as_ref(), self.policy.try_clone()?)?;
            *self = replacement;
            Ok(())
        }
    }
    impl<'a, P: AllocationPolicy> TryCloneIn<'a> for Box<Path, P> {
        type Output = Box<Path, Budget<'a>>;
        fn try_clone_in(&self, budget: Budget<'a>) -> Result<Self::Output, Error> {
            Box::<Path, _>::try_from_path_in(self.as_ref(), budget)
        }
    }
    impl crate::TryDefault for Box<OsStr> {
        fn try_default() -> Result<Self, Error> {
            Self::try_from_os_str(OsStr::new(""))
        }
    }
    impl crate::TryDefault for Box<Path> {
        fn try_default() -> Result<Self, Error> {
            Self::try_from_path(Path::new(""))
        }
    }
}
