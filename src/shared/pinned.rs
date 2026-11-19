use super::*;
use core::pin::Pin;
pub struct Pinned<T: ?Sized, C: Counter, P: SharedPolicy = Unmetered> {
    owner: Strong<T, C, P>,
}
impl<T, C: Counter> Strong<T, C> {
    pub fn try_pin(value: T) -> Result<Pinned<T, C>, Rejected<T>> {
        Self::try_pin_in(value, Unmetered)
    }
}
impl<T, C: Counter, P: SharedPolicy> Strong<T, C, P> {
    pub fn try_pin_in(value: T, policy: P) -> Result<Pinned<T, C, P>, Rejected<T>> {
        Self::try_new_in(value, policy).map(|owner| Pinned { owner })
    }
    pub fn try_pin_init_in<I: crate::init::TryInit<T>>(
        policy: P,
        initialize: I,
    ) -> Result<Pinned<T, C, P>, InitError<I::Error>> {
        Self::try_init_in(policy, initialize).map(|owner| Pinned { owner })
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> Strong<T, C, P> {
    pub fn into_pin(this: Self) -> Pinned<T, C, P>
    where
        T: Unpin,
    {
        Pinned { owner: this }
    }
    /// Pin an existing owner only when neither ordinary strong aliases nor
    /// weak upgrades could later regain movable access to the payload.
    pub fn try_into_pin(this: Self) -> Result<Pinned<T, C, P>, Self> {
        let (strong, weak) = unsafe { counts(this.ptr) };
        if !C::unique(strong, weak) {
            return Err(this);
        }
        Ok(Pinned { owner: this })
    }
    /// # Safety
    /// Preserve pinning until T is destroyed. Every other ordinary strong/raw
    /// owner or weak upgrade must also respect pinning throughout that lifetime.
    pub unsafe fn into_pin_unchecked(this: Self) -> Pinned<T, C, P> {
        Pinned { owner: this }
    }
}
impl<T: ?Sized, C: Counter, P: SharedPolicy> Pinned<T, C, P> {
    pub fn as_ref(&self) -> Pin<&T> {
        // SAFETY: shared allocation never moves while a pinned owner exists;
        // no ordinary owner, weak or mutable reference is exposed through it.
        unsafe { Pin::new_unchecked(self.owner.as_ref()) }
    }
    pub fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self {
            owner: Strong::try_clone(&self.owner)?,
        })
    }
    pub fn into_inner(self) -> Strong<T, C, P>
    where
        T: Unpin,
    {
        self.owner
    }
    /// # Safety
    /// The caller preserves T's pinning guarantee through final destruction.
    pub unsafe fn into_inner_unchecked(self) -> Strong<T, C, P> {
        self.owner
    }
}

/// Ordinary aliases cannot be converted into an untracked pinned owner.
/// ```compile_fail
/// use jzap::rc::Rc;
/// struct Immovable(core::marker::PhantomPinned);
/// let owner=Rc::try_new(Immovable(core::marker::PhantomPinned)).ok().unwrap();
/// let _=Rc::into_pin(owner);
/// ```
/// ```compile_fail
/// use jzap::{budget::{ConcurrentBudget,ConcurrentMemoryBudget},sync::Arc};
/// let weak;
/// { let budget=ConcurrentMemoryBudget::new(1024);
///   let owner=Arc::try_new_in(17,ConcurrentBudget(&budget)).ok().unwrap();
///   weak=Arc::try_downgrade(&owner).unwrap(); }
/// drop(weak);
/// ```
mod pin_and_budget_contracts {}
