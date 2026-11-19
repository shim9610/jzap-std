//! Owning policies keep their ledger alive without a public lifetime parameter.
//!
//! Ledger/control-block bootstrap storage is allocated with Unmetered. The limit
//! applies to allocations charged to the ledger; construction reports bootstrap
//! allocation failure explicitly. Policy cloning acquires a fallible Rc/Arc credit.
use super::{Accounting, AllocationPolicy, MemoryBudget, Reservation};
use crate::{Error, TryClone};

/// Single-threaded, owning budget policy. Its ledger is shared through fallible Rc.
/// The ledger allocation is separate from the requested-byte limit.
#[derive(Debug)]
pub struct OwnedBudget {
    inner: crate::rc::Rc<MemoryBudget>,
}
impl OwnedBudget {
    pub fn try_new(limit: usize) -> Result<Self, Error> {
        crate::rc::Rc::try_new(MemoryBudget::new(limit))
            .map(|inner| Self { inner })
            .map_err(|rejected| rejected.error)
    }
    #[inline]
    pub fn ledger(&self) -> &MemoryBudget {
        self.inner.as_ref()
    }
    #[inline]
    pub fn limit(&self) -> usize {
        self.ledger().limit()
    }
    #[inline]
    pub fn occupied(&self) -> usize {
        self.ledger().occupied()
    }
    #[inline]
    pub fn reserved(&self) -> usize {
        self.ledger().reserved()
    }
    #[inline]
    pub fn available(&self) -> usize {
        self.ledger().available()
    }
    #[inline]
    pub fn reserve(&self, bytes: usize) -> Result<Reservation<'_>, Error> {
        self.ledger().reserve(bytes)
    }
}
impl TryClone for OwnedBudget {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self {
            inner: crate::rc::Rc::try_clone(&self.inner)?,
        })
    }
}
impl AllocationPolicy for OwnedBudget {
    #[inline]
    fn budget(&self) -> Accounting<'_> {
        Accounting::Local(self.ledger())
    }
}

#[cfg(target_has_atomic = "ptr")]
use super::{ConcurrentMemoryBudget, ConcurrentReservation};

/// Thread-safe owning budget policy using fallible Arc and atomic accounting.
/// Containers storing this policy can be Send + Sync + 'static when their data is.
/// The ledger allocation is separate from the requested-byte limit.
#[cfg(target_has_atomic = "ptr")]
#[derive(Debug)]
pub struct OwnedConcurrentBudget {
    inner: crate::sync::Arc<ConcurrentMemoryBudget>,
}
#[cfg(target_has_atomic = "ptr")]
impl OwnedConcurrentBudget {
    pub fn try_new(limit: usize) -> Result<Self, Error> {
        crate::sync::Arc::try_new(ConcurrentMemoryBudget::new(limit))
            .map(|inner| Self { inner })
            .map_err(|rejected| rejected.error)
    }
    #[inline]
    pub fn ledger(&self) -> &ConcurrentMemoryBudget {
        self.inner.as_ref()
    }
    #[inline]
    pub fn limit(&self) -> usize {
        self.ledger().limit()
    }
    #[inline]
    pub fn used(&self) -> usize {
        self.ledger().used()
    }
    #[inline]
    pub fn available(&self) -> usize {
        self.ledger().available()
    }
    #[inline]
    pub fn reserve(&self, bytes: usize) -> Result<ConcurrentReservation<'_>, Error> {
        self.ledger().reserve(bytes)
    }
}
#[cfg(target_has_atomic = "ptr")]
impl TryClone for OwnedConcurrentBudget {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self {
            inner: crate::sync::Arc::try_clone(&self.inner)?,
        })
    }
}
#[cfg(target_has_atomic = "ptr")]
impl AllocationPolicy for OwnedConcurrentBudget {
    #[inline]
    fn budget(&self) -> Accounting<'_> {
        Accounting::Concurrent(self.ledger())
    }
}
