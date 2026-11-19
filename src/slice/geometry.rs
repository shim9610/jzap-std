//! Original checked endpoints shared by scratch and borrowed slice operations.
use crate::arithmetic::bounded::{wrapping_add as add, wrapping_sub as sub};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(end) => start <= capacity && end <= capacity
            && end as int == start as int + count as int,
        None => start > capacity || start as int + count as int > capacity as int,
    },
))]
#[inline]
pub(crate) fn admitted_end(start: usize, count: usize, capacity: usize) -> Option<usize> {
    if start > capacity {
        return None;
    }
    if count > sub(capacity, start) {
        return None;
    }
    Some(add(start, count))
}
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((a, b)) => a == start && b == end && a <= b && b <= length,
        Err(crate::Error::Bounds) => start > end || end > length,
        _ => false,
    },
))]
#[inline]
pub(crate) fn range(
    start: usize,
    end: usize,
    length: usize,
) -> Result<(usize, usize), crate::Error> {
    if start > end || end > length {
        return Err(crate::Error::Bounds);
    }
    Ok((start, end))
}
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old > 0,
    ensures out < old, out as int + 1 == old as int,
))]
#[inline]
pub(crate) fn retired_last(old: usize) -> usize {
    sub(old, 1)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires index < _limit,
    ensures out <= _limit, out as int == index as int + 1,
))]
#[inline]
pub(crate) fn initialized_next(index: usize, _limit: usize) -> usize {
    add(index, 1)
}
