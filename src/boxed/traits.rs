use super::*;
use core::{
    any::Any,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};
impl<T: ?Sized, P: AllocationPolicy> Unpin for Box<T, P> {}
impl<T: ?Sized + fmt::Debug, P: AllocationPolicy> fmt::Debug for Box<T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}
impl<T: ?Sized + fmt::Display, P: AllocationPolicy> fmt::Display for Box<T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}
impl<T: ?Sized, P: AllocationPolicy> fmt::Pointer for Box<T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.ptr.as_ptr(), f)
    }
}
impl<T: ?Sized + PartialEq, P: AllocationPolicy, Q: AllocationPolicy> PartialEq<Box<T, Q>>
    for Box<T, P>
{
    fn eq(&self, other: &Box<T, Q>) -> bool {
        self.as_ref().eq(other.as_ref())
    }
}
impl<T: ?Sized + Eq, P: AllocationPolicy> Eq for Box<T, P> {}
impl<T: ?Sized + PartialOrd, P: AllocationPolicy, Q: AllocationPolicy> PartialOrd<Box<T, Q>>
    for Box<T, P>
{
    fn partial_cmp(&self, other: &Box<T, Q>) -> Option<Ordering> {
        self.as_ref().partial_cmp(other.as_ref())
    }
}
impl<T: ?Sized + Ord, P: AllocationPolicy> Ord for Box<T, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}
impl<T: ?Sized + Hash, P: AllocationPolicy> Hash for Box<T, P> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}
impl<T: ?Sized + core::error::Error, P: AllocationPolicy> core::error::Error for Box<T, P> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        self.as_ref().source()
    }
}
macro_rules! downcast {
    ($($bounds:tt)*) => {
        impl<P: AllocationPolicy> Box<dyn Any $($bounds)*, P> {
            pub fn downcast<T: Any>(self) -> Result<Box<T, P>, Self> {
                if !self.as_ref().is::<T>() { return Err(self); }
                let (ptr, policy) = Self::into_raw_parts(self);
                // SAFETY: Any verified concrete T; same data allocation and charge.
                Ok(unsafe { Box::from_raw_parts(ptr.cast(), policy) })
            }
        }
    };
}
downcast!();
downcast!(+ Send);
downcast!(+ Send + Sync);
impl<I: Iterator, P: AllocationPolicy> Iterator for Box<I, P> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.as_mut().next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.as_ref().size_hint()
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.as_mut().nth(n)
    }
}
impl<I: DoubleEndedIterator, P: AllocationPolicy> DoubleEndedIterator for Box<I, P> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.as_mut().next_back()
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.as_mut().nth_back(n)
    }
}
impl<I: ExactSizeIterator, P: AllocationPolicy> ExactSizeIterator for Box<I, P> {
    fn len(&self) -> usize {
        self.as_ref().len()
    }
}
impl<I: core::iter::FusedIterator, P: AllocationPolicy> core::iter::FusedIterator for Box<I, P> {}

impl<I: crate::TryIterator + ?Sized, P: AllocationPolicy> crate::TryIterator for Box<I, P> {
    type Item = I::Item;
    type Error = I::Error;
    #[inline]
    fn try_next(&mut self) -> Result<Option<I::Item>, I::Error> {
        self.as_mut().try_next()
    }
    #[inline]
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), I::Error> {
        self.as_ref().try_size_hint()
    }
}
impl<I: crate::TryDoubleEndedIterator + ?Sized, P: AllocationPolicy> crate::TryDoubleEndedIterator
    for Box<I, P>
{
    #[inline]
    fn try_next_back(&mut self) -> Result<Option<I::Item>, I::Error> {
        self.as_mut().try_next_back()
    }
}
impl<I: crate::TryExactSizeIterator + ?Sized, P: AllocationPolicy> crate::TryExactSizeIterator
    for Box<I, P>
{
    #[inline]
    fn try_len(&self) -> Result<usize, I::Error> {
        self.as_ref().try_len()
    }
}
impl<I: crate::TryFusedIterator + ?Sized, P: AllocationPolicy> crate::TryFusedIterator
    for Box<I, P>
{
}

macro_rules! hasher_writes { ($($method:ident:$ty:ty),*) => { $(fn $method(&mut self, value: $ty) { self.as_mut().$method(value) })* }; }
impl<H: ?Sized + Hasher, P: AllocationPolicy> Hasher for Box<H, P> {
    fn finish(&self) -> u64 {
        self.as_ref().finish()
    }
    fn write(&mut self, bytes: &[u8]) {
        self.as_mut().write(bytes)
    }
    hasher_writes!(write_u8:u8,write_u16:u16,write_u32:u32,write_u64:u64,write_u128:u128,write_usize:usize,
        write_i8:i8,write_i16:i16,write_i32:i32,write_i64:i64,write_i128:i128,write_isize:isize);
}
macro_rules! error_downcast { ($($bounds:tt)*) => {
    impl<P: AllocationPolicy> Box<dyn core::error::Error $($bounds)*, P> {
        pub fn downcast<T: core::error::Error + 'static>(self) -> Result<Box<T, P>, Self> {
            if !self.as_ref().is::<T>() { return Err(self); }
            let (ptr, policy) = Self::into_raw_parts(self);
            // SAFETY: Error's type check proved concrete T; allocation unchanged.
            Ok(unsafe { Box::from_raw_parts(ptr.cast(), policy) })
        }
    }
    impl<'a, P: AllocationPolicy> Box<dyn core::error::Error $($bounds)* + 'a, P> {
        pub fn try_from_error_in<E: core::error::Error $($bounds)* + 'a>(value: E, policy: P) -> Result<Self, Rejected<E>> {
            Box::try_new_in(value, policy).map(|owned| crate::box_unsize!(owned => dyn core::error::Error $($bounds)* + 'a))
        }
    }
}; }
error_downcast!();
error_downcast!(+ Send);
error_downcast!(+ Send + Sync);
