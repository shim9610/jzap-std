use super::*;
impl<T, P: AllocationPolicy, L: LengthPolicy> VecDeque<T, P, L> {
    /// Circular memmove. The caller chooses a direction that cannot overwrite unread input.
    /// Both starting positions and every copied chunk are within the same allocation.
    pub(super) unsafe fn copy_ring(
        &mut self,
        mut src: usize,
        mut dst: usize,
        mut count: usize,
        backward: bool,
    ) {
        if mem::size_of::<T>() == 0 || count == 0 || src == dst {
            return;
        }
        let cap = self.capacity();
        let p = self.ptr.as_ptr();
        if backward {
            src = geometry::physical(src, count, cap);
            dst = geometry::physical(dst, count, cap);
            while count != 0 {
                let (chunk, next_src, next_dst, left) =
                    geometry::backward_chunk(src, dst, count, cap);
                src = next_src;
                dst = next_dst;
                // SAFETY: chunk bounded by both segment endpoints, overlap allowed.
                unsafe { ptr::copy(p.add(src), p.add(dst), chunk) };
                count = left;
            }
        } else {
            while count != 0 {
                let (chunk, next_src, next_dst, left) =
                    geometry::forward_chunk(src, dst, count, cap);
                // SAFETY: chunk bounded by both contiguous segment suffixes, overlap allowed.
                unsafe { ptr::copy(p.add(src), p.add(dst), chunk) };
                count = left;
                src = next_src;
                dst = next_dst;
            }
        }
    }
    pub fn swap(&mut self, a: usize, b: usize) -> Result<(), Error> {
        if a >= self.len() || b >= self.len() {
            return Err(Error::Bounds);
        }
        // SAFETY: both initialized indices; ptr::swap supports identical pointers including ZSTs.
        unsafe { ptr::swap(self.slot(a), self.slot(b)) };
        Ok(())
    }
    pub fn try_insert(&mut self, index: usize, value: T) -> Result<(), Rejected<T>> {
        self.try_insert_mut(index, value).map(|_| ())
    }
    pub fn try_insert_mut(&mut self, index: usize, value: T) -> Result<&mut T, Rejected<T>> {
        if index > self.len() {
            return Err(Rejected {
                value,
                error: Error::Bounds,
            });
        }
        if let Err(error) = self.try_reserve(1) {
            return Err(Rejected { value, error });
        }
        let tail = sub(self.len(), index);
        // SAFETY: reservation creates one hole; shift the shorter adjacent initialized range toward
        // that hole, in the direction preserving unread elements. Ownership moves, never duplicates.
        unsafe {
            if index < tail {
                if mem::size_of::<T>() != 0 {
                    self.head = Checked::new(geometry::retreat(self.head.get(), 1, self.capacity()))
                }
                self.copy_ring(self.physical(1), self.physical(0), index, false);
            } else {
                self.copy_ring(
                    self.physical(index),
                    self.physical(add(index, 1)),
                    tail,
                    true,
                )
            }
            let p = self.slot(index);
            p.write(value);
            self.len = Checked::new(add(self.len(), 1));
            Ok(&mut *p)
        }
    }
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        let tail = sub(sub(self.len(), index), 1);
        // SAFETY: read the selected live value once, then move the shorter side into its hole.
        let value = unsafe { self.slot(index).read() };
        unsafe {
            if index < tail {
                self.copy_ring(self.physical(0), self.physical(1), index, true);
                if mem::size_of::<T>() != 0 {
                    self.head = Checked::new(geometry::allocated_physical(
                        self.head.get(),
                        1,
                        self.capacity(),
                    ))
                }
            } else {
                self.copy_ring(
                    self.physical(add(index, 1)),
                    self.physical(index),
                    tail,
                    false,
                )
            }
        }
        self.len = Checked::new(sub(self.len(), 1));
        Some(value)
    }
    pub fn swap_remove_front(&mut self, index: usize) -> Option<T> {
        self.swap(index, 0).ok()?;
        self.pop_front()
    }
    pub fn swap_remove_back(&mut self, index: usize) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        self.swap(index, sub(self.len(), 1)).ok()?;
        self.pop_back()
    }
    pub fn rotate_left(&mut self, n: usize) -> Result<(), Error> {
        if n > self.len() {
            return Err(Error::Bounds);
        }
        if n == 0 || n == self.len() || mem::size_of::<T>() == 0 {
            return Ok(());
        }
        let right = sub(self.len(), n);
        if n > right {
            return self.rotate_right(right);
        }
        // SAFETY: forward move reads each prefix element before any wrapped destination can
        // overwrite it; n <= len/2. A full ring only changes head.
        unsafe {
            if self.len() != self.capacity() {
                self.copy_ring(self.physical(0), self.physical(self.len()), n, false)
            }
        }
        self.head = Checked::new(geometry::allocated_physical(
            self.head.get(),
            n,
            self.capacity(),
        ));
        Ok(())
    }
    pub fn rotate_right(&mut self, n: usize) -> Result<(), Error> {
        if n > self.len() {
            return Err(Error::Bounds);
        }
        if n == 0 || n == self.len() || mem::size_of::<T>() == 0 {
            return Ok(());
        }
        let left = sub(self.len(), n);
        if n > left {
            return self.rotate_left(left);
        }
        let head = geometry::retreat(self.head.get(), n, self.capacity());
        // SAFETY: symmetric backward move into the preceding hole; n <= len/2.
        unsafe {
            if self.len() != self.capacity() {
                self.copy_ring(self.physical(left), head, n, true)
            }
        }
        self.head = Checked::new(head);
        Ok(())
    }
    /// Allocation refusal preserves both rings, policies and charges.
    pub fn try_append(&mut self, other: &mut Self) -> Result<(), Error> {
        self.try_reserve(other.len())?;
        let len = self.len();
        let more = other.len();
        let (a, b) = other.as_slices();
        // SAFETY: exclusive distinct owners, admitted destination spare, no callbacks after reserve.
        unsafe {
            self.write_slice(len, a);
            self.write_slice(add(len, a.len()), b);
        }
        self.len = Checked::new(add(len, more));
        other.len = Checked::new(0);
        Ok(())
    }
    /// Copy bits from a distinct initialized slice into admitted spare logical slots.
    pub(super) unsafe fn write_slice(&mut self, index: usize, source: &[T]) {
        if source.is_empty() || mem::size_of::<T>() == 0 {
            return;
        }
        let physical = self.physical(index);
        let first = source.len().min(sub(self.capacity(), physical));
        // SAFETY: source and destination allocations are disjoint; split fits validated spare range.
        unsafe {
            ptr::copy_nonoverlapping(source.as_ptr(), self.ptr.as_ptr().add(physical), first);
            ptr::copy_nonoverlapping(
                source.as_ptr().add(first),
                self.ptr.as_ptr(),
                sub(source.len(), first),
            );
        }
    }
    pub fn try_split_off(&mut self, at: usize) -> Result<Self, Error> {
        if at > self.len() {
            return Err(Error::Bounds);
        }
        let count = sub(self.len(), at);
        let mut result = Self::empty(self.policy.try_clone()?, self.limit);
        result.try_reserve_exact(count)?;
        let pos = if count == 0 { 0 } else { self.physical(at) };
        let first = count.min(sub(self.capacity(), pos));
        // SAFETY: two distinct owners, exact admitted destination; source live suffix transferred once.
        unsafe {
            ptr::copy_nonoverlapping(self.ptr.as_ptr().add(pos), result.ptr.as_ptr(), first);
            ptr::copy_nonoverlapping(
                self.ptr.as_ptr(),
                result.ptr.as_ptr().add(first),
                sub(count, first),
            );
        }
        result.len = Checked::new(count);
        self.len = Checked::new(at);
        Ok(result)
    }
    pub fn try_retain<E>(&mut self, mut f: impl FnMut(&T) -> Result<bool, E>) -> Result<(), E> {
        self.try_retain_mut(|v| f(v))
    }
    pub fn try_retain_mut<E>(
        &mut self,
        mut f: impl FnMut(&mut T) -> Result<bool, E>,
    ) -> Result<(), E> {
        struct Repair<'a, T, P: AllocationPolicy, L: LengthPolicy> {
            owner: &'a mut VecDeque<T, P, L>,
            state: crate::compaction::Compaction,
        }
        impl<T, P: AllocationPolicy, L: LengthPolicy> Drop for Repair<'_, T, P, L> {
            fn drop(&mut self) {
                if self.state.written() != self.state.read() && self.state.remaining() != 0 {
                    // SAFETY: compact unvisited suffix into prior holes, preserving live ownership and order.
                    unsafe {
                        self.owner.copy_ring(
                            self.owner.physical(self.state.read()),
                            self.owner.physical(self.state.written()),
                            self.state.remaining(),
                            false,
                        )
                    }
                }
                self.owner.len = Checked::new(self.state.restored_len());
            }
        }
        let state = crate::compaction::Compaction::full(self.len());
        let mut guard = Repair { owner: self, state };
        #[cfg_attr(all(kani, jzap_loop_contracts), kani::loop_invariant(guard.state.valid_for(guard.owner.len()) && guard.owner.len() <= guard.owner.capacity() && guard.owner.capacity() <= guard.owner.max_len() && guard.owner.max_len() == on_entry(guard.owner.max_len()) && !guard.owner.probe_ptr().is_null()))]
        while let Some(read) = guard.state.peek() {
            // SAFETY: compaction read is live; exclusive callback borrow ends before movement.
            let p = unsafe { guard.owner.slot(read) };
            if f(unsafe { &mut *p })? {
                let write = guard.state.written();
                guard.state.keep_live();
                if read != write {
                    // SAFETY: earlier rejection creates a disjoint logical hole, no live alias remains.
                    unsafe { ptr::copy(p, guard.owner.slot(write), 1) }
                }
            } else {
                guard.state.remove_live();
                // SAFETY: retire from compaction before invoking its destructor.
                unsafe { ptr::drop_in_place(p) }
            }
        }
        Ok(())
    }
    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        self.iter().any(|v| v == value)
    }
    pub fn binary_search(&self, value: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.binary_search_by(|v| v.cmp(value))
    }
    pub fn binary_search_by_key<B: Ord>(
        &self,
        key: &B,
        mut f: impl FnMut(&T) -> B,
    ) -> Result<usize, usize> {
        self.binary_search_by(|v| f(v).cmp(key))
    }
    pub fn binary_search_by(
        &self,
        mut f: impl FnMut(&T) -> core::cmp::Ordering,
    ) -> Result<usize, usize> {
        let mut left = 0;
        let mut right = self.len();
        while left < right {
            let mid = add(left, sub(right, left) / 2);
            // SAFETY: midpoint belongs to the bounded live search interval.
            match f(unsafe { &*self.slot(mid) }) {
                core::cmp::Ordering::Less => left = add(mid, 1),
                core::cmp::Ordering::Greater => right = mid,
                core::cmp::Ordering::Equal => return Ok(mid),
            }
        }
        Err(left)
    }
    pub fn partition_point(&self, mut f: impl FnMut(&T) -> bool) -> usize {
        let mut left = 0;
        let mut right = self.len();
        while left < right {
            let mid = add(left, sub(right, left) / 2);
            // SAFETY: bounded live midpoint.
            if f(unsafe { &*self.slot(mid) }) {
                left = add(mid, 1)
            } else {
                right = mid
            }
        }
        left
    }
}
