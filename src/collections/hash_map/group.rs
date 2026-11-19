//! A group load always lies inside the checked control allocation plus mirror.
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const WIDTH: usize = 16;
pub(crate) const EMPTY: u8 = 0xff;
pub(crate) const DELETED: u8 = 0x80;
/// # Safety
/// Sixteen initialized readable control bytes at ptr.
#[inline(always)]
pub(crate) unsafe fn matching(ptr: *const u8, tag: u8) -> u16 {
    #[cfg(all(target_arch = "x86_64", not(kani)))]
    {
        use core::arch::x86_64::*;
        // SAFETY: SSE2 is part of x86-64; caller guarantees the 16-byte read.
        unsafe {
            _mm_movemask_epi8(_mm_cmpeq_epi8(
                _mm_loadu_si128(ptr.cast()),
                _mm_set1_epi8(tag as i8),
            )) as u16
        }
    }
    #[cfg(not(all(target_arch = "x86_64", not(kani))))]
    {
        let mut result = 0u16;
        for i in 0..WIDTH {
            // SAFETY: i < WIDTH.
            if unsafe { ptr.add(i).read() } == tag {
                result |= 1u16.wrapping_shl(i as u32);
            }
        }
        result
    }
}
/// # Safety
/// Sixteen initialized readable control bytes at ptr.
#[inline(always)]
pub(crate) unsafe fn vacant(ptr: *const u8) -> u16 {
    #[cfg(all(target_arch = "x86_64", not(kani)))]
    {
        use core::arch::x86_64::*;
        // SAFETY: SSE2 and readable group inherited from caller.
        unsafe { _mm_movemask_epi8(_mm_loadu_si128(ptr.cast())) as u16 }
    }
    #[cfg(not(all(target_arch = "x86_64", not(kani))))]
    {
        let mut result = 0u16;
        for i in 0..WIDTH {
            // SAFETY: i < WIDTH.
            if unsafe { ptr.add(i).read() } & 0x80 != 0 {
                result |= 1u16.wrapping_shl(i as u32);
            }
        }
        result
    }
}
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out <= 16, bits != 0 ==> out < 16,
))]
#[inline(always)]
pub(crate) fn first(bits: u16) -> usize {
    bits.trailing_zeros() as usize
}
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out <= bits, bits != 0 ==> out < bits,
))]
#[inline(always)]
pub(crate) fn clear_first(bits: u16) -> u16 {
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(bits != 0 ==> bits & ((bits - 1) as u16) < bits) by(bit_vector);
        assert(bits & 0 == 0) by(bit_vector);
    }
    bits & bits.saturating_sub(1)
}
