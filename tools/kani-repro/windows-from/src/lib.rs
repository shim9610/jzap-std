//! Reproduction (c): `slice.get(start..).windows(2)` with a symbolic `start`
//! over an array of small `Copy` structs.
//!
//! `PartialLookup::pass` resumes its adjacent-pair loop with
//! `keys.get(progress.pairs..).unwrap_or(&[]).windows(2)` and stops at the
//! first pair its stored parts cannot order. `Key` mirrors
//! `PrefixKey<'a, 1>` (Rust layout: the one-byte inline array is not at
//! offset zero); `KeyFirst` is the same key with `#[repr(C)]` and the inline
//! array first, the control for the CBMC fault found with reproduction (a).
//!
//! The property: the resumed `windows(2)` loop gives what a loop over
//! concrete pair indexes gives, skipping the pairs below `start`.
#![cfg_attr(not(any(test, kani)), no_std)]

use core::cmp::Ordering;

/// What the pair loop ends with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pairs {
    /// Every pair from the start is ordered; the index of the last key, or
    /// the start when no pair was checked.
    Ordered(usize),
    /// The pair `(i, i + 1)` is undecided by the stored parts.
    Undecided(usize),
    /// A pair is out of order or equal.
    Invalid,
}

pub trait Stored: Sized + Copy {
    /// Order two keys from their stored parts; `None` when undecided.
    fn cmp_stored(&self, other: &Self) -> Option<Ordering>;
}

pub trait Build<'a>: Stored {
    fn build(full: &[u8]) -> Self;
    fn complete(&mut self, full: &'a [u8]);
}

macro_rules! key_type {
    ($name:ident, $($repr:meta)?) => {
        #[derive(Clone, Copy, Debug)]
        $(#[$repr])?
        pub struct $name<'a> {
            prefix: [u8; 1],
            len: usize,
            full: Option<&'a [u8]>,
        }

        impl<'a> $name<'a> {
            pub fn inline(&self) -> &[u8] {
                self.prefix.get(..self.len.min(1)).unwrap_or(self.prefix.as_slice())
            }
        }

        impl<'a> Stored for $name<'a> {
            fn cmp_stored(&self, other: &Self) -> Option<Ordering> {
                match self.inline().cmp(other.inline()) {
                    Ordering::Equal => {}
                    order => return Some(order),
                }
                match (self.len <= 1, other.len <= 1) {
                    // Equal inline bytes: a complete inline key is a prefix
                    // of the other key.
                    (true, true) => Some(Ordering::Equal),
                    (true, false) => Some(Ordering::Less),
                    (false, true) => Some(Ordering::Greater),
                    (false, false) => Some(self.full?.cmp(other.full?)),
                }
            }
        }

        impl<'a> Build<'a> for $name<'a> {
            fn build(full: &[u8]) -> Self {
                let mut prefix = [0; 1];
                for (slot, byte) in prefix.iter_mut().zip(full) {
                    *slot = *byte;
                }
                Self { prefix, len: full.len(), full: None }
            }

            fn complete(&mut self, full: &'a [u8]) {
                self.full = Some(full);
            }
        }
    };
}

key_type!(Key,);
key_type!(KeyFirst, repr(C));

fn order<K: Stored>(a: &K, b: &K, left: usize) -> Option<Pairs> {
    match a.cmp_stored(b) {
        Some(Ordering::Less) => None,
        Some(_) => Some(Pairs::Invalid),
        None => Some(Pairs::Undecided(left)),
    }
}

/// The product's loop shape: resume at `start` with `windows(2)`.
pub fn pairs_by_windows<K: Stored>(keys: &[K], start: usize) -> Pairs {
    let mut left = start;
    for pair in keys.get(left..).unwrap_or(&[]).windows(2) {
        let (Some(a), Some(b)) = (pair.first(), pair.last()) else {
            return Pairs::Invalid;
        };
        if let Some(end) = order(a, b, left) {
            return end;
        }
        left += 1;
    }
    Pairs::Ordered(left)
}

