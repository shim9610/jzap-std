//! Transactional allocation planning, followed by allocation-free recycling merge.
use super::*;
use super::{add, sub};
use crate::budget::Accounting;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[derive(Default)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
struct NodeCounts {
    leaves: usize,
    branches: usize,
}
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl NodeCounts {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires if height==0 {old(self).leaves<usize::MAX} else {old(self).branches<usize::MAX},
        ensures final(self).leaves as int == old(self).leaves as int + if height==0 {1int} else {0int},
            final(self).branches as int == old(self).branches as int + if height==0 {0int} else {1int},
    ))]
    fn record_retired(&mut self, height: usize) {
        let count = if height == 0 {
            &mut self.leaves
        } else {
            &mut self.branches
        };
        *count = super::geometry::retired_successor(*count);
    }

    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures final(self).leaves == if old(self).leaves >= leaves.saturating_sub(retired.leaves) {
                old(self).leaves
            } else { leaves.saturating_sub(retired.leaves) },
            final(self).branches == if old(self).branches >= branches.saturating_sub(retired.branches) {
                old(self).branches
            } else { branches.saturating_sub(retired.branches) },
            final(self).leaves>=old(self).leaves,
            final(self).branches>=old(self).branches,
            final(self).leaves as int + retired.leaves as int >= leaves as int,
            final(self).branches as int + retired.branches as int >= branches as int,
    ))]
    fn observe(&mut self, leaves: usize, branches: usize, retired: &Self) {
        self.leaves = super::geometry::allocation_deficit(self.leaves, leaves, retired.leaves);
        self.branches =
            super::geometry::allocation_deficit(self.branches, branches, retired.branches);
    }
}
struct PlanningStream<'a, K, V> {
    state: iter::State<K, V>,
    marker: PhantomData<&'a (K, V)>,
}
impl<'a, K, V> PlanningStream<'a, K, V> {
    fn new<P: AllocationPolicy>(map: &'a BTreeMap<K, V, P>) -> Self {
        Self {
            state: iter::State::new(map),
            marker: PhantomData,
        }
    }
    fn next(&mut self, retired: &mut NodeCounts) -> Option<&'a K> {
        if self.state.remaining.get() == 0 {
            return None;
        }
        let c = self.state.front?;
        // SAFETY: the shared source borrow holds every node and key live. Use
        // the *same* frontier/retirement kernel as IntoIter, but count exhausted
        // allocations without moving pairs or freeing any source storage.
        unsafe {
            let key = &*node::key(c.node, c.index());
            self.state.advance_owned(c, false, |_, h| {
                retired.record_retired(h);
            });
            Some(key)
        }
    }
}

