//! Actual count transitions shared by native, model-checking and Loom builds.
use crate::Error;
use core::cell::Cell;
#[cfg(not(jzap_loom))]
#[cfg(target_has_atomic = "ptr")]
use core::sync::atomic::{AtomicUsize, Ordering, fence};
#[cfg(jzap_loom)]
use loom::sync::atomic::{AtomicUsize, Ordering, fence};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(not(jzap_small_counts))]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub const MAX_COUNT: usize = isize::MAX as usize;
#[cfg(jzap_small_counts)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub const MAX_COUNT: usize = 7;
pub const LOCK: usize = usize::MAX;
/// The high bit distinguishes a retired allocation from every live count.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub const RETIRED: usize = !(isize::MAX as usize);
/// Atomic weak counters reserve their top bit for retired-size validity.
/// Excluding the final payload count also keeps the all-ones weak lock distinct.
pub const WEAK_MAX_COUNT: usize = MAX_COUNT.saturating_sub(1);
/// The native fast path relies on Linux's x86-64 task/address-space bounds.
/// Other targets retain CAS; reduced-count model executions have <=3 threads.
/// See docs/verification/arc-headroom-integration.md for the platform boundary.
pub const BUFFERED_STRONG: bool = cfg!(any(
    jzap_small_counts,
    all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )
));
#[cfg(verus_keep_ghost)]
verus! {
proof fn retirement_mask_properties()
    ensures RETIRED as int == isize::MAX as int + 1,
        (isize::MAX as usize) == (!0usize) >> 1,
{
        let maximum = usize::MAX;
        assert(maximum <= !0usize) by(bit_vector);
        let half = isize::MAX as usize;
        let flag = RETIRED;
        vstd::bits::lemma_usize_shr_is_div(usize::MAX, 1);
        reveal(vstd::arithmetic::power::pow);
        assert(vstd::arithmetic::power2::pow2(1) == 2) by(compute_only);
        assert(half == usize::MAX >> 1);
        assert(flag == !half);
        assert(flag == half + 1) by(bit_vector)
            requires half == (!0usize) >> 1, flag == !half;
}
}
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires bytes <= isize::MAX as usize,
    ensures out as int == bytes as int + isize::MAX as int + 1,
))]
pub(crate) fn retired_state(bytes: usize) -> usize {
    let out = bytes | RETIRED;
    #[cfg(verus_keep_ghost)]
    proof! {
        retirement_mask_properties();
        let half = isize::MAX as usize;
        let flag = RETIRED;
        assert(out == bytes + flag) by(bit_vector)
            requires out == bytes | flag, bytes <= half, half == (!0usize) >> 1, flag == !half;
    }
    out
}
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out <= isize::MAX as usize,
        out as int == state as int % (isize::MAX as int + 1),
))]
pub(crate) fn retired_bytes(state: usize) -> usize {
    let out = state & !RETIRED;
    #[cfg(verus_keep_ghost)]
    proof! {
        retirement_mask_properties();
        let half = isize::MAX as usize;
        let flag = RETIRED;
        assert(out == state % flag) by(bit_vector)
            requires out == state & !flag, half == (!0usize) >> 1, flag == !half;
    }
    out
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(next) => old < MAX_COUNT && next as int == old as int + 1 && next <= MAX_COUNT,
        Err(Error::RefCount) => old >= MAX_COUNT,
        _ => false,
    },
))]
pub(crate) fn admitted_count(old: usize) -> Result<usize, Error> {
    if old >= MAX_COUNT {
        return Err(Error::RefCount);
    }
    old.checked_add(1).ok_or(Error::RefCount)
}
mod sealed {
    pub trait Sealed {}
}
/// Sealed implementation detail exposed only for compiler-checked unsizing.
#[doc(hidden)]
pub trait Counter: sealed::Sealed {
    /// # Safety
    /// Establish and maintain a valid count state for every subsequent protocol
    /// operation. Counters are constructed only inside an owned control block.
    unsafe fn new(value: usize) -> Self;
    fn load(&self) -> usize;
    fn increment(&self, live: bool) -> Result<bool, Error>;
    fn clone_strong(&self) -> Result<(), Error>;
    fn clone_weak(&self) -> Result<(), Error>;
    fn upgrade(&self, weak: &Self) -> Result<bool, Error>;
    fn initial_weak(retired: bool) -> usize;
    fn live_count(&self, weak: &Self) -> usize;
    fn external_weaks(&self) -> usize;
    fn release(&self) -> bool;
    fn release_weak(&self) -> bool;
    fn take_last(&self) -> bool;
    fn publish(&self, weak: &Self);
    fn retire(&self, weak: &Self, bytes: usize);
    fn unique(strong: &Self, weak: &Self) -> bool;
}
#[doc(hidden)]
#[repr(transparent)]
pub struct LocalCount(Cell<usize>);
impl sealed::Sealed for LocalCount {}
impl Counter for LocalCount {
    #[inline]
    fn clone_strong(&self) -> Result<(), Error> {
        self.increment(false).map(|_| ())
    }
    #[inline]
    fn clone_weak(&self) -> Result<(), Error> {
        self.increment(false).map(|_| ())
    }
    #[inline]
    fn upgrade(&self, _weak: &Self) -> Result<bool, Error> {
        self.increment(true)
    }
    #[inline]
    fn initial_weak(_retired: bool) -> usize {
        1
    }
    #[inline]
    fn live_count(&self, _weak: &Self) -> usize {
        let n = self.load();
        if n <= MAX_COUNT { n } else { 0 }
    }
    #[inline]
    fn external_weaks(&self) -> usize {
        match self.load() {
            LOCK => 0,
            n => n.saturating_sub(1),
        }
    }
    #[inline]
    fn release_weak(&self) -> bool {
        self.release()
    }
    #[inline]
    unsafe fn new(value: usize) -> Self {
        Self(Cell::new(value))
    }
    #[inline]
    fn load(&self) -> usize {
        self.0.get()
    }
    #[inline]
    fn increment(&self, live: bool) -> Result<bool, Error> {
        let old = self.0.get();
        if live && (old == 0 || old > MAX_COUNT) {
            return Ok(false);
        }
        self.0.set(admitted_count(old)?);
        Ok(true)
    }
    #[inline]
    fn release(&self) -> bool {
        let old = self.0.get();
        // A live handle owns one credit; proof establishes old >= 1.
        self.0.set(crate::arithmetic::bounded::wrapping_sub(old, 1));
        old == 1
    }
    #[inline]
    fn take_last(&self) -> bool {
        if self.0.get() != 1 {
            return false;
        }
        self.0.set(0);
        true
    }
    #[inline]
    fn publish(&self, _weak: &Self) {
        self.0.set(1);
    }
    #[inline]
    fn retire(&self, _weak: &Self, bytes: usize) {
        self.0.set(retired_state(bytes));
    }
    #[inline]
    fn unique(strong: &Self, weak: &Self) -> bool {
        strong.load() == 1 && weak.load() == 1
    }
}
#[cfg(target_has_atomic = "ptr")]
#[doc(hidden)]
#[repr(transparent)]
pub struct AtomicCount(AtomicUsize);
#[cfg(target_has_atomic = "ptr")]
impl sealed::Sealed for AtomicCount {}
#[cfg(target_has_atomic = "ptr")]
impl AtomicCount {
    /// Called only when the protocol supplies a bound on outstanding rejected
    /// increments. A reservation borrows a live strong owner until rollback.
    #[inline]
    pub(crate) fn buffered_clone_strong(&self) -> Result<(), Error> {
        let old = self.0.fetch_add(1, Ordering::Relaxed);
        let guard = MAX_COUNT.saturating_add(1);
        if crate::arithmetic::bounded::wrapping_add(old, 1) & guard != 0 {
            self.0.fetch_sub(1, Ordering::Release);
            Err(Error::RefCount)
        } else {
            Ok(())
        }
    }
    /// A high live word is a provisional reservation, not evidence of death.
    /// Acquire reads pair with cyclic publication and retirement publication.
    #[inline]
    fn observed_live_word(&self, weak: &Self) -> Option<usize> {
        let mut word = self.0.load(Ordering::Relaxed);
        loop {
            if word == 0 {
                return None;
            }
            if word <= MAX_COUNT {
                return Some(word);
            }
            let acquired = self.0.load(Ordering::Acquire);
            if acquired != word {
                word = acquired;
                continue;
            }
            if weak.0.load(Ordering::Acquire) & RETIRED != 0 {
                return None;
            }
            let refreshed = self.0.load(Ordering::Acquire);
            if refreshed != word {
                word = refreshed;
                continue;
            }
            if weak.0.load(Ordering::Acquire) & RETIRED != 0 {
                return None;
            }
            return Some(word);
        }
    }
}
#[cfg(target_has_atomic = "ptr")]
impl Counter for AtomicCount {
    #[inline]
    unsafe fn new(value: usize) -> Self {
        Self(AtomicUsize::new(value))
    }
    #[inline]
    fn load(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
    #[inline]
    fn increment(&self, live: bool) -> Result<bool, Error> {
        let mut old = self.0.load(Ordering::Relaxed);
        loop {
            if live && old == 0 {
                return Ok(false);
            }
            if old == LOCK {
                #[cfg(not(jzap_loom))]
                core::hint::spin_loop();
                #[cfg(jzap_loom)]
                loom::thread::yield_now();
                old = self.0.load(Ordering::Relaxed);
                continue;
            }
            let next = admitted_count(old)?;
            match self
                .0
                .compare_exchange(old, next, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => return Ok(true),
                Err(observed) => old = observed,
            }
        }
    }
    #[inline]
    fn clone_strong(&self) -> Result<(), Error> {
        if !BUFFERED_STRONG {
            return self.increment(false).map(|_| ());
        }
        self.buffered_clone_strong()
    }
    #[inline]
    fn clone_weak(&self) -> Result<(), Error> {
        let mut old = self.0.load(Ordering::Relaxed);
        loop {
            if old == LOCK {
                #[cfg(not(jzap_loom))]
                core::hint::spin_loop();
                #[cfg(jzap_loom)]
                loom::thread::yield_now();
                old = self.0.load(Ordering::Relaxed);
                continue;
            }
            if old & !RETIRED >= WEAK_MAX_COUNT {
                return Err(Error::RefCount);
            }
            let next = old.checked_add(1).ok_or(Error::RefCount)?;
            match self
                .0
                .compare_exchange(old, next, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => return Ok(()),
                Err(observed) => old = observed,
            }
        }
    }
    #[inline]
    fn upgrade(&self, weak: &Self) -> Result<bool, Error> {
        loop {
            let Some(old) = self.observed_live_word(weak) else {
                return Ok(false);
            };
            let next = admitted_count(old)?;
            if self
                .0
                .compare_exchange(old, next, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(true);
            }
        }
    }
    #[inline]
    fn initial_weak(retired: bool) -> usize {
        if retired { RETIRED | 1 } else { 1 }
    }
    #[inline]
    fn live_count(&self, weak: &Self) -> usize {
        self.observed_live_word(weak)
            .map_or(0, |n| n.min(MAX_COUNT))
    }
    #[inline]
    fn external_weaks(&self) -> usize {
        let word = self.0.load(Ordering::Relaxed);
        if word == LOCK || word & RETIRED != 0 {
            0
        } else {
            word.saturating_sub(1)
        }
    }
    #[inline]
    fn release_weak(&self) -> bool {
        let old = self.0.fetch_sub(1, Ordering::Release);
        if old & !RETIRED != 1 {
            return false;
        }
        fence(Ordering::Acquire);
        true
    }
    #[inline]
    fn release(&self) -> bool {
        if self.0.fetch_sub(1, Ordering::Release) != 1 {
            return false;
        }
        fence(Ordering::Acquire);
        true
    }
    #[inline]
    fn take_last(&self) -> bool {
        self.0
            .compare_exchange(1, 0, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }
    #[inline]
    fn publish(&self, weak: &Self) {
        if weak.0.load(Ordering::Relaxed) & RETIRED != 0 {
            self.0.store(0, Ordering::Relaxed);
            weak.0.fetch_and(!RETIRED, Ordering::Release);
        }
        self.0.store(1, Ordering::Release);
    }
    #[inline]
    fn retire(&self, weak: &Self, bytes: usize) {
        // The initialized payload's final strong transition has acquired prior
        // releases. A sole implicit weak has no remaining weak-creation source.
        if weak.0.load(Ordering::Acquire) == 1 {
            weak.0.store(RETIRED | 1, Ordering::Release);
        } else {
            weak.0.fetch_or(RETIRED, Ordering::Release);
        }
        self.0.store(retired_state(bytes), Ordering::Release);
    }
    #[inline]
    fn unique(strong: &Self, weak: &Self) -> bool {
        if weak
            .0
            .compare_exchange(1, LOCK, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return false;
        }
        let unique = strong.0.load(Ordering::Acquire) == 1;
        weak.0.store(1, Ordering::Release);
        unique
    }
}
