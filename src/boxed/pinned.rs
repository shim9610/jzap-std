use super::*;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// Owning pin adapter for both sized and dynamic targets. It deliberately exposes
/// pinned borrows rather than an implicit raw slice/str dereference.
pub struct Pinned<T: ?Sized, P: AllocationPolicy = Unmetered> {
    inner: Box<T, P>,
}
impl<T: ?Sized, P: AllocationPolicy> Box<T, P> {
    pub fn into_pin(this: Self) -> Pinned<T, P> {
        Pinned { inner: this }
    }
}
impl<T, P: AllocationPolicy> Box<T, P> {
    pub fn try_pin_in(value: T, policy: P) -> Result<Pinned<T, P>, Rejected<T>> {
        Self::try_new_in(value, policy).map(Self::into_pin)
    }
    pub fn try_pin_init_in<I: crate::init::TryInit<T>>(
        policy: P,
        init: I,
    ) -> Result<Pinned<T, P>, InitError<I::Error>> {
        Self::try_init_in(policy, init).map(Self::into_pin)
    }
}
impl<T> Box<T> {
    pub fn try_pin(value: T) -> Result<Pinned<T>, Rejected<T>> {
        Self::try_pin_in(value, Unmetered)
    }
}
impl<T: ?Sized, P: AllocationPolicy> Pinned<T, P> {
    pub fn as_ref(&self) -> Pin<&T> {
        // SAFETY: inner's value remains at the same address through destruction.
        unsafe { Pin::new_unchecked(self.inner.as_ref()) }
    }
    pub fn as_mut(&mut self) -> Pin<&mut T> {
        // SAFETY: no unpinned mutable access or ownership extraction is exposed.
        unsafe { Pin::new_unchecked(self.inner.as_mut()) }
    }
    pub fn into_inner(self) -> Box<T, P>
    where
        T: Unpin,
    {
        self.inner
    }
    /// # Safety
    /// Caller must maintain the pointee's pinning guarantee until destruction.
    pub unsafe fn into_inner_unchecked(self) -> Box<T, P> {
        self.inner
    }
}
impl<T: ?Sized> Pinned<T> {
    pub fn into_std(self) -> Pin<alloc::boxed::Box<T>> {
        // SAFETY: ownership transfer does not relocate the pinned allocation.
        unsafe { Pin::new_unchecked(Box::into_std(self.inner)) }
    }
    pub fn from_std(value: Pin<alloc::boxed::Box<T>>) -> Self {
        // SAFETY: move only the owner handle; preserve the pinning guarantee.
        Self {
            inner: Box::from_std(unsafe { Pin::into_inner_unchecked(value) }),
        }
    }
}
impl<F: ?Sized + Future, P: AllocationPolicy> Future for Pinned<F, P> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().as_mut().poll(cx)
    }
}
impl<F: ?Sized + Future + Unpin, P: AllocationPolicy> Future for Box<F, P> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.get_mut().as_mut()).poll(cx)
    }
}

/// Preserve pinning through a compiler-checked, allocation-free pointer coercion.
#[macro_export]
macro_rules! pin_box_unsize {
    ($value:expr => $target:ty) => {{
        let __pinned = $value;
        // SAFETY: only the handle is moved, and the same allocation is immediately
        // repinned. No caller code can observe an unpinned intermediate owner.
        let __owner = unsafe { $crate::boxed::Pinned::into_inner_unchecked(__pinned) };
        $crate::boxed::Box::into_pin($crate::box_unsize!(__owner => $target))
    }};
}
