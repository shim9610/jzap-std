use super::*;
use core::{
    borrow::Borrow,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};
impl<T, C: Counter, P: SharedPolicy> Deref for Strong<T, C, P> {
    type Target = T;
    fn deref(&self) -> &T {
        self.as_ref()
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> AsRef<T> for Strong<T, C, P> {
    fn as_ref(&self) -> &T {
        // SAFETY: a live strong credit keeps the initialized payload alive.
        unsafe { &*data(self.ptr) }
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> Borrow<T> for Strong<T, C, P> {
    fn borrow(&self) -> &T {
        self.as_ref()
    }
}
impl<T: ?Sized + fmt::Debug, C: Counter, P: SharedPolicy> fmt::Debug for Strong<T, C, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}
impl<T: ?Sized + fmt::Display, C: Counter, P: SharedPolicy> fmt::Display for Strong<T, C, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> fmt::Pointer for Strong<T, C, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Self::as_ptr(self).fmt(f)
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> fmt::Debug for Weak<T, C, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(Weak)")
    }
}
impl<T: ?Sized + PartialEq, C: Counter, P: SharedPolicy> PartialEq for Strong<T, C, P> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref().eq(other.as_ref())
    }
}
impl<T: ?Sized + Eq, C: Counter, P: SharedPolicy> Eq for Strong<T, C, P> {}
impl<T: ?Sized + PartialOrd, C: Counter, P: SharedPolicy> PartialOrd for Strong<T, C, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_ref().partial_cmp(other.as_ref())
    }
}
impl<T: ?Sized + Ord, C: Counter, P: SharedPolicy> Ord for Strong<T, C, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}
impl<T: ?Sized + Hash, C: Counter, P: SharedPolicy> Hash for Strong<T, C, P> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_ref().hash(h)
    }
}
impl<T, C: Counter, P: SharedPolicy> Default for Weak<T, C, P> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T, C: Counter, P: SharedPolicy> crate::TryDefault for Weak<T, C, P> {
    fn try_default() -> Result<Self, Error> {
        Ok(Self::new())
    }
}
impl<T: crate::TryDefault, C: Counter> crate::TryDefault for Strong<T, C> {
    fn try_default() -> Result<Self, Error> {
        Self::try_init_in(Unmetered, T::try_default_into).map_err(|e| match e {
            InitError::Storage(e) | InitError::Initialize(e) => e,
        })
    }
}
macro_rules! downcast {
    ($target:ty) => {
        impl<C: Counter, P: SharedPolicy> Strong<$target, C, P> {
            pub fn downcast<T: core::any::Any>(this: Self) -> Result<Strong<T, C, P>, Self> {
                if !this.as_ref().is::<T>() {
                    return Err(this);
                }
                let ptr = Self::into_raw(this).cast::<Control<C, P, T>>();
                // SAFETY: Any's type identity check establishes the exact T.
                Ok(unsafe { Strong::from_raw(ptr) })
            }
        }
    };
}
downcast!(dyn core::any::Any);
downcast!(dyn core::any::Any + Send);
downcast!(dyn core::any::Any + Send + Sync);

// Moving a handle never relocates its payload. Pinning is provided by the
// separate adapter, which controls extraction rather than the handle address.
impl<T: ?Sized, C: Counter, P: SharedPolicy> Unpin for Strong<T, C, P> {}
impl<T: ?Sized, C: Counter, P: SharedPolicy> Unpin for Weak<T, C, P> {}

// Count and initialization guards restore private bookkeeping before unwinding
// exposes an owner again. Match std's payload/policy unwind-safety boundary.
impl<T: ?Sized + core::panic::RefUnwindSafe, C: Counter, P: SharedPolicy + core::panic::UnwindSafe>
    core::panic::UnwindSafe for Strong<T, C, P>
{
}
impl<T: ?Sized + core::panic::RefUnwindSafe, C: Counter, P: SharedPolicy + core::panic::UnwindSafe>
    core::panic::RefUnwindSafe for Strong<T, C, P>
{
}
