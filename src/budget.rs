//! Borrowed ledgers and explicit fallible ownership of allocation budgets.
#[cfg(target_has_atomic = "ptr")]
pub(crate) mod admission;
#[cfg(feature = "alloc")]
mod owned;
#[cfg(feature = "alloc")]
pub use owned::OwnedBudget;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
pub use owned::OwnedConcurrentBudget;
#[cfg(target_has_atomic = "ptr")]
mod concurrent;
use crate::Error;
use crate::budget_numeric;
#[cfg(target_has_atomic = "ptr")]
pub use concurrent::{
    ConcurrentBudget, ConcurrentCharge, ConcurrentMemoryBudget, ConcurrentReservation,
};
use core::cell::Cell;
#[cfg(all(kani, jzap_loop_contracts))]
#[path = "budget_proofs.rs"]
mod panic_proofs;
#[derive(Debug)]
pub struct MemoryBudget {
    limit: usize,
    occupied: Cell<usize>,
    available: Cell<usize>,
}
impl MemoryBudget {
    pub const fn new(limit: usize) -> Self {
        Self {
            limit,
            occupied: Cell::new(0),
            available: Cell::new(limit),
        }
    }
    pub const fn limit(&self) -> usize {
        self.limit
    }
    #[inline]
    pub fn occupied(&self) -> usize {
        self.occupied.get()
    }
    #[inline]
    pub fn reserved(&self) -> usize {
        // Invariant: occupied + available <= limit; the rest is reserved.
        let available = self.available.get();
        budget_numeric::pending(self.limit, self.occupied.get(), available)
    }
    #[inline]
    pub fn available(&self) -> usize {
        self.available.get()
    }
    #[inline]
    pub fn reserve(&self, bytes: usize) -> Result<Reservation<'_>, Error> {
        let available = budget_numeric::reserve(self.available(), bytes)?;
        self.available.set(available);
        Ok(Reservation {
            budget: self,
            bytes,
        })
    }
    #[inline]
    pub(crate) fn release(&self, bytes: usize) {
        self.occupied
            .set(budget_numeric::retire(self.occupied.get(), bytes));
        self.available
            .set(budget_numeric::credit(self.available.get(), bytes));
    }
}
#[must_use]
pub struct Reservation<'a> {
    budget: &'a MemoryBudget,
    bytes: usize,
}
impl<'a> Reservation<'a> {
    #[inline]
    pub fn commit(self) -> Charge<'a> {
        let bytes = self.bytes;
        self.budget
            .occupied
            .set(budget_numeric::credit(self.budget.occupied.get(), bytes));
        let charge = Charge {
            budget: self.budget,
            bytes,
        };
        // The reservation becomes occupied without restoring available bytes.
        core::mem::forget(self);
        charge
    }
}
impl Drop for Reservation<'_> {
    #[inline]
    fn drop(&mut self) {
        self.budget.available.set(budget_numeric::credit(
            self.budget.available.get(),
            self.bytes,
        ));
    }
}
#[must_use]
pub struct Charge<'a> {
    budget: &'a MemoryBudget,
    bytes: usize,
}
impl Charge<'_> {
    #[cfg(feature = "alloc")]
    pub(crate) fn retain(self) {
        core::mem::forget(self);
    }
}
impl Drop for Charge<'_> {
    #[inline]
    fn drop(&mut self) {
        self.budget.release(self.bytes);
    }
}
/// Budget choice is always explicit. Unmetered still reports allocator refusal.
#[derive(Clone, Copy, Debug)]
pub enum Budget<'a> {
    Limited(&'a MemoryBudget),
    Unmetered,
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Budget<'_> {}
    impl Sealed for super::Unmetered {}
    #[cfg(feature = "alloc")]
    impl Sealed for super::OwnedBudget {}
    #[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
    impl Sealed for super::OwnedConcurrentBudget {}
    impl<P: super::AllocationPolicy> Sealed for &P {}
    #[cfg(target_has_atomic = "ptr")]
    impl Sealed for super::ConcurrentBudget<'_> {}
}
/// Sealed allocation policy: explicit duplication preserves the same accounting
/// domain without duplicating its charge. Refusal leaves the source unchanged.
pub trait AllocationPolicy: sealed::Sealed + crate::TryClone {
    fn budget(&self) -> Accounting<'_>;
}
/// A scoped policy borrow keeps the original owner alive without acquiring a
/// new policy owner. This is also used by infallible cleanup guards.
impl<P: AllocationPolicy> AllocationPolicy for &P {
    #[inline]
    fn budget(&self) -> Accounting<'_> {
        P::budget(self)
    }
}
impl AllocationPolicy for Budget<'_> {
    #[inline]
    fn budget(&self) -> Accounting<'_> {
        match *self {
            Budget::Limited(b) => Accounting::Local(b),
            Budget::Unmetered => Accounting::Unmetered,
        }
    }
}
impl crate::TryClone for Budget<'_> {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(*self)
    }
}
/// Zero-sized explicit policy; an unmetered Box carries only its data pointer.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unmetered;
impl crate::TryClone for Unmetered {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(*self)
    }
}
impl AllocationPolicy for Unmetered {
    #[inline]
    fn budget(&self) -> Accounting<'_> {
        Accounting::Unmetered
    }
}

