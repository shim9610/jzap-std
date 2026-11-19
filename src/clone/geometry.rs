//! Executable admission for the nested-array bulk clone adapter.
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(count) => source == destination
            && count as int == source as int * width as int,
        Err(crate::Error::Bounds) => source != destination,
        Err(crate::Error::Capacity) => source == destination
            && source as int * width as int > usize::MAX,
        _ => false,
    },
))]
#[inline]
pub(crate) fn flatten_count(
    source: usize,
    destination: usize,
    width: usize,
) -> Result<usize, crate::Error> {
    if source != destination {
        return Err(crate::Error::Bounds);
    }
    let Some(count) = source.checked_mul(width) else {
        return Err(crate::Error::Capacity);
    };
    Ok(count)
}
