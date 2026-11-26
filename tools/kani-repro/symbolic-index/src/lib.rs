//! Reproduction (a): a symbolic index into an array of small `Copy` structs
//! that hold an inline byte array and an `Option<&[u8]>`.
//!
//! The struct mirrors `PrefixKey<'a, 1>` of
//! `src/collections/btree_map/view/bplus/partial.rs` (no dependency on jzap):
//! one inline byte, the full length and an optionally attached full key.
//! `attach` accepts a full key only when its length and inline bytes match.
//!
//! Each key is built from its own completion, so attaching the completion of
//! the same index always succeeds. The harnesses select that index
//! symbolically, as the removed lookup harness did with `keys[requested]`.
#![cfg_attr(not(any(test, kani)), no_std)]

#[derive(Clone, Copy, Debug)]
pub struct Key<'a, const P: usize> {
    prefix: [u8; P],
    len: usize,
    full: Option<&'a [u8]>,
}

fn inline<const P: usize>(prefix: &[u8; P], len: usize) -> &[u8] {
    prefix.get(..len.min(P)).unwrap_or(prefix.as_slice())
}

fn admits(a: &[u8], a_len: usize, full: &[u8]) -> bool {
    full.len() == a_len && full.get(..a.len()) == Some(a)
}

impl<'a, const P: usize> Key<'a, P> {
    /// The stored parts of `full`: its first `P` bytes and its length.
    pub fn stored(full: &[u8]) -> Self {
        let mut prefix = [0; P];
        for (slot, byte) in prefix.iter_mut().zip(full) {
            *slot = *byte;
        }
        Self { prefix, len: full.len(), full: None }
    }

    pub fn inline(&self) -> &[u8] {
        inline(&self.prefix, self.len)
    }

    /// The complete key: attached, or inline when it fits.
    pub fn full(&self) -> Option<&[u8]> {
        self.full.or_else(|| (self.len <= P).then(|| inline(&self.prefix, self.len)))
    }

    /// Whether `full` is a completion of the stored parts.
    pub fn admits(&self, full: &[u8]) -> bool {
        admits(self.inline(), self.len, full)
    }

    pub fn attach(&mut self, full: &'a [u8]) -> Result<(), ()> {
        if !self.admits(full) {
            return Err(());
        }
        self.full = Some(full);
        Ok(())
    }
}

/// Keys of up to `M` bytes stored with `P` inline bytes, `N` of them.
pub const P: usize = 1;
pub const M: usize = 2;
pub const N: usize = 2;

#[cfg(kani)]
mod proofs {
    use super::*;

    fn bytes() -> ([u8; M], usize) {
        let len = usize::from(kani::any::<u8>());
        kani::assume(len <= M);
        (kani::any(), len)
    }

