//! Ring geometry included unchanged by the direct Verus entry point.
use crate::arithmetic::bounded::{wrapping_add as add, wrapping_sub as sub};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires cap > 0, head < cap, offset <= cap,
    ensures out < cap,
        (head as int) + (offset as int) == (out as int)
            || (head as int) + (offset as int) == (out as int) + (cap as int),
))]
#[inline(always)]
pub(crate) fn physical(head: usize, offset: usize, cap: usize) -> usize {
    let end = sub(cap, head);
    if offset >= end {
        sub(offset, end)
    } else {
        add(head, offset)
    }
}

/// Non-ZST array layouts bound capacity by isize::MAX, so this sum cannot wrap.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires cap > 0, cap <= isize::MAX, head < cap, offset <= cap,
    ensures out < cap,
        (head as int) + (offset as int) == out as int
            || (head as int) + (offset as int) == out as int + cap as int,
))]
#[inline(always)]
pub(crate) fn allocated_physical(head: usize, offset: usize, cap: usize) -> usize {
    let sum = add(head, offset);
    if sum >= cap { sub(sum, cap) } else { sum }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires len > 0, len <= cap,
    ensures out < cap, out as int == len as int - 1,
))]
#[inline(always)]
pub(crate) fn after_pop(len: usize, cap: usize) -> usize {
    let _ = cap;
    sub(len, 1)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires cap > 0, head < cap, offset <= cap,
    ensures out < cap,
        (head as int) - (offset as int) == out as int
            || (head as int) - (offset as int) + (cap as int) == out as int,
))]
#[inline(always)]
pub(crate) fn retreat(head: usize, offset: usize, cap: usize) -> usize {
    if offset <= head {
        sub(head, offset)
    } else {
        sub(cap, sub(offset, head))
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires len <= cap, head <= cap, cap == 0 ==> head == 0, cap > 0 ==> head < cap,
    ensures out.0 <= cap - head, out.1 <= head,
        (out.0 as int) + (out.1 as int) == len as int,
        len <= cap - head ==> out.0 == len && out.1 == 0,
        len > cap - head ==> out.0 == cap - head && out.1 == len - (cap - head),
))]
#[inline]
pub(crate) fn split(head: usize, len: usize, cap: usize) -> (usize, usize) {
    let end = sub(cap, head);
    let first = if len < end { len } else { end };
    (first, sub(len, first))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires len <= old, head < old, old < new,
    ensures out < new,
        len > old - head && old - head <= len - (old - head)
            ==> out == new - (old - head),
        len > old - head && out == head
            ==> old as int + (len as int - (old as int - head as int)) <= new as int,
        len > old - head && out != head ==> out == new - (old - head),
))]
#[inline]
pub(crate) fn grown_head(head: usize, len: usize, old: usize, new: usize) -> usize {
    let high = sub(old, head);
    if len <= high {
        return head;
    }
    let low = sub(len, high);
    if high > low && sub(new, old) >= low {
        head
    } else {
        sub(new, high)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires cap > 0, src < cap, dst < cap, count <= cap,
    ensures out.0 <= count, out.0 <= cap - src, out.0 <= cap - dst,
        count > 0 ==> out.0 > 0 && out.3 < count,
        out.1 < cap, out.2 < cap, out.3 as int == count as int - out.0 as int,
))]
#[inline(always)]
pub(crate) fn forward_chunk(
    src: usize,
    dst: usize,
    count: usize,
    cap: usize,
) -> (usize, usize, usize, usize) {
    let a = sub(cap, src);
    let b = sub(cap, dst);
    let c = if count < a { count } else { a };
    let n = if c < b { c } else { b };
    (
        n,
        physical(src, n, cap),
        physical(dst, n, cap),
        sub(count, n),
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires cap > 0, src <= cap, dst <= cap, count > 0, count <= cap,
    ensures out.0 > 0, out.0 <= count,
        out.1 < cap, out.2 < cap,
        out.1 as int + out.0 as int <= cap as int,
        out.2 as int + out.0 as int <= cap as int,
        out.3 as int == count as int - out.0 as int, out.3 < count,
))]
#[inline(always)]
pub(crate) fn backward_chunk(
    src: usize,
    dst: usize,
    count: usize,
    cap: usize,
) -> (usize, usize, usize, usize) {
    let src = if src == 0 { cap } else { src };
    let dst = if dst == 0 { cap } else { dst };
    let a = if count < src { count } else { src };
    let n = if a < dst { a } else { dst };
    (n, sub(src, n), sub(dst, n), sub(count, n))
}

/// The branch also supplies the universally checked rotation bounds.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires len <= cap, head <= cap, cap == 0 ==> head == 0, cap > 0 ==> head < cap,
    ensures out <= 4,
        (out == 0) == (len <= cap - head),
        out > 0 ==> cap > 0 && cap - head > 0 && cap - head < len,
        out == 3 ==> len - (cap - head) < len,
        out == 4 ==> cap - head < len,
        out == 1 ==> (cap - head) as int + len as int <= cap as int,
        out == 2 ==> len as int + (len - (cap - head)) as int <= cap as int,
))]
#[inline(always)]
pub(crate) fn contiguous_kind(head: usize, len: usize, cap: usize) -> u8 {
    let (a, b) = split(head, len, cap);
    if b == 0 {
        return 0;
    }
    let free = sub(cap, len);
    if free >= a {
        1
    } else if free >= b {
        2
    } else if a > b {
        3
    } else {
        4
    }
}