struct Pool<K, V, P: AllocationPolicy, const ALLOCATE: bool = false> {
    leaves: Spares<K, V, P>,
    branches: Spares<K, V, P>,
}
impl<K, V, P: AllocationPolicy, const ALLOCATE: bool> Pool<K, V, P, ALLOCATE> {
    fn take(&mut self, height: usize) -> Result<Link<K, V>, Error> {
        if ALLOCATE {
            return node::allocate(height, &self.leaves.policy);
        }
        if height == 0 {
            self.leaves.take()
        } else {
            self.branches.take()
        }
        .ok_or(Error::InvalidStructure)
    }
    /// # Safety
    /// Unique exhausted node allocated under the destination policy.
    unsafe fn retire(&mut self, n: Link<K, V>, h: usize) {
        // SAFETY: all initialized ownership was consumed before retirement.
        unsafe {
            if h == 0 {
                self.leaves.recycle(n, h)
            } else {
                self.branches.recycle(n, h)
            }
        };
    }
    fn next(&mut self, stream: &mut IntoIter<K, V, P>) -> Option<(K, V)> {
        stream.take_with(false, |n, h| {
            // SAFETY: stream transfers each exhausted allocation exactly once.
            unsafe { self.retire(n, h) };
        })
    }
}
fn same_account(a: Accounting<'_>, b: Accounting<'_>) -> bool {
    match (a, b) {
        (Accounting::Unmetered, Accounting::Unmetered) => true,
        (Accounting::Local(a), Accounting::Local(b)) => ptr::eq(a, b),
        #[cfg(target_has_atomic = "ptr")]
        (Accounting::Concurrent(a), Accounting::Concurrent(b)) => ptr::eq(a, b),
        _ => false,
    }
}
/// Upper bound for packed output height/node counts, without arithmetic overflow.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((leaves,branches,height)) => height<32 && height<usize::BITS
            && height<=branches && ((height==0)==(length<12))
            && (length==0 ==> leaves==0 && branches==0 && height==0)
            && (length>0 ==> leaves as int==length as int/12+1),
        Err(_) => false,
    },
))]
fn output_shape(length: usize) -> Result<(usize, usize, usize), Error> {
    if length == 0 {
        return Ok((0, 0, 0));
    }
    let leaves = length / 12;
    let leaves = leaves.checked_add(1).ok_or(Error::Capacity)?;
    let mut level = leaves;
    let mut branches = 0usize;
    let mut height = 0usize;
    #[cfg(verus_keep_ghost)]
    let mut bound = verus_exec_expr!(Ghost(0x2000_0000_0000_0000u64));
    #[cfg(verus_keep_ghost)]
    let mut steps = verus_exec_expr!(Ghost(0u64));
    #[cfg(verus_keep_ghost)]
    proof! {
        let b=bound@;
        let h=steps@;
        assert(b == 0x2000_0000_0000_0000u64 >> h.wrapping_mul(2)) by(bit_vector)
            requires b==0x2000_0000_0000_0000u64,h==0;
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant 1<=level<=leaves,
            leaves as int==length as int/12+1,
            2*(leaves as int)<=usize::MAX as int,
            branches as int+level as int<=2*(leaves as int),
            height<32,height<=branches,
            height==0 ==> level==leaves && branches==0,
            height>0 ==> leaves>1,
            steps@ as int==height as int,
            level as int-1<=bound@ as int,
            bound@ == 0x2000_0000_0000_0000u64 >> steps@.wrapping_mul(2),
        decreases level,
    ))]
    while level > 1 {
        #[cfg(verus_keep_ghost)]
        let previous = verus_exec_expr!(Ghost(level));
        level = add(level / 12, if level % 12 != 0 { 1 } else { 0 });
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(1<=level);
            assert(2*(level as int)<=previous@ as int);
            assert(4*(level as int-1)<=previous@ as int-1);
            let h=steps@;
            let b=bound@;
            if h==31 {
                assert(b==0) by(bit_vector)
                    requires h==31,b==0x2000_0000_0000_0000u64 >> h.wrapping_mul(2);
                assert(false);
            }
            assert(h<31);
            vstd::bits::lemma_u64_shr_is_div(b,2);
            reveal(vstd::arithmetic::power::pow);
            assert(vstd::arithmetic::power2::pow2(2)==4) by(compute_only);
            assert(level as int-1 <= (b>>2) as int);
            assert(b>>2 == 0x2000_0000_0000_0000u64 >> h.wrapping_add(1).wrapping_mul(2)) by(bit_vector)
                requires h<31,b==0x2000_0000_0000_0000u64 >> h.wrapping_mul(2);
            *bound=b>>2;
            *steps=(h+1) as u64;
        }
        branches = branches.checked_add(level).ok_or(Error::Capacity)?;
        height = height.checked_add(1).ok_or(Error::Capacity)?;
    }
    Ok((leaves, branches, height))
}
impl<K, V, P: AllocationPolicy> BTreeMap<K, V, P> {
    /// Only owned set algebra calls this sorted-source builder. Errors destroy
    /// the private complete prefix; an underfull right border is never exposed.
    #[inline]
    pub(in crate::collections) fn try_from_sorted_source(
        source: impl Iterator<Item = Result<(K, V), Error>>,
        policy: P,
    ) -> Result<Self, Error> {
        let mut result = Self::new_in(policy);
        let mut pool = Pool::<_, _, _, true> {
            leaves: Spares::new(result.policy.try_clone()?),
            branches: Spares::new(result.policy.try_clone()?),
        };
        let mut leaf = None;
        for pair in source {
            let pair = pair?;
            if result.len().checked_add(1).is_none() {
                return Err(Error::Capacity);
            }
            result
                .bulk_push(&mut leaf, pair, &mut pool)
                .map_err(|e| e.error)?;
        }
        // SAFETY: bulk_push publishes complete packed prefixes. Repair their
        // right border before the new destination can become observable.
        unsafe { result.fix_packed_border() };
        Ok(result)
    }
    fn pillar_from_pool<'p, const ALLOCATE: bool>(
        height: usize,
        policy: &'p P,
        pool: &mut Pool<K, V, P, ALLOCATE>,
    ) -> Result<BTreeMap<K, V, &'p P>, Error> {
        let mut map = BTreeMap::new_in(policy);
        map.root = Some(pool.take(0)?);
        while map.height.get() < height {
            let n = pool.take(1)?;
            if let Some(child) = map.root {
                // SAFETY: empty complete pillar owned by map, fresh empty parent.
                unsafe { node::set_child(n, 0, child) };
            }
            map.root = Some(n);
            map.height = Checked::new(add(map.height.get(), 1));
        }
        Ok(map)
    }
    /// # Safety
    /// Complete source subtree; checked total accounting is calculated before
    /// changing either map or reserving ownership in a different budget.
    unsafe fn allocated_bytes(n: Link<K, V>, h: usize) -> Result<usize, Error> {
        let mut bytes = node::layout::<K, V>(h).size();
        // SAFETY: initialized edges of the complete shared source tree.
        unsafe {
            if h > 0 {
                for i in 0..=node::len(n) {
                    bytes = bytes
                        .checked_add(Self::allocated_bytes(node::child(n, i), sub(h, 1))?)
                        .ok_or(Error::Capacity)?;
                }
            }
        }
        Ok(bytes)
    }
    #[inline(always)]
    fn bulk_push<const ALLOCATE: bool>(
        &mut self,
        leaf: &mut Option<Link<K, V>>,
        pair: (K, V),
        pool: &mut Pool<K, V, P, ALLOCATE>,
    ) -> Result<(), Rejected<(K, V)>> {
        let (k, v) = pair;
        let Some(mut n) = *leaf else {
            let n = match pool.take(0) {
                Ok(n) => n,
                Err(error) => {
                    return Err(Rejected {
                        value: (k, v),
                        error,
                    });
                }
            };
            // SAFETY: empty leaf now owns the first output pair.
            unsafe { node::insert(n, 0, k, v, None) };
            self.root = Some(n);
            self.len = Checked::new(1);
            *leaf = Some(n);
            return Ok(());
        };
        // SAFETY: output is a complete, possibly underfull-right-border tree;
        // every newly attached subtree is first constructed as a complete pillar.
        unsafe {
            if node::len(n) < CAP {
                node::insert(n, node::len(n), k, v, None);
            } else {
                let mut height = 0;
                while node::len(n) == CAP {
                    match node::parent(n) {
                        Some((parent, _)) => {
                            n = parent;
                            height = add(height, 1);
                        }
                        None => break,
                    }
                }
                let new_root = node::len(n) == CAP;
                let target_height = if new_root { add(height, 1) } else { height };
                let mut right =
                    match Self::pillar_from_pool(sub(target_height, 1), &self.policy, pool) {
                        Ok(r) => r,
                        Err(error) => {
                            return Err(Rejected {
                                value: (k, v),
                                error,
                            });
                        }
                    };
                let parent = if new_root {
                    let parent = match pool.take(1) {
                        Ok(p) => p,
                        Err(error) => {
                            return Err(Rejected {
                                value: (k, v),
                                error,
                            });
                        }
                    };
                    node::set_child(parent, 0, n);
                    self.root = Some(parent);
                    self.height = Checked::new(target_height);
                    parent
                } else {
                    n
                };
                let child = right.root.take();
                let mut last = child;
                let mut h = right.height.get();
                while h > 0 {
                    if let Some(n) = last {
                        last = Some(node::child(n, 0));
                    }
                    h = sub(h, 1);
                }
                node::insert(parent, node::len(parent), k, v, child);
                *leaf = last;
            }
            self.len = Checked::new(add(self.len(), 1));
        }
        Ok(())
    }
    /// # Safety
    /// Packed bulk builder output: all non-border siblings are full.
    unsafe fn fix_packed_border(&mut self) {
        // SAFETY: a full left sibling supplies up to MIN keys without becoming
        // underfull. No merge or allocation is required for packed construction.
        unsafe {
            let Some(mut n) = self.root else { return };
            let mut h = self.height.get();
            while h > 0 {
                let i = sub(node::len(n), 1);
                let right = node::child(n, add(i, 1));
                while node::len(right) < MIN {
                    node::steal_left(n, i, sub(h, 1));
                }
                n = right;
                h = sub(h, 1);
            }
        }
    }
}
impl<K: Ord, V, P: AllocationPolicy> BTreeMap<K, V, P> {
    fn append_plan(&self, other: &Self, length: usize) -> Result<NodeCounts, Error> {
        let (mut left, mut right) = (PlanningStream::new(self), PlanningStream::new(other));
        let mut retired = NodeCounts::default();
        let (mut a, mut b) = (left.next(&mut retired), right.next(&mut retired));
        let mut output: usize = 0;
        let mut needed = NodeCounts::default();
        loop {
            match (a, b) {
                (None, None) => return Ok(needed),
                (Some(_), None) => a = left.next(&mut retired),
                (None, Some(_)) => b = right.next(&mut retired),
                (Some(x), Some(y)) => match x.cmp(y) {
                    Ordering::Less => a = left.next(&mut retired),
                    Ordering::Greater => b = right.next(&mut retired),
                    Ordering::Equal => {
                        a = left.next(&mut retired);
                        b = right.next(&mut retired);
                    }
                },
            }
            output = output.checked_add(1).ok_or(Error::Capacity)?;
            if output > length {
                return Err(Error::InvalidStructure);
            }
            if output == 1 || output.is_multiple_of(12) {
                let (leaves, branches, _) = output_shape(output)?;
                needed.observe(leaves, branches, &retired);
            }
        }
    }
    /// Move the ordered union into self, retaining self's key and other's value
    /// for duplicates. All allocations/accounting admissions precede mutation.
    /// Removed input nodes are recycled by exact allocation kind.
    pub fn try_append(&mut self, other: &mut Self) -> Result<(), Error> {
        if other.is_empty() {
            return Ok(());
        }
        let length = self.len().checked_add(other.len()).ok_or(Error::Capacity)?;
        let transfer = !same_account(self.policy.budget(), other.policy.budget());
        let bytes = if transfer {
            match other.root {
                // SAFETY: other owns its complete valid tree.
                Some(n) => unsafe { Self::allocated_bytes(n, other.height.get()) }?,
                None => 0,
            }
        } else {
            0
        };
        if self.is_empty() {
            {
                let reservation = if transfer {
                    self.policy.budget().reserve(bytes)?
                } else {
                    None
                };
                if let Some(r) = reservation {
                    r.commit().retain();
                }
            }
            if transfer {
                other.policy.budget().release(bytes);
            }
            self.clear();
            self.root = other.root.take();
            self.height = other.height;
            self.len = other.len;
            other.height = Checked::new(0);
            other.len = Checked::new(0);
            return Ok(());
        }
        let (leaves, branches, height) = output_shape(length)?;
        let branch_credit = self
            .height
            .get()
            .checked_add(other.height.get())
            .and_then(|h| h.checked_add(height))
            .and_then(|h| h.checked_add(2))
            .ok_or(Error::Capacity)?;
        let upper = NodeCounts {
            leaves: leaves.min(2),
            branches: branches.min(branch_credit),
        };
        let leaf_bytes = node::layout::<K, V>(0).size();
        let extra = leaf_bytes.checked_mul(upper.leaves).and_then(|leaf| {
            node::layout::<K, V>(1)
                .size()
                .checked_mul(upper.branches)
                .and_then(|branch| leaf.checked_add(branch))
        });
        // At least ceil(length/11) nodes are already owned. When the conservative
        // spare bound is <=10% of a lower bound on input storage, keep the O(1)
        // credit plan. Otherwise simulate the merge to avoid transient overbooking.
        let amortized = extra
            .and_then(|n| n.checked_mul(10))
            .zip((length / 11).checked_mul(leaf_bytes))
            .is_some_and(|(extra, base)| extra <= base);
        let needed = if amortized {
            upper
        } else {
            self.append_plan(other, length)?
        };
        let (mut pool, destination, right_policy, empty_other_policy) = {
            // Acquire every independent policy owner before mutating either tree.
            let destination = self.policy.try_clone()?;
            let right_policy = self.policy.try_clone()?;
            let empty_other_policy = other.policy.try_clone()?;
            let mut pool = Pool::<_, _, _, false> {
                leaves: Spares::new(self.policy.try_clone()?),
                branches: Spares::new(self.policy.try_clone()?),
            };
            let reservation = if transfer {
                self.policy.budget().reserve(bytes)?
            } else {
                None
            };
            for _ in 0..needed.leaves {
                pool.leaves.reserve(0)?;
            }
            for _ in 0..needed.branches {
                pool.branches.reserve(1)?;
            }
            // End the accounting borrow before moving either original policy.
            if let Some(r) = reservation {
                r.commit().retain();
            }
            (pool, destination, right_policy, empty_other_policy)
        };
        if transfer {
            other.policy.budget().release(bytes);
        }
        let left = mem::replace(self, Self::new_in(destination));
        let mut right = mem::replace(other, Self::new_in(empty_other_policy));
        right.policy = right_policy;
        let mut left = left.into_iter();
        let mut right = right.into_iter();
        let mut a = pool.next(&mut left);
        let mut b = pool.next(&mut right);
        let mut leaf = None;
        struct Repair<'a, K, V, P: AllocationPolicy>(&'a mut BTreeMap<K, V, P>);
        impl<K, V, P: AllocationPolicy> Drop for Repair<'_, K, V, P> {
            fn drop(&mut self) {
                // SAFETY: published bulk prefixes are complete packed trees. Repair
                // also runs if user Ord/Drop unwinds after a prefix was committed.
                unsafe { self.0.fix_packed_border() };
            }
        }
        let repair = Repair(self);
        loop {
            let pair = match (a.as_ref(), b.as_ref()) {
                (None, None) => break,
                (Some(_), None) => {
                    let p = a.take();
                    a = pool.next(&mut left);
                    p
                }
                (None, Some(_)) => {
                    let p = b.take();
                    b = pool.next(&mut right);
                    p
                }
                (Some((ak, _)), Some((bk, _))) => match ak.cmp(bk) {
                    Ordering::Less => {
                        let p = a.take();
                        a = pool.next(&mut left);
                        p
                    }
                    Ordering::Greater => {
                        let p = b.take();
                        b = pool.next(&mut right);
                        p
                    }
                    Ordering::Equal => {
                        let first = a.take();
                        let second = b.take();
                        a = pool.next(&mut left);
                        b = pool.next(&mut right);
                        match (first, second) {
                            (Some((k, old)), Some((discard, v))) => {
                                drop((discard, old));
                                Some((k, v))
                            }
                            _ => None,
                        }
                    }
                },
            };
            if let Some(pair) = pair {
                repair
                    .0
                    .bulk_push(&mut leaf, pair, &mut pool)
                    .map_err(|e| e.error)?;
            }
        }
        drop(repair);
        Ok(())
    }
}

