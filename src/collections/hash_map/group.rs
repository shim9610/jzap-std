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
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little",
        not(any(kani, miri))
    ))]
    {
        use core::arch::aarch64::{vceqq_u8, vdupq_n_u8, vld1q_u8};
        // SAFETY: the target has NEON and the caller admits all 16 bytes.
        neon_mask(unsafe { vceqq_u8(vld1q_u8(ptr), vdupq_n_u8(tag)) })
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", not(kani)),
        all(
            target_arch = "aarch64",
            target_feature = "neon",
            target_endian = "little",
            not(any(kani, miri))
        )
    )))]
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
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little",
        not(any(kani, miri))
    ))]
    {
        use core::arch::aarch64::{vcltzq_s8, vld1q_s8};
        // SAFETY: the target has NEON and the caller admits all 16 bytes.
        neon_mask(unsafe { vcltzq_s8(vld1q_s8(ptr.cast())) })
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", not(kani)),
        all(
            target_arch = "aarch64",
            target_feature = "neon",
            target_endian = "little",
            not(any(kani, miri))
        )
    )))]
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

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little",
    not(any(kani, miri))
))]
#[inline(always)]
fn neon_mask(lanes: core::arch::aarch64::uint8x16_t) -> u16 {
    use core::arch::aarch64::{
        vaddv_u8, vandq_u8, vcombine_u8, vcreate_u8, vget_high_u8, vget_low_u8,
    };
    // Comparisons produce 0x00 or 0xff. Give each lane a distinct bit
    // within its byte, then sum each half independently. Each sum fits
    // in a byte. The lower half supplies the low byte of the mask.
    // SAFETY: NEON is enabled by the module's target configuration.
    unsafe {
        let weights = vcreate_u8(u64::from_le_bytes([1, 2, 4, 8, 16, 32, 64, 128]));
        let bits = vandq_u8(lanes, vcombine_u8(weights, weights));
        let low = vaddv_u8(vget_low_u8(bits));
        let high = vaddv_u8(vget_high_u8(bits));
        u16::from_le_bytes([low, high])
    }
}

/// # Safety
/// Sixteen initialized readable control bytes at ptr.
#[inline(always)]
pub(crate) unsafe fn any_empty(ptr: *const u8) -> bool {
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little",
        not(any(kani, miri))
    ))]
    {
        use core::arch::aarch64::{vld1q_u8, vmaxvq_u8};
        // EMPTY is the maximum possible control byte.
        // SAFETY: the target has NEON and the caller admits all 16 bytes.
        unsafe { vmaxvq_u8(vld1q_u8(ptr)) == EMPTY }
    }
    #[cfg(not(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little",
        not(any(kani, miri))
    )))]
    {
        // SAFETY: the caller admits the same group read.
        unsafe { matching(ptr, EMPTY) != 0 }
    }
}

/// # Safety
/// Sixteen initialized readable control bytes at ptr.
#[inline(always)]
pub(crate) unsafe fn first_vacant(ptr: *const u8) -> usize {
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little",
        not(any(kani, miri))
    ))]
    {
        use core::arch::aarch64::{vcltzq_s8, vld1q_s8};
        // SAFETY: the target has NEON and the caller admits all 16 bytes.
        let lanes = unsafe { vcltzq_s8(vld1q_s8(ptr.cast())) };
        neon_nibbles(lanes).trailing_zeros() as usize / 4
    }
    #[cfg(not(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little",
        not(any(kani, miri))
    )))]
    {
        // SAFETY: the caller admits the same group read.
        first(unsafe { vacant(ptr) })
    }
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little",
    not(any(kani, miri))
))]
#[inline(always)]
fn neon_nibbles(lanes: core::arch::aarch64::uint8x16_t) -> u64 {
    use core::arch::aarch64::{
        vget_lane_u64, vreinterpret_u64_u8, vreinterpretq_u16_u8, vshrn_n_u16,
    };
    // Each comparison byte is 0x00 or 0xff. Narrow each pair to one
    // byte, keeping four bits per lane in the original lane order.
    // A zero word still yields WIDTH (64 / 4).
    // SAFETY: NEON is enabled by the function's target configuration.
    unsafe {
        let packed = vshrn_n_u16::<4>(vreinterpretq_u16_u8(lanes));
        vget_lane_u64::<0>(vreinterpret_u64_u8(packed))
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

#[cfg(test)]
#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little",
    not(any(kani, miri))
))]
mod tests {
    use super::*;

    #[test]
    fn neon_matches_every_mask_on_unaligned_controls() {
        #[repr(align(16))]
        struct Controls([u8; WIDTH + 1]);

        let mut controls = Controls([0; WIDTH + 1]);
        for mask in 0..=u16::MAX {
            for (i, byte) in controls.0[1..].iter_mut().enumerate() {
                *byte = if mask & (1 << i) == 0 { 0x7f } else { DELETED };
            }
            let ptr = controls.0[1..].as_ptr();
            // SAFETY: ptr starts the 16-byte suffix.
            unsafe {
                assert_eq!(matching(ptr, DELETED), mask);
                assert_eq!(matching(ptr, 0x7f), !mask);
                assert_eq!(matching(ptr, EMPTY), 0);
                assert!(!any_empty(ptr));
                assert_eq!(first_vacant(ptr), first(mask));
                assert_eq!(vacant(ptr), mask);
            }
        }

        controls.0[1..].copy_from_slice(&[
            0x00, 0x01, 0x02, 0x03, 0x10, 0x80, 0x7e, 0xff, 0x80, 0x01, 0x80, 0xfe, 0x05, 0xff,
            0x05, 0xff,
        ]);
        let bytes = &controls.0[1..];
        // SAFETY: ptr is the start of the initialized 16-byte suffix.
        unsafe {
            assert!(any_empty(bytes.as_ptr()));
            assert_eq!(first_vacant(bytes.as_ptr()), 3);
        }
        for tag in 0..=u8::MAX {
            let expected = bytes
                .iter()
                .enumerate()
                .fold(0, |mask, (i, &byte)| mask | (u16::from(byte == tag) << i));
            // SAFETY: bytes is the 16-byte suffix.
            assert_eq!(unsafe { matching(bytes.as_ptr(), tag) }, expected);
        }
    }
}
