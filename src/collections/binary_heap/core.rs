//! Hole ownership: one removed value, one empty slot, restored on every exit.
use super::geometry::{child_offset, children, parent, selected};
use crate::{
    arithmetic::bounded::wrapping_sub as sub,
    budget::AllocationPolicy,
    vec::{LengthPolicy, Vec},
};
use core::{mem::ManuallyDrop, ptr};

struct Hole<'a, T> {
    data: &'a mut [T],
    value: ManuallyDrop<T>,
    pos: usize,
}
impl<'a, T> Hole<'a, T> {
    /// Requires pos < data.len(). The hole exclusively owns the removed value.
    unsafe fn new(data: &'a mut [T], pos: usize) -> Self {
        // SAFETY: caller establishes an initialized in-bounds position.
        let value = ManuallyDrop::new(unsafe { ptr::read(data.as_ptr().add(pos)) });
        Self { data, value, pos }
    }
    /// Requires index < len and index != pos; move transfers the empty slot.
    unsafe fn move_to(&mut self, index: usize) {
        // SAFETY: distinct initialized source and missing destination in one allocation.
        let data = self.data.as_mut_ptr();
        unsafe { ptr::copy_nonoverlapping(data.add(index), data.add(self.pos), 1) };
        self.pos = index;
    }
    /// Requires index < len and index != pos.
    unsafe fn get(&self, index: usize) -> &T {
        // SAFETY: the only uninitialized slot is excluded by the caller.
        unsafe { &*self.data.as_ptr().add(index) }
    }
}
impl<T> Drop for Hole<'_, T> {
    fn drop(&mut self) {
        // SAFETY: exactly one missing slot, with exclusive ownership of value.
        unsafe { ptr::copy_nonoverlapping(&*self.value, self.data.as_mut_ptr().add(self.pos), 1) };
    }
}

/// Requires start <= pos < data.len(). Prefix [start,pos) has heap order.
#[inline]
pub(super) unsafe fn sift_up<T: Ord>(data: &mut [T], start: usize, pos: usize) {
    // SAFETY: caller provides an initialized in-bounds position.
    let mut hole = unsafe { Hole::new(data, pos) };
    while hole.pos > start {
        let p = parent(hole.pos);
        // SAFETY: p < pos < len; the parent is not the hole.
        if &*hole.value <= unsafe { hole.get(p) } {
            break;
        }
        // SAFETY: same parent bounds and distinctness.
        unsafe { hole.move_to(p) };
    }
}

/// Requires pos < end, and for non-ZSTs end <= isize::MAX (valid slice layout).
/// Leaf positions may produce an out-of-range integer sentinel, never a pointer.
#[inline(always)]
fn first_child<T>(pos: usize, end: usize) -> usize {
    if core::mem::size_of::<T>() == 0 {
        if pos >= end / 2 {
            end
        } else {
            children(pos, end).0
        }
    } else {
        // A non-ZST Rust slice has at most isize::MAX elements, hence
        // pos < end <= isize::MAX. This layout fact discharges child_offset.
        child_offset(pos)
    }
}
/// Requires pos < end <= data.len(). Each child subtree is already a heap.
#[inline]
pub(super) unsafe fn sift_down<T: Ord>(data: &mut [T], pos: usize, end: usize) {
    // SAFETY: caller provides an initialized in-bounds position.
    let mut hole = unsafe { Hole::new(data, pos) };
    let last = sub(end, 1);
    let mut child = first_child::<T>(pos, end);
    while child < last {
        let right = crate::arithmetic::bounded::wrapping_add(child, 1);
        // SAFETY: child < end-1 proves both children are initialized, in bounds,
        // and strictly below the hole. Bool addition enables branchless selection.
        let choose_right = unsafe { hole.get(child) <= hole.get(right) };
        child = selected(child, end, choose_right);
        // SAFETY: selected child remains below end and distinct from the hole.
        if &*hole.value >= unsafe { hole.get(child) } {
            return;
        }
        // SAFETY: transfer the initialized selected child into the missing slot.
        unsafe { hole.move_to(child) };
        child = first_child::<T>(hole.pos, end);
    }
    if child == last {
        // SAFETY: the sole left child is initialized, in bounds and not the hole.
        if &*hole.value < unsafe { hole.get(child) } {
            // SAFETY: same distinct in-bounds child.
            unsafe { hole.move_to(child) };
        }
    }
}
/// Requires pos < end <= data.len(). Descend first, then restore upward.
#[inline]
pub(super) unsafe fn sift_bottom<T: Ord>(data: &mut [T], pos: usize, end: usize) {
    let start = pos;
    // SAFETY: caller establishes the initial bounds.
    let mut hole = unsafe { Hole::new(data, pos) };
    let last = sub(end, 1);
    let mut child = first_child::<T>(pos, end);
    while child < last {
        let right = crate::arithmetic::bounded::wrapping_add(child, 1);
        // SAFETY: paired-child frontier excludes the missing slot and proves bounds.
        child = selected(child, end, unsafe { hole.get(child) <= hole.get(right) });
        // SAFETY: selected initialized child is in bounds and distinct.
        unsafe { hole.move_to(child) };
        child = first_child::<T>(hole.pos, end);
    }
    if child == last {
        // SAFETY: sole left child, with a distinct initialized source slot.
        unsafe { hole.move_to(child) };
    }
    while hole.pos > start {
        let p = parent(hole.pos);
        // SAFETY: parent is below the hole index and remains initialized.
        if &*hole.value <= unsafe { hole.get(p) } {
            break;
        }
        // SAFETY: bounded distinct parent.
        unsafe { hole.move_to(p) };
    }
}

