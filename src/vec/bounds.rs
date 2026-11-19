//! Endpoint normalization used after each returning RangeBounds callback.
use crate::{Checked, Error};
use core::ops::Bound;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
verus! {
pub(crate) open spec fn lower_value(bound: Bound<&usize>) -> int {
    match bound { Bound::Included(n) => *n as int, Bound::Excluded(n) => *n as int + 1, Bound::Unbounded => 0 }
}
pub(crate) open spec fn upper_value(bound: Bound<&usize>, length: usize) -> int {
    match bound { Bound::Included(n) => *n as int + 1, Bound::Excluded(n) => *n as int, Bound::Unbounded => length as int }
}
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(value) => value as int == lower_value(bound),
        Err(Error::Bounds) => lower_value(bound) > usize::MAX,
        _ => false,
    },
))]
#[inline]
pub(crate) fn lower(bound: Bound<&usize>) -> Result<usize, Error> {
    let start = match bound {
        Bound::Included(&n) => n,
        Bound::Excluded(&n) => (Checked::new(n) + Checked::new(1))
            .map_err(
                #[cfg_attr(verus_keep_ghost, verus_spec(error: Error => ensures error == Error::Bounds,))]
                |_error| Error::Bounds,
            )?
            .get(),
        Bound::Unbounded => 0,
    };
    Ok(start)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(value) => value as int == upper_value(bound, length),
        Err(Error::Bounds) => upper_value(bound, length) > usize::MAX,
        _ => false,
    },
))]
#[inline]
pub(crate) fn upper(bound: Bound<&usize>, length: usize) -> Result<usize, Error> {
    let end = match bound {
        Bound::Included(&n) => (Checked::new(n) + Checked::new(1))
            .map_err(
                #[cfg_attr(verus_keep_ghost, verus_spec(error: Error => ensures error == Error::Bounds,))]
                |_error| Error::Bounds,
            )?
            .get(),
        Bound::Excluded(&n) => n,
        Bound::Unbounded => length,
    };
    Ok(end)
}