/// The reference: every pair index from zero, skipping those below `start`;
/// the pair index only takes the loop's own concrete values.
pub fn pairs_by_indexes<K: Stored, const L: usize>(keys: &[K; L], start: usize) -> Pairs {
    let mut last = start;
    for (left, pair) in keys.windows(2).enumerate() {
        if left < start {
            continue;
        }
        if let Some(end) = order(&pair[0], &pair[1], left) {
            return end;
        }
        last = left + 1;
    }
    // With no pair at or above `start` (including a start past the end,
    // where `get(start..)` is empty), the loop ends where it started.
    Pairs::Ordered(last)
}

pub const N: usize = 3;
pub const M: usize = 2;

#[cfg(kani)]
mod proofs {
    use super::*;

    fn bytes() -> ([u8; M], usize) {
        let len = usize::from(kani::any::<u8>());
        kani::assume(len <= M);
        (kani::any(), len)
    }

    fn strings(raw: &[([u8; M], usize); N]) -> [&[u8]; N] {
        [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1], &raw[2].0[..raw[2].1]]
    }

    fn keys<'a, K: Build<'a>>(completed: [&'a [u8]; N]) -> [K; N] {
        let mut keys = [K::build(completed[0]), K::build(completed[1]), K::build(completed[2])];
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                key.complete(full);
            }
        }
        keys
    }

    /// The resumed `windows(2)` loop from a symbolic start equals the loop
    /// over concrete pair indexes.
    fn check_symbolic_start<'a, K: Build<'a>>(completed: [&'a [u8]; N]) {
        let keys: [K; N] = keys(completed);
        let start: usize = kani::any();
        kani::assume(start <= N);
        assert!(pairs_by_windows(&keys, start) == pairs_by_indexes(&keys, start));
    }

    /// The same with a concrete start of zero: the control for the symbolic
    /// start.
    fn check_zero_start<'a, K: Build<'a>>(completed: [&'a [u8]; N]) {
        let keys: [K; N] = keys(completed);
        assert!(pairs_by_windows(&keys, 0) == pairs_by_indexes(&keys, 0));
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn c1_symbolic_start_windows_match_indexes() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes(), bytes()];
        check_symbolic_start::<Key<'_>>(strings(&raw));
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn c2_zero_start_windows_match_indexes() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes(), bytes()];
        check_zero_start::<Key<'_>>(strings(&raw));
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn c3_offset_zero_symbolic_start_windows_match_indexes() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes(), bytes()];
        check_symbolic_start::<KeyFirst<'_>>(strings(&raw));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings() -> Vec<([u8; M], usize)> {
        let mut all = Vec::new();
        for len in 0..=M {
            for a in 0..3u8 {
                for b in 0..3u8 {
                    let bytes = [a, b];
                    if bytes[len..].iter().all(|&x| x == 0) {
                        all.push((bytes, len));
                    }
                }
            }
        }
        all
    }

    fn check<'a, K: Build<'a>>(completed: [&'a [u8]; N], mask: u8) {
        let mut keys = completed.map(K::build);
        for (index, (key, full)) in keys.iter_mut().zip(completed).enumerate() {
            if mask & (1 << index) != 0 {
                key.complete(full);
            }
        }
        for start in 0..=N + 1 {
            assert_eq!(pairs_by_windows(&keys, start), pairs_by_indexes(&keys, start));
        }
    }

    /// Every triple of strings over {0, 1, 2} of up to two bytes, every
    /// attachment mask and every start up to `N + 1`, for both layouts.
    #[test]
    fn windows_properties_hold_natively() {
        let all = strings();
        for x in &all {
            for y in &all {
                for z in &all {
                    let raw = [*x, *y, *z];
                    let completed: [&[u8]; N] =
                        [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1], &raw[2].0[..raw[2].1]];
                    for mask in 0..8u8 {
                        check::<Key<'_>>(completed, mask);
                        check::<KeyFirst<'_>>(completed, mask);
                    }
                }
            }
        }
    }
}
