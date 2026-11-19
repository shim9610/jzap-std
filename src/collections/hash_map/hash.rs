//! SipHash-1-3 and per-thread keys with explicitly returned entropy failures.
//! No fixed fallback seed. Contract: docs/random-state.md.
use crate::{Error, TryClone, TryDefault};
#[cfg(all(
    feature = "std",
    any(
        target_os = "linux",
        target_os = "android",
        all(target_os = "windows", not(target_vendor = "win7")),
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "solaris"
    )
))]
use core::cell::Cell;
use core::hash::{BuildHasher, Hasher};

#[derive(Clone, Copy)]
pub struct RandomState {
    k0: u64,
    k1: u64,
}
impl RandomState {
    /// Explicit deterministic keys; callers choose their entropy/security policy.
    pub const fn from_seed(k0: u64, k1: u64) -> Self {
        Self { k0, k1 }
    }
    /// Returns this thread's keys and advances its `k0`, as std's
    /// `RandomState::new` does. The thread's first call, or the first call
    /// after a refusal, obtains OS entropy; refusal returns `Entropy` and leaves
    /// the thread uninitialized. Never panics or blocks. Platforms without a
    /// default source (see `entropy`) return `Entropy { code: 0 }`.
    #[inline]
    pub fn try_new() -> Result<Self, Error> {
        #[cfg(all(
            feature = "std",
            any(
                target_os = "linux",
                target_os = "android",
                all(target_os = "windows", not(target_vendor = "win7")),
                target_vendor = "apple",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "dragonfly",
                target_os = "haiku",
                target_os = "illumos",
                target_os = "solaris"
            )
        ))]
        {
            // A const, drop-free thread local is a plain native TLS static, so
            // access cannot fail; the error arm replaces `with`'s panic.
            match KEYS.try_with(|keys| next_keys(keys, super::entropy::os_seed)) {
                Ok(Ok((k0, k1))) => Ok(Self::from_seed(k0, k1)),
                Ok(Err(error)) => Err(error),
                Err(_) => Err(Error::Entropy { code: 0 }),
            }
        }
        #[cfg(not(all(
            feature = "std",
            any(
                target_os = "linux",
                target_os = "android",
                all(target_os = "windows", not(target_vendor = "win7")),
                target_vendor = "apple",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "dragonfly",
                target_os = "haiku",
                target_os = "illumos",
                target_os = "solaris"
            )
        )))]
        {
            Err(Error::Entropy { code: 0 })
        }
    }
}
impl core::fmt::Debug for RandomState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RandomState").finish_non_exhaustive()
    }
}
/// One thread's keys. The readiness flag is separate from the pair, so the
/// steady state stores only the advanced `k0`, as std's thread-local cell does.
/// No field needs drop, keeping the thread local a plain native TLS static.
#[cfg(all(
    feature = "std",
    any(
        target_os = "linux",
        target_os = "android",
        all(target_os = "windows", not(target_vendor = "win7")),
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "solaris"
    )
))]
struct ThreadKeys {
    keys: Cell<(u64, u64)>,
    ready: Cell<bool>,
}
#[cfg(all(
    feature = "std",
    any(
        target_os = "linux",
        target_os = "android",
        all(target_os = "windows", not(target_vendor = "win7")),
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "solaris"
    )
))]
impl ThreadKeys {
    const fn new() -> Self {
        Self {
            keys: Cell::new((0, 0)),
            ready: Cell::new(false),
        }
    }
}
#[cfg(all(
    feature = "std",
    any(
        target_os = "linux",
        target_os = "android",
        all(target_os = "windows", not(target_vendor = "win7")),
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "solaris"
    )
))]
std::thread_local! {
    static KEYS: ThreadKeys = const { ThreadKeys::new() };
}
/// Returns the current pair and stores `k0 + 1` (explicit key variation, not
/// memory arithmetic). A provider refusal or unwind leaves the thread unready.
#[cfg(all(
    feature = "std",
    any(
        target_os = "linux",
        target_os = "android",
        all(target_os = "windows", not(target_vendor = "win7")),
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "solaris"
    )
))]
#[inline]
fn next_keys(
    thread: &ThreadKeys,
    provider: impl FnOnce() -> Result<(u64, u64), Error>,
) -> Result<(u64, u64), Error> {
    if !thread.ready.get() {
        thread.keys.set(provider()?);
        thread.ready.set(true);
    }
    let (k0, k1) = thread.keys.get();
    thread.keys.set((k0.wrapping_add(1), k1));
    Ok((k0, k1))
}
impl TryClone for RandomState {
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(*self)
    }
}
impl TryDefault for RandomState {
    fn try_default() -> Result<Self, Error> {
        Self::try_new()
    }
}
impl BuildHasher for RandomState {
    type Hasher = SipHasher13;
    #[inline]
    fn build_hasher(&self) -> SipHasher13 {
        SipHasher13::new(self.k0, self.k1)
    }
}

