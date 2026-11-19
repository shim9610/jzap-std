//! Executable search intervals and packed owned-cursor coordinates.
use crate::arithmetic::bounded::{wrapping_add as add, wrapping_sub as sub};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires low < high,
    ensures low <= out < high,
        out as int == low as int + (high as int-low as int)/2,
))]
pub(crate) fn middle(low: usize, high: usize) -> usize {
    add(low, sub(high, low) / 2)
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires low <= middle < high,
    ensures low <= out.0 <= out.1 <= high,
        less ==> out.1 == high, !less ==> out.0 == low,
        (out.1 as int)-(out.0 as int) < (high as int)-(low as int),
))]
pub(crate) fn advance(low: usize, high: usize, middle: usize, less: bool) -> (usize, usize) {
    if less {
        (add(middle, 1), high)
    } else {
        (low, middle)
    }
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires index<=11,height<64,
    ensures out<1024,(out&15) as usize==index,(out>>4) as usize==height,
))]
pub(crate) fn pack(index: usize, height: usize) -> u16 {
    let index = index as u16;
    let height = height as u16;
    let out = height.wrapping_shl(4) | index;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(out<1024 && out&15==index && out>>4==height) by(bit_vector)
            requires index<=11,height<64,out==height.wrapping_shl(4)|index;
    }
    out
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out==(position&15) as usize,))]
pub(crate) fn index(position: u16) -> usize {
    (position & 15) as usize
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out==(position>>4) as usize,))]
pub(crate) fn height(position: u16) -> usize {
    (position >> 4) as usize
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires position<1024,index<=11,
    ensures out<1024,out&15==index as u16,out>>4==position>>4,
))]
pub(crate) fn replace_index(position: u16, index: usize) -> u16 {
    let index = index as u16;
    let out = (position & !15) | index;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(out<1024 && out&15==index && out>>4==position>>4) by(bit_vector)
            requires position<1024,index<=11,out==(position&!15)|index;
    }
    out
}

/// Additional nodes needed beyond allocations retired by the merge frontier.
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == if maximum >= allocated.saturating_sub(retired) {
            maximum
        } else { allocated.saturating_sub(retired) },
        out >= maximum, out as int + retired as int >= allocated as int,
))]
pub(crate) fn allocation_deficit(maximum: usize, allocated: usize, retired: usize) -> usize {
    maximum.max(allocated.saturating_sub(retired))
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires count < usize::MAX,
    ensures out as int == count as int + 1,
))]
pub(crate) fn retired_successor(count: usize) -> usize {
    add(count, 1)
}

/// Skip only payloads covered by the exact remaining owning-iterator count.
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires index<count,remaining>0,
    ensures out<count-index,out<remaining,
        out == if count-index-1 <= remaining-1 {count-index-1} else {remaining-1},
))]
pub(crate) fn drop_skip(index: usize, count: usize, remaining: usize) -> usize {
    let suffix = sub(sub(count, index), 1);
    suffix.min(sub(remaining, 1))
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires index<=_count,if back {index>0} else {index<_count},
    ensures out.0<_count,out.1<=_count,
        out.0 as int == index as int-if back {1int} else {0int},
        out.1 as int == out.0 as int+if back {0int} else {1int},
))]
pub(crate) fn leaf_edge(index: usize, _count: usize, back: bool) -> (usize, usize) {
    let i = if back { sub(index, 1) } else { index };
    (i, if back { i } else { add(i, 1) })
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == (index>0),))]
pub(crate) fn consider_left(index: usize) -> bool {
    index > 0
}
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == (index==0 && index<parent),
))]
pub(crate) fn consider_right(index: usize, parent: usize) -> bool {
    index == 0 && index < parent
}
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires sibling<=11,child<=11,
    ensures out == (sibling as int+child as int+1>11),
))]
pub(crate) fn borrow_required(sibling: usize, child: usize) -> bool {
    add(add(sibling, child), 1) > super::node::CAP
}

/// Exact initialized-pair/edge spans used by the node ownership kernels.
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old<11,index<=old,
    ensures out.0 as int==index as int+1,
        out.1 as int==old as int-index as int,
        out.2 as int==old as int+1,
        out.3 as int==index as int+2,
        out.0<=11,out.2<=11,out.3<=12,
        index+out.1<=11,out.0+out.1<=11,out.3+out.1<=12,
))]
pub(crate) fn insert_span(old: usize, index: usize) -> (usize, usize, usize, usize) {
    let after = add(index, 1);
    (after, sub(old, index), add(old, 1), add(after, 1))
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old<=11,mid<old,
    ensures out.0 as int==mid as int+1,
        out.1 as int==old as int-mid as int-1,
        out.2 as int==out.1 as int+1,
        out.0+out.1<=11,out.0+out.2<=12,out.2<=12,
))]
pub(crate) fn split_span(old: usize, mid: usize) -> (usize, usize, usize) {
    let start = add(mid, 1);
    let count = sub(old, start);
    (start, count, add(count, 1))
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old<=11,index<=old,
    ensures out.0 as int==old as int-index as int,
        out.1 as int==index as int+1,
        index+out.0<=11,out.1+out.0<=12,1+out.0<=12,out.0<=11,
))]
pub(crate) fn suffix_span(old: usize, index: usize) -> (usize, usize) {
    (sub(old, index), add(index, 1))
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old<=11,index<old,
    ensures out.0 as int==index as int+1,
        out.1 as int==old as int-index as int-1,
        out.2 as int==old as int-1,
        out.0+out.1<=11,index+out.1<=11,
        out.0+1+out.1<=12,out.0+out.1<=12,out.2<11,
))]
pub(crate) fn remove_span(old: usize, index: usize) -> (usize, usize, usize) {
    let after = add(index, 1);
    (after, sub(old, after), sub(old, 1))
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires left<=11,right<=11,left as int+right as int+1<=11,
    ensures out.0 as int==left as int+1,
        out.1 as int==left as int+right as int+1,
        out.2 as int==right as int+1,
        left<11,out.0+right<=11,out.0+out.2<=12,out.1<=11,
))]
pub(crate) fn merge_span(left: usize, right: usize) -> (usize, usize, usize) {
    let start = add(left, 1);
    (start, add(start, right), add(right, 1))
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires 0<left<=11,right<11,
    ensures out.0 as int==left as int-1,
        out.1 as int==right as int+1,
        out.0<11,out.1<=11,right+1<=11,1+out.1<=12,out.0+1<12,
))]
pub(crate) fn steal_left_span(left: usize, right: usize) -> (usize, usize) {
    (sub(left, 1), add(right, 1))
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires left<11,0<right<=11,
    ensures out.0 as int==left as int+1,
        out.1 as int==right as int-1,
        out.0<=11,out.1<11,1+out.1<=11,1+right<=12,
))]
pub(crate) fn steal_right_span(left: usize, right: usize) -> (usize, usize) {
    (add(left, 1), sub(right, 1))
}
