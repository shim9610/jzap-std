//! Pure admission kernel shared by normal Rust and direct Verus checking.
use crate::Error;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(result =>
    ensures match result {
        Ok(next) => used <= limit && (bytes as int) <= (limit as int) - (used as int)
            && (next as int) == (used as int) + (bytes as int) && next <= limit,
        Err(Error::Capacity) => used > limit,
        Err(Error::Budget { requested, available }) => used <= limit
            && (bytes as int) > (limit as int) - (used as int)
            && requested == bytes && (available as int) == (limit as int) - (used as int),
        _ => false,
    },
))]
#[inline]
pub(crate) fn admit(limit: usize, used: usize, bytes: usize) -> Result<usize, Error> {
    let available = limit.checked_sub(used).ok_or(Error::Capacity)?;
    if bytes > available {
        return Err(Error::Budget {
            requested: bytes,
            available,
        });
    }
    used.checked_add(bytes).ok_or(Error::Capacity)
}