    /// A1: plain read of a field through a symbolic index. Every key admits
    /// its own completion (checked with a concrete loop first), so the key at
    /// a symbolic index below `N` admits it as well.
    #[kani::proof]
    #[kani::unwind(4)]
    fn a1_symbolic_index_admits_own_completion() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes()];
        let completed: [&[u8]; N] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let keys: [Key<'_, P>; N] = [Key::stored(completed[0]), Key::stored(completed[1])];
        for (key, full) in keys.iter().zip(completed) {
            assert!(key.admits(full));
        }
        let requested: usize = kani::any();
        kani::assume(requested < N);
        assert!(keys[requested].admits(completed[requested]));
    }

    /// A2: the removed harness shape. Some keys are attached at random; the
    /// key at a symbolic index, when unresolved, accepts its completion.
    #[kani::proof]
    #[kani::unwind(4)]
    fn a2_symbolic_index_attach_after_random_attachments() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes()];
        let completed: [&[u8]; N] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let mut keys: [Key<'_, P>; N] = [Key::stored(completed[0]), Key::stored(completed[1])];
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                assert!(key.attach(full).is_ok());
            }
        }
        let requested: usize = kani::any();
        kani::assume(requested < N);
        if keys[requested].full().is_none() {
            assert!(keys[requested].attach(completed[requested]).is_ok());
        }
    }

    /// A3: the control of A2. The same key is selected with a concrete loop,
    /// as the current product harnesses do.
    #[kani::proof]
    #[kani::unwind(4)]
    fn a3_concrete_loop_attach_after_random_attachments() {
        let raw: [([u8; M], usize); N] = [bytes(), bytes()];
        let completed: [&[u8]; N] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
        let mut keys: [Key<'_, P>; N] = [Key::stored(completed[0]), Key::stored(completed[1])];
        for (key, full) in keys.iter_mut().zip(completed) {
            if kani::any() {
                assert!(key.attach(full).is_ok());
            }
        }
        let requested: usize = kani::any();
        kani::assume(requested < N);
        for (index, (key, full)) in keys.iter_mut().zip(completed).enumerate() {
            if index == requested && key.full().is_none() {
                assert!(key.attach(full).is_ok());
            }
        }
    }
    #[derive(Clone, Copy)]
    #[repr(C)]
    struct MemberAtOne {
        a: u8,
        prefix: [u8; 1],
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    struct MemberAtZero {
        prefix: [u8; 1],
        a: u8,
    }

    /// A4: the reduced form of A1. A reference to an array-typed member at
    /// offset 1 of the element at a symbolic index, then a read of its only
    /// byte. CBMC reads `keys[0].prefix[2 * r]` instead (see the report).
    #[kani::proof]
    fn a4_member_array_at_nonzero_offset() {
        let v: [u8; 2] = [kani::any(), kani::any()];
        let keys = [MemberAtOne { a: 9, prefix: [v[0]] }, MemberAtOne { a: 9, prefix: [v[1]] }];
        let r: usize = kani::any();
        kani::assume(r < 2);
        let p = &keys[r].prefix;
        assert!(p[0] == v[r]);
    }

    /// A5: the control of A4, with the array member at offset 0.
    #[kani::proof]
    fn a5_member_array_at_offset_zero() {
        let v: [u8; 2] = [kani::any(), kani::any()];
        let keys = [MemberAtZero { prefix: [v[0]], a: 9 }, MemberAtZero { prefix: [v[1]], a: 9 }];
        let r: usize = kani::any();
        kani::assume(r < 2);
        let p = &keys[r].prefix;
        assert!(p[0] == v[r]);
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    /// Every byte string of up to `M` bytes over the bytes {0, 1, 2}.
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

    /// The properties of A1–A3, for every pair of strings, attachment mask
    /// and index. Bytes beyond the length do not affect the keys.
    #[test]
    fn symbolic_index_properties_hold_natively() {
        let all = strings();
        for x in &all {
            for y in &all {
                let raw = [*x, *y];
                let completed: [&[u8]; N] = [&raw[0].0[..raw[0].1], &raw[1].0[..raw[1].1]];
                for mask in 0..4u8 {
                    let mut keys: [Key<'_, P>; N] = [Key::stored(completed[0]), Key::stored(completed[1])];
                    for (index, (key, full)) in keys.iter_mut().zip(completed).enumerate() {
                        assert!(key.admits(full));
                        if mask & (1 << index) != 0 {
                            assert!(key.attach(full).is_ok());
                        }
                    }
                    for requested in 0..N {
                        let mut copy = keys;
                        if copy[requested].full().is_none() {
                            assert!(copy[requested].attach(completed[requested]).is_ok());
                        }
                    }
                }
            }
        }
    }

    /// The property of A4 and A5 for every byte pair and index.
    #[test]
    fn member_array_reads_hold_natively() {
        #[derive(Clone, Copy)]
        #[repr(C)]
        struct MemberAtOne {
            a: u8,
            prefix: [u8; 1],
        }
        #[derive(Clone, Copy)]
        #[repr(C)]
        struct MemberAtZero {
            prefix: [u8; 1],
            a: u8,
        }
        for x in 0..=255u8 {
            for y in 0..=255u8 {
                let v = [x, y];
                let one = [MemberAtOne { a: 9, prefix: [x] }, MemberAtOne { a: 9, prefix: [y] }];
                let zero = [MemberAtZero { prefix: [x], a: 9 }, MemberAtZero { prefix: [y], a: 9 }];
                for r in 0..2 {
                    assert_eq!(one[r].a, 9);
                    assert_eq!(zero[r].a, 9);
                    let p = &one[r].prefix;
                    assert_eq!(p[0], v[r]);
                    let p = &zero[r].prefix;
                    assert_eq!(p[0], v[r]);
                }
            }
        }
    }
}