#[derive(Clone)]
pub struct SipHasher13 {
    v0: u64,
    v1: u64,
    v2: u64,
    v3: u64,
    tail: u64,
    tail_bytes: u8,
    length: u8,
}
impl SipHasher13 {
    pub const fn new(k0: u64, k1: u64) -> Self {
        Self {
            v0: k0 ^ 0x736f6d6570736575,
            v1: k1 ^ 0x646f72616e646f6d,
            v2: k0 ^ 0x6c7967656e657261,
            v3: k1 ^ 0x7465646279746573,
            tail: 0,
            tail_bytes: 0,
            length: 0,
        }
    }
    #[inline(always)]
    fn round(&mut self) {
        self.v0 = self.v0.wrapping_add(self.v1);
        self.v2 = self.v2.wrapping_add(self.v3);
        self.v1 = self.v1.rotate_left(13) ^ self.v0;
        self.v3 = self.v3.rotate_left(16) ^ self.v2;
        self.v0 = self.v0.rotate_left(32);
        self.v2 = self.v2.wrapping_add(self.v1);
        self.v0 = self.v0.wrapping_add(self.v3);
        self.v1 = self.v1.rotate_left(17) ^ self.v2;
        self.v3 = self.v3.rotate_left(21) ^ self.v0;
        self.v2 = self.v2.rotate_left(32);
    }
    #[inline(always)]
    fn block(&mut self, word: u64) {
        self.v3 ^= word;
        self.round();
        self.v0 ^= word;
    }
    /// The same state change as `write` of `size` (1..=8) bytes whose
    /// little-endian value is `word`; bits of `word` above `size` bytes are
    /// zero. No slice walk: the bytes join the tail as one shifted word and at
    /// most one compression runs, so an integer write stays short even when the
    /// caller is not inlined and the tail length is unknown.
    #[inline(always)]
    fn short_write(&mut self, word: u64, size: u8) {
        self.length = self.length.wrapping_add(size);
        // tail_bytes is 0..=7, so the shift is at most 56 bits.
        let used = self.tail_bytes;
        self.tail |= word.wrapping_shl(u32::from(used).wrapping_shl(3));
        let filled = used.wrapping_add(size);
        if filled < 8 {
            self.tail_bytes = filled;
            return;
        }
        self.block(self.tail);
        // 8 - used bytes of word completed the block; keep the remaining
        // 0..=7 bytes. A rotation by 64 bits (empty tail) is the identity and
        // the mask is then zero, so no shift by the full width is needed.
        let rest = filled.wrapping_sub(8);
        let keep = 1u64
            .wrapping_shl(u32::from(rest).wrapping_shl(3))
            .wrapping_sub(1);
        self.tail = word.rotate_right(u32::from(8u8.wrapping_sub(used)).wrapping_shl(3)) & keep;
        self.tail_bytes = rest;
    }
}
/// Fewer than eight bytes as a little-endian word, read as at most one 4-byte,
/// one 2-byte and one 1-byte group like core's `u8to64_le`. Callers pass a
/// slice shorter than eight bytes; longer input would ignore bytes past seven.
#[inline(always)]
fn gather(mut bytes: &[u8]) -> u64 {
    let mut word = 0;
    let mut shift = 0u32;
    if let Some((four, rest)) = bytes.split_first_chunk::<4>() {
        word = u64::from(u32::from_le_bytes(*four));
        shift = 32;
        bytes = rest;
    }
    if let Some((two, rest)) = bytes.split_first_chunk::<2>() {
        word |= u64::from(u16::from_le_bytes(*two)).wrapping_shl(shift);
        shift = shift.wrapping_add(16);
        bytes = rest;
    }
    if let Some(one) = bytes.first() {
        word |= u64::from(*one).wrapping_shl(shift);
    }
    word
}
impl Hasher for SipHasher13 {
    /// Core's SipHash `write` order: complete a buffered tail with one gathered
    /// word, compress whole 8-byte words, then buffer the remaining 0..=7 bytes.
    /// At most one compression handles the tail, so fixed-size writes fold.
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // Only the low eight length bits participate in SipHash's final block.
        self.length = self.length.wrapping_add(bytes.len() as u8);
        let mut rest = bytes;
        if self.tail_bytes != 0 {
            // tail_bytes is 1..=7 here, so 1..=7 bytes complete the word.
            let needed = usize::from(8u8.wrapping_sub(self.tail_bytes));
            let (head, after) = rest.split_at_checked(needed).unwrap_or((rest, &[]));
            self.tail |= gather(head).wrapping_shl(u32::from(self.tail_bytes).wrapping_shl(3));
            if head.len() < needed {
                self.tail_bytes = self.tail_bytes.wrapping_add(head.len() as u8);
                return;
            }
            self.block(self.tail);
            rest = after;
        }
        while let Some((word, after)) = rest.split_first_chunk::<8>() {
            self.block(u64::from_le_bytes(*word));
            rest = after;
        }
        self.tail = gather(rest);
        self.tail_bytes = rest.len() as u8;
    }
    // core's defaults write `i.to_ne_bytes()`; reading those bytes as
    // little-endian gives the identical byte stream on every endianness.
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.short_write(u64::from(i), 1);
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.short_write(u64::from(u16::from_le_bytes(i.to_ne_bytes())), 2);
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.short_write(u64::from(u32::from_le_bytes(i.to_ne_bytes())), 4);
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.short_write(u64::from_le_bytes(i.to_ne_bytes()), 8);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        let bytes = i.to_ne_bytes();
        #[cfg(target_pointer_width = "64")]
        self.short_write(u64::from_le_bytes(bytes), 8);
        #[cfg(target_pointer_width = "32")]
        self.short_write(u64::from(u32::from_le_bytes(bytes)), 4);
        #[cfg(target_pointer_width = "16")]
        self.short_write(u64::from(u16::from_le_bytes(bytes)), 2);
        #[cfg(not(any(
            target_pointer_width = "16",
            target_pointer_width = "32",
            target_pointer_width = "64"
        )))]
        self.write(&bytes);
    }
    #[inline]
    fn finish(&self) -> u64 {
        let mut state = self.clone();
        state.block(state.tail | u64::from(state.length).wrapping_shl(56));
        state.v2 ^= 0xff;
        state.round();
        state.round();
        state.round();
        state.v0 ^ state.v1 ^ state.v2 ^ state.v3
    }
}
#[cfg(all(
    test,
    feature = "std",
    any(
        target_os = "linux",
        target_os = "android",
        all(target_os = "windows", not(target_vendor = "win7")),
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "solaris"
    )
))]
mod seed_tests {
    use super::*;
    use std::vec::Vec;