/// Ephemeral accounting dispatch. Stored policy handles keep their original size.
#[derive(Clone, Copy, Debug)]
pub enum Accounting<'a> {
    Unmetered,
    Local(&'a MemoryBudget),
    #[cfg(target_has_atomic = "ptr")]
    Concurrent(&'a ConcurrentMemoryBudget),
}
#[cfg(feature = "alloc")]
impl<'a> Accounting<'a> {
    /// Allocation ownership can move without a charge transfer only within the
    /// identical ledger. Equal limits or balances do not establish identity.
    #[inline]
    pub(crate) fn same_domain(self, other: Accounting<'_>) -> bool {
        match (self, other) {
            (Self::Unmetered, Accounting::Unmetered) => true,
            (Self::Local(a), Accounting::Local(b)) => core::ptr::eq(a, b),
            #[cfg(target_has_atomic = "ptr")]
            (Self::Concurrent(a), Accounting::Concurrent(b)) => core::ptr::eq(a, b),
            _ => false,
        }
    }
    #[inline]
    pub(crate) fn is_metered(self) -> bool {
        !matches!(self, Self::Unmetered)
    }
    #[inline]
    pub(crate) fn reserve(self, bytes: usize) -> Result<Option<AccountingReservation<'a>>, Error> {
        match self {
            Self::Unmetered => Ok(None),
            Self::Local(b) => b
                .reserve(bytes)
                .map(|r| Some(AccountingReservation::Local(r))),
            #[cfg(target_has_atomic = "ptr")]
            Self::Concurrent(b) => b
                .reserve(bytes)
                .map(|r| Some(AccountingReservation::Concurrent(r))),
        }
    }
    #[inline]
    pub(crate) fn release(self, bytes: usize) {
        match self {
            Self::Unmetered => {}
            Self::Local(b) => b.release(bytes),
            #[cfg(target_has_atomic = "ptr")]
            Self::Concurrent(b) => b.release(bytes),
        }
    }
}
#[cfg(feature = "alloc")]
pub(crate) enum AccountingReservation<'a> {
    Local(Reservation<'a>),
    #[cfg(target_has_atomic = "ptr")]
    Concurrent(ConcurrentReservation<'a>),
}
#[cfg(feature = "alloc")]
impl<'a> AccountingReservation<'a> {
    #[inline]
    pub(crate) fn commit(self) -> AccountingCharge<'a> {
        match self {
            Self::Local(r) => AccountingCharge::Local(r.commit()),
            #[cfg(target_has_atomic = "ptr")]
            Self::Concurrent(r) => AccountingCharge::Concurrent(r.commit()),
        }
    }
}
#[cfg(feature = "alloc")]
pub(crate) enum AccountingCharge<'a> {
    Local(Charge<'a>),
    #[cfg(target_has_atomic = "ptr")]
    Concurrent(ConcurrentCharge<'a>),
}
#[cfg(feature = "alloc")]
impl AccountingCharge<'_> {
    #[inline]
    pub(crate) fn retain(self) {
        match self {
            Self::Local(c) => c.retain(),
            #[cfg(target_has_atomic = "ptr")]
            Self::Concurrent(c) => c.retain(),
        }
    }
}
#[cfg(target_has_atomic = "ptr")]
impl AllocationPolicy for ConcurrentBudget<'_> {
    #[inline]
    fn budget(&self) -> Accounting<'_> {
        Accounting::Concurrent(self.0)
    }
}

#[cfg(all(test, feature = "alloc"))]
#[path = "../tests/support/policy_ownership.rs"]
mod policy_ownership_tests;
