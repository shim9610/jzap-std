//! Pure metadata offsets, shared by the table and direct-source proofs.
use crate::arithmetic::bounded::{wrapping_add as add, wrapping_sub as sub};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires buckets >= 4, buckets <= (usize::MAX / 2) - 16,
        buckets & ((buckets - 1) as usize) == 0, index < buckets,
    ensures out >= 16, out < buckets + 16,
        out & ((buckets - 1) as usize) == index,
))]
#[inline(always)]
pub(crate) fn mirror(index: usize, buckets: usize) -> usize {
    let mask = sub(buckets, 1);
    // For a power-of-two table, this is -16 modulo buckets, including
    // small tables. The positive bias avoids arithmetic underflow.
    let bias = mask & !15usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(bias <= mask) by(bit_vector) requires bias == mask & !15usize;
    }
    let shifted = add(index, bias);
    let offset = shifted & mask;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(offset <= mask) by(bit_vector) requires offset == shifted & mask;
        assert(((offset as int + 16) as usize) & mask == index) by(bit_vector)
            requires buckets >= 4, buckets & mask == 0, index < buckets,
                mask == (buckets - 1) as usize, offset == shifted & mask,
                bias == mask & !15usize,
                shifted == ((index as int + bias as int) as usize);
    }
    add(offset, 16)
}
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires mask >= 3, mask < (usize::MAX / 2) - 16,
        pos <= mask, stride <= mask,
    ensures out.0 <= mask, out.1 == stride + 16,
))]
#[inline(always)]
pub(crate) fn probe_next(pos: usize, stride: usize, mask: usize) -> (usize, usize) {
    let stride = add(stride, 16);
    let sum = add(pos, stride);
    let next = sum & mask;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(next <= mask) by(bit_vector) requires next == sum & mask;
    }
    (next, stride)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires alignment > 0, alignment & ((alignment - 1) as usize) == 0,
    ensures match out {
        Ok((bytes, offset)) => offset as int >= (size as int) * (buckets as int)
            && bytes as int == offset as int + buckets as int + 16
            && bytes <= isize::MAX
            && offset & (((if alignment < 16 { 16 } else { alignment }) - 1) as usize) == 0
            && (size as int) * (buckets as int)
                + (if alignment < 16 { 16 } else { alignment as int }) - 1 <= usize::MAX
            && offset == (((size as int) * (buckets as int)
                + (if alignment < 16 { 16 } else { alignment as int }) - 1) as usize)
                & !(((if alignment < 16 { 16 } else { alignment }) - 1) as usize),
        Err(_) => true,
    },
))]
#[inline]
pub(crate) fn checked_sizes(
    size: usize,
    alignment: usize,
    buckets: usize,
) -> Result<(usize, usize), crate::Error> {
    use crate::Error;
    let align = if alignment < 16 { 16 } else { alignment };
    let mask = align.checked_sub(1).ok_or(Error::Layout)?;
    let data = buckets.checked_mul(size).ok_or(Error::Layout)?;
    let padded = data.checked_add(mask).ok_or(Error::Layout)?;
    let offset = padded & !mask;
    #[cfg(verus_keep_ghost)]
    proof! {
        if alignment < 16 {
            assert(align == 16);
            assert(align & ((align - 1) as usize) == 0) by(bit_vector) requires align == 16;
        }
        assert(data as int == (size as int) * (buckets as int)) by(nonlinear_arith)
            requires data as int == (buckets as int) * (size as int);
        assert(offset >= data && offset & mask == 0) by(bit_vector)
            requires align >= 16, align & ((align - 1) as usize) == 0,
                mask == (align - 1) as usize,
                padded as int == data as int + mask as int, offset == padded & !mask;
    }
    let prefix = offset.checked_add(buckets).ok_or(Error::Layout)?;
    let bytes = prefix.checked_add(16).ok_or(Error::Layout)?;
    if bytes > isize::MAX as usize {
        return Err(Error::Layout);
    }
    Ok((bytes, offset))
}