    fn keys_of(state: RandomState) -> (u64, u64) {
        (state.k0, state.k1)
    }
    #[test]
    fn refusal_and_unwind_leave_thread_uninitialized_then_advance() {
        let keys = ThreadKeys::new();
        assert_eq!(
            next_keys(&keys, || Err(Error::Entropy { code: 5 })),
            Err(Error::Entropy { code: 5 })
        );
        assert_eq!((keys.ready.get(), keys.keys.get()), (false, (0, 0)));
        let keys_ref = &keys;
        assert!(
            std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
                next_keys(keys_ref, || panic!("provider"))
            }))
            .is_err()
        );
        assert_eq!((keys.ready.get(), keys.keys.get()), (false, (0, 0)));
        assert_eq!(next_keys(&keys, || Ok((u64::MAX, 7))), Ok((u64::MAX, 7)));
        assert_eq!(
            next_keys(&keys, || panic!("cached provider called")),
            Ok((0, 7))
        );
        assert_eq!(
            next_keys(&keys, || panic!("cached provider called")),
            Ok((1, 7))
        );
        assert_eq!((keys.ready.get(), keys.keys.get()), (true, (2, 7)));
    }
    #[test]
    fn keys_are_per_thread_and_advance_like_std() {
        let (a0, a1) = keys_of(RandomState::try_new().unwrap());
        let (b0, b1) = keys_of(RandomState::try_new().unwrap());
        assert_eq!((b0, b1), (a0.wrapping_add(1), a1));
        let others = (0..4)
            .map(|_| std::thread::spawn(|| keys_of(RandomState::try_new().unwrap())))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<Vec<_>>();
        // Independent 64-bit OS keys collide with probability about 2^-64.
        for (i, (_, k1)) in others.iter().enumerate() {
            assert_ne!(*k1, a1);
            assert!(others.iter().skip(i + 1).all(|other| other.1 != *k1));
        }
        // Another thread's construction did not advance this thread's keys.
        assert_eq!(
            keys_of(RandomState::try_new().unwrap()),
            (a0.wrapping_add(2), a1)
        );
    }
}

impl<H> TryClone for core::hash::BuildHasherDefault<H> {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self::new())
    }
}
impl<H> TryDefault for core::hash::BuildHasherDefault<H> {
    #[inline]
    fn try_default() -> Result<Self, Error> {
        Ok(Self::new())
    }
}
#[cfg(feature = "std")]
impl TryClone for std::collections::hash_map::RandomState {
    #[inline]
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(self.clone())
    }
}
