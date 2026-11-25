//! Reproduction (b): consuming a `Filter` over a range whose closure borrows
//! an array of small `Copy` structs.
//!
//! `PartialLookup::unresolved_keys` is `(0..len).filter(needs_key)`, where
//! `needs_key` reads `keys.get(index)` and compares the stored inline bytes
//! of neighbouring keys. `Key` mirrors `PrefixKey<'a, 1>` (Rust layout: the
//! one-byte inline array is placed after the `usize` and the fat pointer, at
//! a nonzero offset). `KeyFirst` is the same key with `#[repr(C)]` and the
//! inline array first, at offset zero: the control for the CBMC fault found
//! with reproduction (a).
//!
//! The property: the filter yields exactly the indexes a concrete loop
//! selects with the same predicate.
#![cfg_attr(not(any(test, kani)), no_std)]

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
            pub fn stored(full: &[u8]) -> Self {
                let mut prefix = [0; 1];
                for (slot, byte) in prefix.iter_mut().zip(full) {
                    *slot = *byte;
                }
                Self { prefix, len: full.len(), full: None }
            }

            pub fn inline(&self) -> &[u8] {
                self.prefix.get(..self.len.min(1)).unwrap_or(self.prefix.as_slice())
            }

            pub fn attach(&mut self, full: &'a [u8]) {
                self.full = Some(full);
            }

            pub fn full(&self) -> Option<&[u8]> {
                self.full.or_else(|| (self.len <= 1).then(|| self.inline()))
            }

            /// Undecided by the stored parts: both longer than the inline
            /// part, with equal inline bytes.
            pub fn undecided(&self, other: &Self) -> bool {
                self.len > 1 && other.len > 1 && self.inline() == other.inline()
            }
        }

        impl<'a> Build<'a> for $name<'a> {
            fn build(full: &[u8]) -> Self {
                Self::stored(full)
            }
            fn complete(&mut self, full: &'a [u8]) {
                self.attach(full)
            }
        }

        impl<'a> Keyed for $name<'a> {
            fn needs(keys: &[Self], index: usize) -> bool {
                let Some(key) = keys.get(index) else {
                    return false;
                };
                if key.full().is_some() {
                    return false;
                }
                let left = index
                    .checked_sub(1)
                    .and_then(|left| keys.get(left))
                    .is_some_and(|left| left.undecided(key));
                let right = index
                    .checked_add(1)
                    .and_then(|right| keys.get(right))
                    .is_some_and(|right| key.undecided(right));
                left || right
            }
        }
    };
}

pub trait Keyed: Sized {
    /// Whether the key at `index` is unresolved and undecided against a
    /// neighbour: the shape of `needs_key`.
    fn needs(keys: &[Self], index: usize) -> bool;
}

/// Construction, so that the harnesses and tests are generic over layout.
pub trait Build<'a>: Keyed + Copy {
    fn build(full: &[u8]) -> Self;
    fn complete(&mut self, full: &'a [u8]);
}

key_type!(Key,);
key_type!(KeyFirst, repr(C));

/// The indexes `needs` selects, as `unresolved_keys` lists them.
pub fn listed<K: Keyed>(keys: &[K]) -> impl Iterator<Item = usize> + '_ {
    (0..keys.len()).filter(move |&index| K::needs(keys, index))
}

/// The same selection with a concrete loop, as a bit mask.
pub fn listed_by_loop<K: Keyed>(keys: &[K]) -> u32 {
    let mut mask = 0;
    let mut index = 0;
    while index < keys.len() && index < 32 {
        if K::needs(keys, index) {
            mask |= 1 << index;
        }
        index += 1;
    }
    mask
}

/// The filter's output as a bit mask; `None` if it yields an index twice or
/// out of order.
pub fn listed_by_filter<K: Keyed>(keys: &[K]) -> Option<u32> {
    let mut mask = 0u32;
    let mut previous: Option<usize> = None;
    for index in listed(keys) {
        if previous.is_some_and(|p| p >= index) || index >= 32 {
            return None;
        }
        previous = Some(index);
        mask |= 1 << index;
    }
    Some(mask)
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

    /// The completions of three keys: arbitrary strings of up to `M` bytes.
    fn strings(raw: &[([u8; M], usize); N]) -> [&[u8]; N] {
        [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1], &raw[2].0[..raw[2].1]]
    }

    /// Keys built from arbitrary strings, some attached at random.
    fn check_filter_matches_loop<'a, K: Build<'a>>(completed: [&'a [u8]; N]) {
        let mut keys: [K; N] = [K::build(completed[0]), K::build(completed[1]), K::build(completed[2])];
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                key.complete(full);
            }
        }
        assert!(listed_by_filter(&keys) == Some(listed_by_loop(&keys)));
    }

    fn check_filter_count_matches_loop<'a, K: Build<'a>>(completed: [&'a [u8]; N]) {
        let mut keys: [K; N] = [K::build(completed[0]), K::build(completed[1]), K::build(completed[2])];
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                key.complete(full);
            }
        }
        assert!(listed(&keys).count() == listed_by_loop(&keys).count_ones() as usize);
    }

    /// The filter consumed by a `for` loop yields the loop's mask.
    #[kani::proof]
    #[kani::unwind(5)]
    fn b1_filter_matches_loop() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes(), bytes()];
        check_filter_matches_loop::<Key<'_>>(strings(&raw));
    }

    /// `count()` of the filter equals the loop's count.
    #[kani::proof]
    #[kani::unwind(5)]
    fn b2_filter_count_matches_loop() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes(), bytes()];
        check_filter_count_matches_loop::<Key<'_>>(strings(&raw));
    }

    /// Control of b1: the inline array at offset zero.
    #[kani::proof]
    #[kani::unwind(5)]
    fn b3_offset_zero_filter_matches_loop() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes(), bytes()];
        check_filter_matches_loop::<KeyFirst<'_>>(strings(&raw));
    }

    /// Control of b2: the inline array at offset zero.
    #[kani::proof]
    #[kani::unwind(5)]
    fn b4_offset_zero_filter_count_matches_loop() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes(), bytes()];
        check_filter_count_matches_loop::<KeyFirst<'_>>(strings(&raw));
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

    fn check<'a, K: Keyed + Copy>(keys: [K; N]) {
        assert_eq!(listed_by_filter(&keys), Some(listed_by_loop(&keys)));
        assert_eq!(listed(&keys).count(), listed_by_loop(&keys).count_ones() as usize);
    }

    /// Every triple of strings over {0, 1, 2} of up to two bytes, with every
    /// attachment mask, for both layouts.
    #[test]
    fn filter_properties_hold_natively() {
        let all = strings();
        for x in &all {
            for y in &all {
                for z in &all {
                    let raw = [*x, *y, *z];
                    let completed: [&[u8]; N] =
                        [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1], &raw[2].0[..raw[2].1]];
                    for mask in 0..8u8 {
                        let mut keys = completed.map(Key::stored);
                        let mut first = completed.map(KeyFirst::stored);
                        for index in 0..N {
                            if mask & (1 << index) != 0 {
                                keys[index].attach(completed[index]);
                                first[index].attach(completed[index]);
                            }
                        }
                        check(keys);
                        check(first);
                    }
                }
            }
        }
    }
}