pub(super) fn rebuild<T: Ord>(data: &mut [T]) {
    let end = data.len();
    let mut pos = end / 2;
    while pos > 0 {
        pos = sub(pos, 1);
        // SAFETY: reverse interior-node traversal, children already repaired.
        unsafe { sift_down(data, pos, end) };
    }
}
pub(super) fn rebuild_tail<T: Ord>(data: &mut [T], start: usize) {
    let len = data.len();
    if start >= len {
        return;
    }
    let tail = sub(len, start);
    // This cost estimate never controls memory bounds. Saturation remains total.
    let rebuild_all = if start < tail {
        true
    } else {
        let levels = if len <= 2048 {
            usize::BITS
                .saturating_sub(1)
                .saturating_sub(start.leading_zeros()) as usize
        } else {
            11
        };
        len.saturating_mul(2) < tail.saturating_mul(levels)
    };
    if rebuild_all {
        rebuild(data);
    } else {
        let mut pos = start;
        while pos < len {
            // SAFETY: old prefix and previous inserted items are already a heap.
            unsafe { sift_up(data, 0, pos) };
            pos = crate::arithmetic::bounded::wrapping_add(pos, 1);
        }
    }
}

/// Requires len <= data.capacity(); growing also requires initialized new slots.
#[inline(always)]
pub(super) unsafe fn set_len<T, P: AllocationPolicy, L: LengthPolicy>(
    data: &mut Vec<T, P, L>,
    len: usize,
) {
    // SAFETY: the heap protocol proves this unchanged-capacity request is accepted.
    // No fallible user input reaches this helper. Actual Rust kernels check acceptance.
    let _ = unsafe { data.try_set_len(len) };
}
/// Requires a nonempty valid vector. Removes its root and repairs its heap.
#[inline]
pub(super) unsafe fn pop_nonempty<T: Ord, P: AllocationPolicy, L: LengthPolicy>(
    data: &mut Vec<T, P, L>,
) -> T {
    let last = sub(data.len(), 1);
    // SAFETY: last is initialized; retire its ownership before comparison callbacks.
    let value = unsafe { data.as_ptr().add(last).read() };
    // SAFETY: shrinking below the original length and unchanged capacity.
    unsafe { set_len(data, last) };
    if last == 0 {
        return value;
    }
    // SAFETY: remaining root is initialized, uniquely owned, in bounds.
    let result = unsafe { data.as_mut_ptr().replace(value) };
    // SAFETY: nonempty remaining vector; child subtrees retain heap order.
    unsafe { sift_bottom(data.as_mut_slice(), 0, last) };
    result
}