#[cfg(kani)]
mod count_proofs {
    use super::*;
    #[kani::proof]
    fn observed_deficits_preserve_each_all_word_field_and_retirement_frame() {
        let old_leaves: usize = kani::any();
        let old_branches: usize = kani::any();
        let leaves: usize = kani::any();
        let branches: usize = kani::any();
        let retired_leaves: usize = kani::any();
        let retired_branches: usize = kani::any();
        let retired = NodeCounts {
            leaves: retired_leaves,
            branches: retired_branches,
        };
        let mut needed = NodeCounts {
            leaves: old_leaves,
            branches: old_branches,
        };
        needed.observe(leaves, branches, &retired);
        assert!(needed.leaves == old_leaves.max(leaves.saturating_sub(retired_leaves)));
        assert!(needed.branches == old_branches.max(branches.saturating_sub(retired_branches)));
        assert!(retired.leaves == retired_leaves && retired.branches == retired_branches);
    }
    #[kani::proof]
    fn retirement_increments_only_the_selected_all_word_counter() {
        let leaves: usize = kani::any();
        let branches: usize = kani::any();
        let height: usize = kani::any();
        kani::assume(if height == 0 {
            leaves < usize::MAX
        } else {
            branches < usize::MAX
        });
        let mut retired = NodeCounts { leaves, branches };
        retired.record_retired(height);
        assert!(retired.leaves == if height == 0 { leaves + 1 } else { leaves });
        assert!(retired.branches == if height == 0 { branches } else { branches + 1 });
    }
}

#[cfg(kani)]
mod shape_proofs {
    use super::*;
    #[kani::proof]
    #[kani::unwind(24)]
    fn packed_shape_covers_every_machine_word_length() {
        let length: usize = kani::any();
        let result = output_shape(length);
        assert!(result.is_ok());
        if let Ok((leaves, branches, height)) = result {
            assert!(height < usize::BITS as usize && height < 32 && height < 65536);
            assert!(height <= branches && (height == 0) == (length < 12));
            if length == 0 {
                assert!(leaves == 0 && branches == 0 && height == 0);
            } else {
                assert!(leaves == length / 12 + 1);
            }
        }
    }
}
