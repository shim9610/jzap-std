//! Cursor arithmetic called by UTF-8 owner edits and cleanup.
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires width as int + cursor as int <= usize::MAX,
    ensures out as int == cursor as int + width as int,
))]
pub(crate) fn advance(cursor: usize, width: usize) -> usize {
    cursor.saturating_add(width)
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires written <= read <= original,
    ensures out.0 as int == original as int - read as int,
        out.1 as int == written as int + original as int - read as int,
        out.1 <= original,
))]
pub(crate) fn restore(read: usize, written: usize, original: usize) -> (usize, usize) {
    let remaining = original.saturating_sub(read);
    let len = written.saturating_add(remaining);
    (remaining, len)
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires start <= end <= old,
    ensures out.0 as int == old as int - end as int,
        out.1 as int == start as int + old as int - end as int,
        out.1 <= old,
))]
pub(crate) fn drain(start: usize, end: usize, old: usize) -> (usize, usize) {
    let remaining = old.saturating_sub(end);
    let removed = end.saturating_sub(start);
    let length = old.saturating_sub(removed);
    (remaining, length)
}
