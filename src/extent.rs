//! Closed numeric owner state of contiguous storage: length, capacity and limit.
//!
//! Every value satisfies `len <= cap <= limit`, and the capacity of a sized
//! element spans at most `isize::MAX` bytes (the `Capacity` invariant). The
//! fields are private to this module. Every constructor establishes the
//! invariant and every transition preserves it for arbitrary arguments: a
//! transition checks what it needs and refuses or clamps, and none has a
//! numeric precondition. The one exception is `from_raw_parts`, the named
//! trusted entry whose precondition is the public unsafe contract of the
//! owner's `from_raw_parts`.
//!
//! The state is numeric only. Which elements are initialized, and that the
//! storage is live, remain the obligations of the owner's unsafe blocks.
use crate::capacity::Capacity;
#[cfg(verus_keep_ghost)]
use crate::capacity::byte_valid;
#[cfg(verus_keep_ghost)]
use crate::vec::sealed::Sealed;
use crate::vec::{LengthPolicy, Unlimited};
use crate::{Checked, Error};
use core::mem;
#[cfg(verus_keep_ghost)]
use vstd::{arithmetic::div_mod::*, prelude::*};

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct Extent<T, L: LengthPolicy> {
    len: Checked<usize>,
    cap: Capacity<T>,
    limit: L,
}
// A copy of a valid state is a valid state; `T` itself is never copied.
impl<T, L: LengthPolicy> Copy for Extent<T, L> {}
impl<T, L: LengthPolicy> Clone for Extent<T, L> {
    fn clone(&self) -> Self {
        *self
    }
}
#[cfg(verus_keep_ghost)]
verus! {
impl<T, L: LengthPolicy> Extent<T, L> {
    pub(crate) closed spec fn len_spec(self) -> usize { self.len@ }
    pub(crate) closed spec fn cap_spec(self) -> usize { self.cap@ }
    pub(crate) closed spec fn limit_spec(self) -> usize { self.limit.bound() }
    #[verifier::type_invariant]
    spec fn wf(self) -> bool {
        self.len@ <= self.cap@ <= self.limit.bound()
    }
    /// The invariant of every value of this type.
    pub(crate) proof fn invariant(tracked &self)
        ensures
            self.len_spec() <= self.cap_spec() <= self.limit_spec(),
            vstd::layout::size_of::<T>() != 0
                ==> byte_valid(self.cap_spec(), vstd::layout::size_of::<T>() as usize),
    {
        use_type_invariant(self);
        self.cap.bytes_fit();
    }
}
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl<T> Extent<T, Unlimited> {
    /// The empty state without a limit, for constant construction.
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.len_spec() == 0, out.limit_spec() == usize::MAX,
            out.cap_spec() == if vstd::layout::size_of::<T>() == 0 { usize::MAX } else { 0usize },
    ))]
    pub(crate) const fn unlimited() -> Self {
        Self {
            len: Checked::new(0),
            cap: Capacity::empty(usize::MAX),
            limit: Unlimited,
        }
    }
    /// A state without a limit over existing storage. The length is clamped
    /// to the capacity, so every argument yields a valid state.
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.cap_spec() == capacity@, out.limit_spec() == usize::MAX,
            out.len_spec() == if len <= capacity@ { len } else { capacity@ },
    ))]
    pub(crate) fn unlimited_clamped(len: usize, capacity: Capacity<T>) -> Self {
        let cap = capacity.get();
        Self {
            len: Checked::new(if len <= cap { len } else { cap }),
            cap: capacity,
            limit: Unlimited,
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl<T, L: LengthPolicy> Extent<T, L> {
    /// The empty state under `limit`. A zero-sized element needs no storage,
    /// so its capacity is the limit.
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.len_spec() == 0, out.limit_spec() == limit.bound(),
            out.cap_spec() == if vstd::layout::size_of::<T>() == 0 { limit.bound() } else { 0usize },
    ))]
    pub(crate) fn empty(limit: L) -> Self {
        Self {
            len: Checked::new(0),
            cap: Capacity::empty(limit.max_len()),
            limit,
        }
    }
    /// A state over existing storage. Refuses a length above the capacity and
    /// a capacity above the limit.
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(extent) => len <= capacity@ <= limit.bound() && extent.len_spec() == len
                && extent.cap_spec() == capacity@ && extent.limit_spec() == limit.bound(),
            Err(Error::Capacity) => len > capacity@ || capacity@ > limit.bound(),
            _ => false,
        },
    ))]
    pub(crate) fn try_new(len: usize, capacity: Capacity<T>, limit: L) -> Result<Self, Error> {
        let cap = capacity.get();
        if len > cap || cap > limit.max_len() {
            return Err(Error::Capacity);
        }
        Ok(Self {
            len: Checked::new(len),
            cap: capacity,
            limit,
        })
    }
    /// # Safety
    /// `len <= capacity <= limit`, as the public `from_raw_parts` contract of
    /// the owner requires.
    ///
    /// The trusted entry of this type: nothing is checked at run time.
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires len <= capacity@ <= limit.bound(),
        ensures out.len_spec() == len, out.cap_spec() == capacity@,
            out.limit_spec() == limit.bound(),
    ))]
    pub(crate) unsafe fn from_raw_parts(len: usize, capacity: Capacity<T>, limit: L) -> Self {
        Self {
            len: Checked::new(len),
            cap: capacity,
            limit,
        }
    }

    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self.len_spec(),))]
    pub(crate) fn len(&self) -> usize {
        self.len.get()
    }
    /// The length as a checked integer, for checked arithmetic at the caller.
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out@ == self.len_spec(),))]
    pub(crate) fn length(&self) -> Checked<usize> {
        self.len
    }
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self.cap_spec(),))]
    pub(crate) fn cap(&self) -> usize {
        self.cap.get()
    }
    /// The capacity with its element type, for its layout.
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out@ == self.cap_spec(),))]
    pub(crate) fn capacity(&self) -> Capacity<T> {
        self.cap
    }
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out.bound() == self.limit_spec(),))]
    pub(crate) fn limit(&self) -> L {
        self.limit
    }
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self.limit_spec(),))]
    pub(crate) fn max_len(&self) -> usize {
        self.limit.max_len()
    }

    /// Set the length to `needed` when the capacity holds it. Refusal changes
    /// nothing.
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            final(self).cap_spec() == old(self).cap_spec(),
            final(self).limit_spec() == old(self).limit_spec(),
            match out {
                Ok(()) => needed <= old(self).cap_spec() && final(self).len_spec() == needed,
                Err(Error::Capacity) => needed > old(self).cap_spec()
                    && final(self).len_spec() == old(self).len_spec(),
                _ => false,
            },
    ))]
    pub(crate) fn set_len_within(&mut self, needed: usize) -> Result<(), Error> {
        #[cfg(verus_keep_ghost)]
        proof! {
            use_type_invariant(&*self);
        }
        if needed > self.cap.get() {
            return Err(Error::Capacity);
        }
        self.len = Checked::new(needed);
        Ok(())
    }
    /// Set the length to `next`, clamped to the capacity. For guards that
    /// restore a length while dropping and cannot return a refusal.
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures
            final(self).cap_spec() == old(self).cap_spec(),
            final(self).limit_spec() == old(self).limit_spec(),
            final(self).len_spec()
                == if next <= old(self).cap_spec() { next } else { old(self).cap_spec() },
    ))]
    pub(crate) fn set_len_clamped(&mut self, next: usize) {
        #[cfg(verus_keep_ghost)]
        proof! {
            use_type_invariant(&*self);
        }
        let cap = self.cap.get();
        self.len = Checked::new(if next <= cap { next } else { cap });
    }
    /// Set the length to `next` when that does not lengthen; a larger `next`
    /// leaves the length unchanged.
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures
            final(self).cap_spec() == old(self).cap_spec(),
            final(self).limit_spec() == old(self).limit_spec(),
            final(self).len_spec()
                == if next <= old(self).len_spec() { next } else { old(self).len_spec() },
    ))]
    pub(crate) fn shorten_to(&mut self, next: usize) {
        #[cfg(verus_keep_ghost)]
        proof! {
            use_type_invariant(&*self);
        }
        if next < self.len.get() {
            self.len = Checked::new(next);
        }
    }
    /// The same length and limit over a capacity that holds `needed`
    /// elements: exactly `needed` when `exact`, otherwise the amortized growth
    /// of the current capacity. Refuses `needed` below the length or above the
    /// limit, and an array beyond the address space.
    ///
    /// It returns a new state instead of changing this one: the owner obtains
    /// it before it replaces the storage and publishes it afterwards without
    /// a second refusal. Both comparisons repeat what a reservation has just
    /// decided (`needed` is the length plus a count, within the limit), and
    /// the capacity comes from the verified growth rule, so no test is added
    /// to a growth path.
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(extent) => self.len_spec() <= needed <= extent.cap_spec()
                && extent.len_spec() == self.len_spec()
                && extent.limit_spec() == self.limit_spec()
                && (exact ==> extent.cap_spec() == needed),
            Err(Error::Capacity) => needed < self.len_spec() || needed > self.limit_spec(),
            Err(Error::Layout) => self.len_spec() <= needed <= self.limit_spec()
                && needed as int > if vstd::layout::size_of::<T>() == 0 { usize::MAX as int }
                    else { isize::MAX as int / vstd::layout::size_of::<T>() as int },
            _ => false,
        },
    ))]
    pub(crate) fn holding(&self, needed: usize, exact: bool) -> Result<Self, Error> {
        let limit = self.limit.max_len();
        if needed < self.len.get() || needed > limit {
            return Err(Error::Capacity);
        }
        let size = mem::size_of::<T>();
        let count =
            crate::vec::geometry::growth_for_layout(self.cap.get(), needed, limit, size, exact)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            if size > 0 {
                lemma_fundamental_div_mod(isize::MAX as int, size as int);
                assert(count as int * size as int <= isize::MAX as int) by(nonlinear_arith)
                    requires count as int <= isize::MAX as int / size as int, size > 0,
                        isize::MAX as int == size as int * (isize::MAX as int / size as int)
                            + isize::MAX as int % size as int,
                        0 <= isize::MAX as int % size as int;
            }
        }
        // SAFETY: the growth rule bounds the count by the array ceiling of the
        // element size, which is the contract of the trusted entry.
        let cap = unsafe { Capacity::from_validated_raw(count) };
        Ok(Self {
            len: self.len,
            cap,
            limit: self.limit,
        })
    }
    /// The same length and limit over `capacity`. Refuses a capacity below
    /// the length or above the limit.
    #[inline(always)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(extent) => self.len_spec() <= capacity@ <= self.limit_spec()
                && extent.len_spec() == self.len_spec() && extent.cap_spec() == capacity@
                && extent.limit_spec() == self.limit_spec(),
            Err(Error::Capacity) => capacity@ < self.len_spec() || capacity@ > self.limit_spec(),
            _ => false,
        },
    ))]
    pub(crate) fn resized(&self, capacity: Capacity<T>) -> Result<Self, Error> {
        #[cfg(verus_keep_ghost)]
        proof! {
            use_type_invariant(&*self);
        }
        let cap = capacity.get();
        if cap < self.len.get() || cap > self.limit.max_len() {
            return Err(Error::Capacity);
        }
        Ok(Self {
            len: self.len,
            cap: capacity,
            limit: self.limit,
        })
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;
    use crate::vec::LengthLimit;

    fn valid<T, L: LengthPolicy>(extent: &Extent<T, L>) -> bool {
        extent.len() <= extent.cap() && extent.cap() <= extent.max_len()
    }
    fn units(count: usize) -> Capacity<()> {
        // SAFETY: every unit-array count has a valid zero-byte layout.
        unsafe { Capacity::from_validated_raw(count) }
    }

    #[kani::proof]
    fn constructors_establish_the_order_for_every_word() {
        let len: usize = kani::any();
        let cap: usize = kani::any();
        let limit: usize = kani::any();
        let empty = Extent::<(), _>::empty(LengthLimit::new(limit));
        assert!(valid(&empty) && empty.len() == 0 && empty.cap() == limit);
        let sized = Extent::<u64, _>::empty(LengthLimit::new(limit));
        assert!(valid(&sized) && sized.len() == 0 && sized.cap() == 0);
        assert!(sized.max_len() == limit && sized.limit().max_len() == limit);
        let unlimited = Extent::<u64, Unlimited>::unlimited();
        assert!(valid(&unlimited) && unlimited.len() == 0 && unlimited.cap() == 0);
        assert!(unlimited.max_len() == usize::MAX);
        match Extent::try_new(len, units(cap), LengthLimit::new(limit)) {
            Ok(extent) => {
                assert!(len <= cap && cap <= limit);
                assert!(extent.len() == len && extent.cap() == cap && extent.max_len() == limit);
                assert!(extent.length().get() == len && extent.capacity().get() == cap);
            }
            Err(error) => assert!(error == Error::Capacity && (len > cap || cap > limit)),
        }
        let clamped = Extent::unlimited_clamped(len, units(cap));
        assert!(valid(&clamped) && clamped.cap() == cap && clamped.max_len() == usize::MAX);
        assert!(clamped.len() == if len <= cap { len } else { cap });
    }

    #[kani::proof]
    fn every_transition_preserves_the_order_for_every_state_and_argument() {
        let Ok(mut extent) = Extent::try_new(
            kani::any(),
            units(kani::any()),
            LengthLimit::new(kani::any()),
        ) else {
            return;
        };
        let (len, cap, limit) = (extent.len(), extent.cap(), extent.max_len());
        let argument: usize = kani::any();
        if kani::any() {
            if kani::any() {
                let result = extent.set_len_within(argument);
                if argument <= cap {
                    assert!(result == Ok(()) && extent.len() == argument);
                } else {
                    assert!(result == Err(Error::Capacity) && extent.len() == len);
                }
            } else {
                extent.set_len_clamped(argument);
                assert!(extent.len() == if argument <= cap { argument } else { cap });
            }
        } else if kani::any() {
            extent.shorten_to(argument);
            assert!(extent.len() == if argument <= len { argument } else { len });
        } else {
            match extent.resized(units(argument)) {
                Ok(next) => {
                    assert!(len <= argument && argument <= limit && valid(&next));
                    assert!(next.len() == len && next.cap() == argument);
                    assert!(next.max_len() == limit);
                }
                Err(error) => {
                    assert!(error == Error::Capacity && (argument < len || argument > limit))
                }
            }
            assert!(extent.len() == len);
        }
        assert!(valid(&extent) && extent.cap() == cap && extent.max_len() == limit);
    }
}
