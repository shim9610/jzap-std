//! Caller-owned concurrent accounting. `used` includes reservations and charges.
use super::admission::admit;
use crate::Error;
#[cfg(not(jzap_loom))]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(jzap_loom)]
use loom::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct ConcurrentMemoryBudget {
    limit: usize,
    used: AtomicUsize,
}
impl ConcurrentMemoryBudget {
    #[cfg(not(jzap_loom))]
    pub const fn new(limit: usize) -> Self {
        Self {
            limit,
            used: AtomicUsize::new(0),
        }
    }
    #[cfg(jzap_loom)]
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            used: AtomicUsize::new(0),
        }
    }
    pub const fn limit(&self) -> usize {
        self.limit
    }
    #[inline]
    pub fn used(&self) -> usize {
        self.used.load(Ordering::Relaxed)
    }
    #[inline]
    pub fn available(&self) -> usize {
        self.limit.saturating_sub(self.used())
    }
    #[inline]
    pub fn reserve(&self, bytes: usize) -> Result<ConcurrentReservation<'_>, Error> {
        let mut used = self.used.load(Ordering::Relaxed);
        loop {
            let next = admit(self.limit, used, bytes)?;
            match self
                .used
                .compare_exchange(used, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => {
                    return Ok(ConcurrentReservation {
                        budget: self,
                        bytes,
                    });
                }
                Err(observed) => used = observed,
            }
        }
    }
    #[inline]
    pub(crate) fn release(&self, bytes: usize) {
        // Exactly one live reservation/charge owns these bytes. Numeric and
        // Loom token-conservation proofs establish bytes <= current used.
        self.used.fetch_sub(bytes, Ordering::Relaxed);
    }
}
#[must_use]
pub struct ConcurrentReservation<'a> {
    budget: &'a ConcurrentMemoryBudget,
    bytes: usize,
}
impl<'a> ConcurrentReservation<'a> {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    #[inline]
    pub fn commit(self) -> ConcurrentCharge<'a> {
        let charge = ConcurrentCharge {
            budget: self.budget,
            bytes: self.bytes,
        };
        core::mem::forget(self);
        charge
    }
}
impl Drop for ConcurrentReservation<'_> {
    #[inline]
    fn drop(&mut self) {
        self.budget.release(self.bytes);
    }
}
#[must_use]
pub struct ConcurrentCharge<'a> {
    budget: &'a ConcurrentMemoryBudget,
    bytes: usize,
}
impl ConcurrentCharge<'_> {
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    #[cfg(feature = "alloc")]
    pub(crate) fn retain(self) {
        core::mem::forget(self);
    }
}
impl Drop for ConcurrentCharge<'_> {
    #[inline]
    fn drop(&mut self) {
        self.budget.release(self.bytes);
    }
}
/// A one-word, thread-safe borrowed allocation policy.
#[derive(Clone, Copy, Debug)]
pub struct ConcurrentBudget<'a>(pub &'a ConcurrentMemoryBudget);
impl crate::TryClone for ConcurrentBudget<'_> {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(*self)
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;
    #[kani::proof]
    fn concurrent_budget_admission_all_machine_words() {
        let limit: usize = kani::any();
        let used: usize = kani::any();
        let bytes: usize = kani::any();
        let result = admit(limit, used, bytes);
        if used <= limit && bytes <= limit - used {
            let next = result.unwrap();
            assert!(next <= limit);
            assert_eq!(next as u128, used as u128 + bytes as u128);
        } else {
            assert!(result.is_err());
        }
    }
    #[kani::proof]
    #[kani::unwind(3)]
    fn actual_concurrent_reservation_commit_and_cancel() {
        let limit: usize = kani::any();
        let bytes: usize = kani::any();
        let budget = ConcurrentMemoryBudget::new(limit);
        if let Ok(token) = budget.reserve(bytes) {
            assert_eq!(budget.used(), bytes);
            if kani::any() {
                drop(token.commit());
            } else {
                drop(token);
            }
            assert_eq!(budget.used(), 0);
        } else {
            assert_eq!(budget.used(), 0);
        }
    }
}
