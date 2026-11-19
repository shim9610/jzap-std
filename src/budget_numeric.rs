//! Numeric transitions executed by the local Cell ledger.
use crate::Error;
use crate::arithmetic::bounded::{wrapping_add, wrapping_sub};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_verify(dual_spec, open))]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires occupied as int + available as int <= limit,
    ensures out as int == limit as int - available as int - occupied as int,
))]
#[inline]
pub(crate) fn pending(limit: usize, occupied: usize, available: usize) -> usize {
    wrapping_sub(wrapping_sub(limit, available), occupied)
}

#[cfg_attr(verus_keep_ghost, verus_verify(dual_spec, open))]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(next) => bytes <= available && next as int == available as int - bytes as int,
        Err(Error::Budget { requested, available: left }) => requested == bytes
            && left == available && bytes > available,
        _ => false,
    },
))]
#[inline]
pub(crate) fn reserve(available: usize, bytes: usize) -> Result<usize, Error> {
    if bytes > available {
        Err(Error::Budget {
            requested: bytes,
            available,
        })
    } else {
        Ok(wrapping_sub(available, bytes))
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify(dual_spec, open))]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires current as int + bytes as int <= usize::MAX,
    ensures out as int == current as int + bytes as int,
))]
#[inline]
pub(crate) fn credit(current: usize, bytes: usize) -> usize {
    wrapping_add(current, bytes)
}

#[cfg_attr(verus_keep_ghost, verus_verify(dual_spec, open))]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires bytes <= current,
    ensures out as int == current as int - bytes as int,
))]
#[inline]
pub(crate) fn retire(current: usize, bytes: usize) -> usize {
    wrapping_sub(current, bytes)
}
