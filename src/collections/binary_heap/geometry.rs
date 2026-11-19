//! Machine-word heap geometry, also included unchanged by direct Verus.
use crate::arithmetic::bounded::{wrapping_add as add, wrapping_mul as mul, wrapping_sub as sub};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires pos > 0,
    ensures out < pos, out as int == (pos as int - 1) / 2,
))]
#[inline(always)]
pub(crate) fn parent(pos: usize) -> usize {
    sub(pos, 1) / 2
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires pos < end / 2,
    ensures pos < out.0, out.0 < end, out.1 <= end,
        out.0 as int == 2 * pos as int + 1,
        out.1 as int == 2 * pos as int + 2,
))]
#[inline(always)]
pub(crate) fn children(pos: usize, end: usize) -> (usize, usize) {
    // Establish the frontier before multiplication, including usize::MAX ZSTs.
    let _ = end;
    let left = add(mul(pos, 2), 1);
    (left, add(left, 1))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires left < end, choose_right ==> (left as int) + 1 < end,
    ensures out < end, out as int == left as int + if choose_right { 1int } else { 0int },
))]
#[inline(always)]
pub(crate) fn selected(left: usize, end: usize, choose_right: bool) -> usize {
    let _ = end;
    add(left, choose_right as usize)
}

/// A non-ZST valid slice bounds pos below isize::MAX; ZST callers use children.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires pos <= isize::MAX,
    ensures out as int == 2 * pos as int + 1, out > pos,
))]
#[inline(always)]
pub(crate) fn child_offset(pos: usize) -> usize {
    add(mul(pos, 2), 1)
}
