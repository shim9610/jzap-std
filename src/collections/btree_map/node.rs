//! Private node ownership kernels. No user callbacks or allocation during moves.
use crate::{
    Error,
    arithmetic::bounded::{wrapping_add as add, wrapping_sub as sub},
    budget::AllocationPolicy,
    storage,
};
use core::{
    alloc::Layout,
    mem::MaybeUninit,
    ptr::{self, NonNull, addr_of_mut},
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const CAP: usize = 11;
pub(super) const MIN: usize = 5;
pub(super) type Link<K, V> = NonNull<Leaf<K, V>>;

/// Only node transitions below write this closed 0..=11 count.
#[repr(transparent)]
pub(super) struct Count(u16);
#[repr(C)]
pub(super) struct Leaf<K, V> {
    pub(super) parent: Option<Link<K, V>>,
    pub(super) parent_idx: u16,
    len: Count,
    keys: [MaybeUninit<K>; CAP],
    vals: [MaybeUninit<V>; CAP],
}
#[repr(C)]
struct Branch<K, V> {
    leaf: Leaf<K, V>,
    edges: [MaybeUninit<Link<K, V>>; 12],
}

#[inline]
pub(super) fn layout<K, V>(height: usize) -> Layout {
    if height == 0 {
        Layout::new::<Leaf<K, V>>()
    } else {
        Layout::new::<Branch<K, V>>()
    }
}
pub(super) fn allocate<K, V, P: AllocationPolicy>(
    height: usize,
    policy: &P,
) -> Result<Link<K, V>, Error> {
    let node = storage::allocate(layout::<K, V>(height), policy.budget())?.cast::<Leaf<K, V>>();
    // SAFETY: fresh, aligned node storage; arrays intentionally remain uninitialized.
    unsafe {
        addr_of_mut!((*node.as_ptr()).parent).write(None);
        addr_of_mut!((*node.as_ptr()).parent_idx).write(0);
        addr_of_mut!((*node.as_ptr()).len).write(Count(0));
    }
    Ok(node)
}
/// # Safety
/// Unique empty allocation of exactly this kind and policy; no live descendants.
pub(super) unsafe fn free<K, V, P: AllocationPolicy>(node: Link<K, V>, height: usize, policy: &P) {
    // SAFETY: inherited exact allocation ownership.
    unsafe { storage::deallocate(node.cast(), layout::<K, V>(height), policy.budget()) };
}
/// # Safety
/// Node is live and its header initialized. Does not borrow payload slots.
#[inline]
pub(super) unsafe fn len<K, V>(node: Link<K, V>) -> usize {
    // SAFETY: header field access only, valid initialized closed count.
    unsafe { usize::from((*node.as_ptr()).len.0) }
}
/// # Safety
/// Caller has transferred exactly n initialized pairs (0 <= n <= CAP) to node.
#[inline]
pub(super) unsafe fn set_len<K, V>(node: Link<K, V>, n: usize) {
    // SAFETY: exclusive header access; the closed bound makes narrowing exact.
    unsafe { (*node.as_ptr()).len = Count(n as u16) };
}
/// # Safety
/// Live allocation; i <= CAP. Dereferencing requires i initialized or writable.
#[inline]
pub(super) unsafe fn key<K, V>(node: Link<K, V>, i: usize) -> *mut K {
    // SAFETY: address only; creates no reference to a containing node or array.
    unsafe { addr_of_mut!((*node.as_ptr()).keys).cast::<K>().add(i) }
}
/// # Safety
/// Same slot bound and initialization contract as key.
#[inline]
pub(super) unsafe fn val<K, V>(node: Link<K, V>, i: usize) -> *mut V {
    // SAFETY: address only, within the node's value array (or one-past).
    unsafe { addr_of_mut!((*node.as_ptr()).vals).cast::<V>().add(i) }
}
/// # Safety
/// Exclusively owned live node; i < len, with an initialized key in that slot.
/// The replacement is equal in ordering; no node metadata or other slot changes.
#[inline]
pub(super) unsafe fn replace_key<K, V>(node: Link<K, V>, i: usize, replacement: K) -> K {
    // SAFETY: transfer the old initialized key out and publish the new key once.
    unsafe { ptr::replace(key(node, i), replacement) }
}
/// # Safety
/// Live internal allocation; i < 12, slot writable or initialized as appropriate.
#[inline]
unsafe fn edge<K, V>(node: Link<K, V>, i: usize) -> *mut Link<K, V> {
    // SAFETY: internal prefix layout and bounded child slot.
    unsafe {
        addr_of_mut!((*node.cast::<Branch<K, V>>().as_ptr()).edges)
            .cast::<Link<K, V>>()
            .add(i)
    }
}
/// # Safety
/// Internal live node; i <= len and that child is initialized and live.
#[inline]
pub(super) unsafe fn child<K, V>(node: Link<K, V>, i: usize) -> Link<K, V> {
    // SAFETY: inherited initialized edge bound.
    unsafe { edge(node, i).read() }
}
/// # Safety
/// Internal node, i < 12; exclusive ownership of live child transferred to edge.
#[inline]
pub(super) unsafe fn set_child<K, V>(node: Link<K, V>, i: usize, child: Link<K, V>) {
    // SAFETY: exclusive edge/header writes; no payload references are created.
    unsafe {
        edge(node, i).write(child);
        (*child.as_ptr()).parent = Some(node);
        (*child.as_ptr()).parent_idx = i as u16;
    }
}
/// # Safety
/// Live node and initialized header, parent (if any) is live.
#[inline]
pub(super) unsafe fn parent<K, V>(node: Link<K, V>) -> Option<(Link<K, V>, usize)> {
    // SAFETY: header reads only.
    unsafe {
        (*node.as_ptr())
            .parent
            .map(|p| (p, usize::from((*node.as_ptr()).parent_idx)))
    }
}
/// # Safety
/// Ranges are within both arrays; initialized pair ownership is moved, not cloned.
unsafe fn pairs<K, V>(src: Link<K, V>, from: usize, dst: Link<K, V>, to: usize, n: usize) {
    // SAFETY: overlap is allowed; source ownership is unpublished by the caller.
    unsafe {
        ptr::copy(key(src, from), key(dst, to), n);
        ptr::copy(val(src, from), val(dst, to), n);
    }
}
/// # Safety
/// Internal live nodes and bounded initialized source/writable destination ranges.
unsafe fn edges<K, V>(src: Link<K, V>, from: usize, dst: Link<K, V>, to: usize, n: usize) {
    // SAFETY: bounded, potentially overlapping child moves.
    unsafe { ptr::copy(edge(src, from), edge(dst, to), n) };
}
/// # Safety
/// Internal node with all edges from..=to initialized; 0 <= from <= to <= CAP.
unsafe fn fix<K, V>(node: Link<K, V>, from: usize, to: usize) {
    let mut i = from;
    while i <= to {
        // SAFETY: loop visits only initialized child slots.
        unsafe { set_child(node, i, child(node, i)) };
        i = add(i, 1);
    }
}
// Query derived fields at their use sites so counts do not stay live across copies.
/// # Safety
/// node has n<CAP pairs, i<=n, unique storage. right is present iff internal.
pub(super) unsafe fn insert<K, V>(
    node: Link<K, V>,
    i: usize,
    k: K,
    v: V,
    right: Option<Link<K, V>>,
) {
    // SAFETY: callers establish capacity and transfer all moved slots atomically.
    unsafe {
        let n = len(node);
        let (after, tail, _, _) = super::geometry::insert_span(n, i);
        // Appending at the end moves nothing; skip the zero-length copies as
        // std's slice_insert does instead of calling memmove twice.
        if tail != 0 {
            pairs(node, i, node, after, tail);
        }
        key(node, i).write(k);
        val(node, i).write(v);
        if let Some(right) = right {
            if tail != 0 {
                edges(
                    node,
                    after,
                    node,
                    super::geometry::insert_span(n, i).3,
                    tail,
                );
            }
            edge(node, after).write(right);
            fix(node, after, super::geometry::insert_span(n, i).2);
        }
        set_len(node, super::geometry::insert_span(n, i).2);
    }
}
/// std's symmetric degree-six split choices; edge is in 0..=11.
#[inline]
pub(super) fn splitpoint(edge: usize) -> usize {
    if edge < 5 {
        4
    } else if edge <= 6 {
        5
    } else {
        6
    }
}
/// # Safety
/// node full, mid<11, right empty allocation of the same kind. No callbacks.
pub(super) unsafe fn split<K, V>(
    node: Link<K, V>,
    mid: usize,
    right: Link<K, V>,
    height: usize,
) -> (K, V) {
    // SAFETY: disjoint allocated nodes and valid split partition.
    unsafe {
        let (start, n, edge_count) = super::geometry::split_span(len(node), mid);
        pairs(node, start, right, 0, n);
        if height > 0 {
            edges(node, start, right, 0, edge_count);
            fix(right, 0, n);
        }
        set_len(right, n);
        set_len(node, mid);
        (key(node, mid).read(), val(node, mid).read())
    }
}
/// # Safety
/// Unique nodes, index<=left.len, right is an empty matching pillar node. Its
/// edge zero (if internal) is initialized and retained as the boundary child.
pub(super) unsafe fn move_suffix<K, V>(
    left: Link<K, V>,
    index: usize,
    right: Link<K, V>,
    height: usize,
) {
    // SAFETY: disjoint suffix transfer, with no separator removed or duplicated.
    unsafe {
        let (n, edge_start) = super::geometry::suffix_span(len(left), index);
        pairs(left, index, right, 0, n);
        if height > 0 {
            edges(left, edge_start, right, 1, n);
            fix(right, 1, n);
        }
        set_len(left, index);
        set_len(right, n);
    }
}
/// # Safety
/// Exclusive initialized leaf slot i<len. Returned pair becomes caller-owned.
pub(super) unsafe fn remove_leaf<K, V>(node: Link<K, V>, i: usize) -> (K, V) {
    // SAFETY: remove ownership before shifting the initialized suffix.
    unsafe {
        let n = len(node);
        let result = (key(node, i).read(), val(node, i).read());
        let (after, tail, _) = super::geometry::remove_span(n, i);
        pairs(node, after, node, i, tail);
        set_len(node, super::geometry::remove_span(n, i).2);
        result
    }
}
/// # Safety
/// Parent separator i has children with combined length+1<=CAP; right is freed.
pub(super) unsafe fn merge<K, V, P: AllocationPolicy>(
    parent: Link<K, V>,
    i: usize,
    height: usize,
    policy: &P,
) -> Link<K, V> {
    // SAFETY: disjoint sibling ownership, separator and suffix ranges are bounded.
    unsafe {
        let left = child(parent, i);
        let right = child(parent, add(i, 1));
        let l = len(left);
        let r = len(right);
        let start = super::geometry::merge_span(l, r).0;
        key(left, l).write(key(parent, i).read());
        val(left, l).write(val(parent, i).read());
        pairs(right, 0, left, start, r);
        if height > 0 {
            edges(right, 0, left, start, super::geometry::merge_span(l, r).2);
            fix(left, start, super::geometry::merge_span(l, r).1);
        }
        set_len(left, super::geometry::merge_span(l, r).1);
        let n = len(parent);
        let (after, tail, _) = super::geometry::remove_span(n, i);
        pairs(parent, after, parent, i, tail);
        edges(parent, add(after, 1), parent, after, tail);
        set_len(parent, super::geometry::remove_span(n, i).2);
        fix(parent, after, super::geometry::remove_span(n, i).2);
        free(right, height, policy);
        left
    }
}
/// # Safety
/// Adjacent children at separator i; left nonempty, right not full.
pub(super) unsafe fn steal_left<K, V>(parent: Link<K, V>, i: usize, height: usize) {
    // SAFETY: moves ownership of the left maximum through the separator.
    unsafe {
        let left = child(parent, i);
        let right = child(parent, add(i, 1));
        let left_count = len(left);
        let r = len(right);
        let l = super::geometry::steal_left_span(left_count, r).0;
        pairs(right, 0, right, 1, r);
        key(right, 0).write(key(parent, i).read());
        val(right, 0).write(val(parent, i).read());
        key(parent, i).write(key(left, l).read());
        val(parent, i).write(val(left, l).read());
        if height > 0 {
            edges(
                right,
                0,
                right,
                1,
                super::geometry::steal_left_span(left_count, r).1,
            );
            edge(right, 0).write(child(left, add(l, 1)));
            fix(right, 0, super::geometry::steal_left_span(left_count, r).1);
        }
        set_len(left, l);
        set_len(right, super::geometry::steal_left_span(left_count, r).1);
    }
}
/// # Safety
/// Adjacent children at separator i; right nonempty, left not full.
pub(super) unsafe fn steal_right<K, V>(parent: Link<K, V>, i: usize, height: usize) {
    // SAFETY: moves ownership of the right minimum through the separator.
    unsafe {
        let left = child(parent, i);
        let right = child(parent, add(i, 1));
        let l = len(left);
        let r = len(right);
        key(left, l).write(key(parent, i).read());
        val(left, l).write(val(parent, i).read());
        key(parent, i).write(key(right, 0).read());
        val(parent, i).write(val(right, 0).read());
        pairs(
            right,
            1,
            right,
            0,
            super::geometry::steal_right_span(l, r).1,
        );
        if height > 0 {
            set_child(
                left,
                super::geometry::steal_right_span(l, r).0,
                child(right, 0),
            );
            edges(right, 1, right, 0, r);
            fix(right, 0, super::geometry::steal_right_span(l, r).1);
        }
        set_len(left, super::geometry::steal_right_span(l, r).0);
        set_len(right, super::geometry::steal_right_span(l, r).1);
    }
}

/// Empty-node reservation list. Header fields encode next pointer and node kind
/// only while a node has no payload or children and is owned by this list.
pub(super) struct Spares<K, V, P: AllocationPolicy> {
    head: Option<Link<K, V>>,
    pub(super) policy: P,
}
impl<K, V, P: AllocationPolicy> Spares<K, V, P> {
    pub(super) fn new(policy: P) -> Self {
        Self { head: None, policy }
    }
    pub(super) fn reserve(&mut self, height: usize) -> Result<(), Error> {
        let node = allocate(height, &self.policy)?;
        // SAFETY: empty private allocation, no tree references.
        unsafe {
            (*node.as_ptr()).parent = self.head;
            (*node.as_ptr()).parent_idx = u16::from(height > 0);
        }
        self.head = Some(node);
        Ok(())
    }
    pub(super) fn take(&mut self) -> Option<Link<K, V>> {
        let node = self.head?;
        // SAFETY: head is owned and empty; unlink before returning ownership.
        unsafe {
            self.head = (*node.as_ptr()).parent;
            (*node.as_ptr()).parent = None;
            (*node.as_ptr()).parent_idx = 0;
        }
        Some(node)
    }
    /// # Safety
    /// Unique allocation of this policy; all previous payload/child ownership
    /// was consumed. The allocation kind must match height.
    pub(super) unsafe fn recycle(&mut self, node: Link<K, V>, height: usize) {
        // SAFETY: unpublished empty node; parent fields belong to this list now.
        unsafe {
            set_len(node, 0);
            (*node.as_ptr()).parent = self.head;
            (*node.as_ptr()).parent_idx = u16::from(height > 0);
        }
        self.head = Some(node);
    }
}
impl<K, V, P: AllocationPolicy> Drop for Spares<K, V, P> {
    fn drop(&mut self) {
        while let Some(node) = self.head {
            // SAFETY: each list node is a unique empty allocation, tagged exact kind.
            unsafe {
                let height = usize::from((*node.as_ptr()).parent_idx);
                self.head = (*node.as_ptr()).parent;
                free(node, height, &self.policy);
            }
        }
    }
}

/// Owns an initialized node prefix during clone or destruction. Progress is
/// recorded before user Drop, so unwinding resumes cleanup without double drops.
pub(super) struct NodeGuard<'p, K, V, P: AllocationPolicy> {
    pub(super) node: Link<K, V>,
    pub(super) height: usize,
    pub(super) pairs: usize,
    pub(super) children: usize,
    pub(super) policy: &'p P,
}
/// # Safety
/// The caller owns initialized pair i<CAP and keeps its allocation live until
/// both destructors finish. Progress in the enclosing owner is already recorded.
pub(super) unsafe fn drop_pair<K, V>(node: Link<K, V>, i: usize) {
    struct ValueDrop<V>(*mut V);
    impl<V> Drop for ValueDrop<V> {
        fn drop(&mut self) {
            // SAFETY: the enclosing pair/storage owner keeps this value alive.
            unsafe { ptr::drop_in_place(self.0) };
        }
    }
    // SAFETY: inherited initialized pair and live allocation; the value guard
    // also runs when key Drop unwinds, without moving either field to the stack.
    unsafe {
        let value = ValueDrop(val(node, i));
        ptr::drop_in_place(key(node, i));
        drop(value);
    }
}
impl<K, V, P: AllocationPolicy> NodeGuard<'_, K, V, P> {
    fn drain(&mut self) {
        while self.children > 0 {
            self.children = sub(self.children, 1);
            // SAFETY: this guard owns exactly this remaining initialized child prefix.
            unsafe {
                destroy(
                    child(self.node, self.children),
                    sub(self.height, 1),
                    self.policy,
                )
            };
        }
        while self.pairs > 0 {
            self.pairs = sub(self.pairs, 1);
            // SAFETY: progress is recorded before invoking either destructor.
            unsafe { drop_pair(self.node, self.pairs) };
        }
    }
}
impl<K, V, P: AllocationPolicy> Drop for NodeGuard<'_, K, V, P> {
    fn drop(&mut self) {
        struct Cleanup<'a, 'p, K, V, P: AllocationPolicy>(&'a mut NodeGuard<'p, K, V, P>);
        impl<K, V, P: AllocationPolicy> Drop for Cleanup<'_, '_, K, V, P> {
            fn drop(&mut self) {
                self.0.drain();
                // SAFETY: every pair and child has been destroyed; exact owned layout.
                unsafe { free(self.0.node, self.0.height, self.0.policy) };
            }
        }
        let guard = Cleanup(self);
        guard.0.drain();
        // Cleanup frees even when a single user Drop unwinds.
    }
}
/// # Safety
/// Uniquely owns the complete valid subtree, height bounded by usize::BITS.
pub(super) unsafe fn destroy<K, V, P: AllocationPolicy>(
    node: Link<K, V>,
    height: usize,
    policy: &P,
) {
    // SAFETY: valid node has len pairs and, iff internal, len+1 children.
    let n = unsafe { len(node) };
    drop(NodeGuard {
        node,
        height,
        pairs: n,
        children: if height == 0 { 0 } else { add(n, 1) },
        policy,
    });
}
