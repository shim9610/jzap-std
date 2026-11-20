//! Single-allocation grouped table. Only FULL tags own initialized pairs.
use super::group::{self, DELETED, EMPTY, WIDTH};
use crate::{
    Checked, CopyWitness, Error,
    arithmetic::bounded::{wrapping_add as add, wrapping_mul as mul, wrapping_sub as sub},
    budget::Accounting,
    storage,
};
use core::{
    alloc::Layout,
    marker::PhantomData,
    mem,
    ptr::{self, NonNull},
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

pub(crate) enum InsertSlot {
    Occupied(usize),
    Vacant(usize),
}

pub(crate) fn buckets_for(capacity: usize) -> Result<usize, Error> {
    if capacity == 0 {
        return Ok(0);
    }
    if capacity < 4 {
        return Ok(4);
    }
    if capacity < 8 {
        return Ok(8);
    }
    let scaled = capacity.checked_mul(8).ok_or(Error::Capacity)? / 7;
    scaled.checked_next_power_of_two().ok_or(Error::Capacity)
}
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires mask < usize::MAX,
    ensures out as int == if mask < 8 { mask as int } else { (mask as int + 1) / 8 * 7 },
        out <= mask,
))]
pub(crate) fn capacity_for(mask: usize) -> usize {
    if mask < 8 {
        mask
    } else {
        mul(add(mask, 1) / 8, 7)
    }
}
#[inline]
pub(crate) fn checked_layout(
    size: usize,
    alignment: usize,
    buckets: usize,
) -> Result<(Layout, usize), Error> {
    let align = alignment.max(WIDTH);
    let (bytes, offset) = super::geometry::checked_sizes(size, alignment, buckets)?;
    let layout = Layout::from_size_align(bytes, align).map_err(|_| Error::Layout)?;
    Ok((layout, offset))
}
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out < 0x80,
))]
#[inline]
pub(crate) fn tag(hash: u64) -> u8 {
    #[cfg(verus_keep_ghost)]
    proof! {
        assert((hash >> 57) < 0x80) by(bit_vector);
    }
    (hash >> 57) as u8
}

pub(crate) use super::geometry::{mirror, probe_next};

pub(crate) struct RawTable<K, V> {
    pub(crate) ctrl: NonNull<u8>,
    pub(crate) mask: usize,
    pub(crate) len: Checked<usize>,
    pub(crate) growth: Checked<usize>,
    marker: PhantomData<(K, V)>,
}
impl<K, V> RawTable<K, V> {
    pub(crate) const fn empty() -> Self {
        Self {
            ctrl: NonNull::dangling(),
            mask: 0,
            len: Checked::new(0),
            growth: Checked::new(0),
            marker: PhantomData,
        }
    }
    pub(crate) fn allocate(buckets: usize, budget: Accounting<'_>) -> Result<Self, Error> {
        if buckets == 0 {
            return Ok(Self::empty());
        }
        let (layout, offset) =
            checked_layout(mem::size_of::<(K, V)>(), mem::align_of::<(K, V)>(), buckets)?;
        let allocation = storage::allocate(layout, budget)?;
        // SAFETY: checked layout includes buckets+WIDTH initialized ctrl bytes.
        let ctrl = unsafe {
            let p = allocation.as_ptr().add(offset);
            ptr::write_bytes(p, EMPTY, add(buckets, WIDTH));
            NonNull::new_unchecked(p)
        };
        let mask = sub(buckets, 1);
        Ok(Self {
            ctrl,
            mask,
            len: Checked::new(0),
            growth: Checked::new(capacity_for(mask)),
            marker: PhantomData,
        })
    }
    /// Duplicate an admitted table without first filling its control bytes.
    /// The capabilities require bitwise-copyable fields; no user code runs
    /// between allocation and publication of the fully copied table.
    pub(crate) fn try_clone_copy(
        &self,
        budget: Accounting<'_>,
        _keys: CopyWitness<K>,
        _values: CopyWitness<V>,
    ) -> Result<Self, Error> {
        if self.mask == 0 {
            return Ok(Self::empty());
        }
        let (layout, offset) = checked_layout(
            mem::size_of::<(K, V)>(),
            mem::align_of::<(K, V)>(),
            self.buckets(),
        )?;
        let allocation = storage::allocate(layout, budget)?;
        // Before publication, this guard owns only raw storage: it never reads
        // uninitialized controls or attempts to destroy an unpublished pair.
        let free = Free {
            pointer: allocation,
            layout,
            budget,
        };
        // SAFETY: the source's admitted shape has this exact layout. The new
        // allocation is disjoint and equally aligned. Byte copying preserves
        // uninitialized slots/padding and pointer provenance. Both fields have
        // Copy capabilities; every control and mirror byte is copied before
        // FULL tags, length and growth become visible through the new owner.
        let ctrl = unsafe {
            let (source, _) = self.allocation();
            ptr::copy_nonoverlapping(source.as_ptr(), allocation.as_ptr(), layout.size());
            NonNull::new_unchecked(allocation.as_ptr().add(offset))
        };
        let table = Self {
            ctrl,
            mask: self.mask,
            len: self.len,
            growth: self.growth,
            marker: PhantomData,
        };
        mem::forget(free);
        Ok(table)
    }
    #[inline]
    pub(crate) fn buckets(&self) -> usize {
        if self.mask == 0 { 0 } else { add(self.mask, 1) }
    }
    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        add(self.len.get(), self.growth.get())
    }
    /// # Safety
    /// Nonempty allocated table; reconstruct the admitted original layout.
    pub(crate) unsafe fn allocation(&self) -> (NonNull<u8>, Layout) {
        let buckets = self.buckets();
        let align = mem::align_of::<(K, V)>().max(WIDTH);
        let round = sub(align, 1);
        let offset = add(mul(buckets, mem::size_of::<(K, V)>()), round) & !round;
        let bytes = add(add(offset, buckets), WIDTH);
        // SAFETY: identical to successful checked_layout; table retains its shape.
        unsafe {
            (
                NonNull::new_unchecked(self.ctrl.as_ptr().sub(offset)),
                Layout::from_size_align_unchecked(bytes, align),
            )
        }
    }
    /// # Safety
    /// index < buckets; no references to a moved/dropped slot may survive.
    #[inline(always)]
    pub(crate) unsafe fn slot(&self, index: usize) -> *mut (K, V) {
        // SAFETY: reverse slots lie before ctrl inside the admitted allocation.
        unsafe { self.ctrl.as_ptr().cast::<(K, V)>().sub(add(index, 1)) }
    }
    /// # Safety
    /// `index` identifies a FULL slot under an exclusive table borrow. The
    /// replacement key must preserve the caller's hash/equality class.
    #[inline(always)]
    pub(crate) unsafe fn replace_key(&mut self, index: usize, key: K) -> K {
        // SAFETY: one initialized key is replaced without changing the tuple's
        // other field, its control tag or the table's ownership metadata.
        unsafe { mem::replace(&mut *ptr::addr_of_mut!((*self.slot(index)).0), key) }
    }
    /// # Safety
    /// index < buckets, initialized metadata in this live allocation.
    #[inline(always)]
    pub(crate) unsafe fn control(&self, index: usize) -> u8 {
        unsafe { self.ctrl.as_ptr().add(index).read() }
    }
    /// # Safety
    /// index < buckets; callers maintain FULL iff the pair is initialized.
    #[inline(always)]
    pub(crate) unsafe fn set_control(&mut self, index: usize, value: u8) {
        // Equivalent to ((index-WIDTH)&mask)+WIDTH without modular underflow.
        // A live control slot belongs to an allocated, nonempty table.
        let mirror = mirror(index, add(self.mask, 1));
        // SAFETY: index < buckets and mirror < buckets+WIDTH. For small tables
        // unused padding stays EMPTY, and mirrors occupy WIDTH..WIDTH+buckets.
        unsafe {
            self.ctrl.as_ptr().add(index).write(value);
            self.ctrl.as_ptr().add(mirror).write(value)
        }
    }
    #[inline]
    fn groups(&self) -> usize {
        if self.mask < WIDTH {
            1
        } else {
            add(self.mask / WIDTH, 1)
        }
    }
    #[inline]
    pub(crate) fn find(&self, hash: u64, mut equal: impl FnMut(&K) -> bool) -> Option<usize> {
        if self.len.get() == 0 {
            return None;
        }
        let mut pos = (hash as usize) & self.mask;
        let mut stride = 0;
        let mut groups = self.groups();
        while groups > 0 {
            // SAFETY: masked pos plus the full group lies in initialized ctrl.
            unsafe {
                let address = self.ctrl.as_ptr().add(pos);
                let mut bits = group::matching(address, tag(hash));
                while bits != 0 {
                    let index = add(pos, group::first(bits)) & self.mask;
                    if equal(&*ptr::addr_of!((*self.slot(index)).0)) {
                        return Some(index);
                    }
                    bits = group::clear_first(bits);
                }
                if group::any_empty(address) {
                    return None;
                }
            }
            groups = sub(groups, 1);
            if groups != 0 {
                (pos, stride) = probe_next(pos, stride, self.mask);
            }
        }
        None
    }
    /// One bounded search retains a vacant candidate while checking later
    /// matches. A tombstone never terminates the search for an existing key.
    #[inline(always)]
    pub(crate) fn find_insert(
        &self,
        hash: u64,
        mut equal: impl FnMut(&K) -> bool,
    ) -> Option<InsertSlot> {
        if self.mask == 0 {
            return None;
        }
        let mut pos = (hash as usize) & self.mask;
        let mut stride = 0;
        let mut vacant = None;
        loop {
            // SAFETY: masked group reads are inside initialized controls. FULL
            // tags identify initialized keys; the source borrow preserves them.
            unsafe {
                let address = self.ctrl.as_ptr().add(pos);
                let mut bits = group::matching(address, tag(hash));
                while bits != 0 {
                    let index = add(pos, group::first(bits)) & self.mask;
                    if equal(&*ptr::addr_of!((*self.slot(index)).0)) {
                        return Some(InsertSlot::Occupied(index));
                    }
                    bits = group::clear_first(bits);
                }
                if vacant.is_none() {
                    let slots = group::Vacancy::load(address);
                    if slots.found() {
                        let index = add(pos, slots.lane()) & self.mask;
                        if self.control(index) & 0x80 != 0 {
                            vacant = Some(index);
                        } else {
                            // Small-table EMPTY padding can wrap onto FULL.
                            let index = group::first_vacant(self.ctrl.as_ptr());
                            if index < self.buckets() {
                                vacant = Some(index);
                            }
                        }
                    }
                }
                if group::any_empty(address) {
                    return vacant.map(InsertSlot::Vacant);
                }
            }
            // The step grows by WIDTH without wrapping. Check it only after
            // an unsuccessful group, keeping the first-hit path free of a
            // separate group-count calculation.
            (pos, stride) = probe_next(pos, stride, self.mask);
            if stride > self.mask {
                return vacant.map(InsertSlot::Vacant);
            }
        }
    }
    /// Requires at least one vacant bucket; returns only a genuinely vacant slot.
    #[inline]
    pub(crate) fn vacant(&self, hash: u64) -> Option<usize> {
        if self.mask == 0 {
            return None;
        }
        let mut pos = (hash as usize) & self.mask;
        let mut stride = 0;
        let mut groups = self.groups();
        while groups > 0 {
            // SAFETY: allocated initialized ctrl and masked pos.
            unsafe {
                let slots = group::Vacancy::load(self.ctrl.as_ptr().add(pos));
                if slots.found() {
                    let index = add(pos, slots.lane()) & self.mask;
                    if self.control(index) & 0x80 != 0 {
                        return Some(index);
                    }
                    // Small-table padding can name an occupied wrapped slot.
                    // The aligned group contains every real slot before padding.
                    let index = group::first_vacant(self.ctrl.as_ptr());
                    return (index < self.buckets()).then_some(index);
                }
            }
            groups = sub(groups, 1);
            if groups != 0 {
                (pos, stride) = probe_next(pos, stride, self.mask);
            }
        }
        None
    }
    /// # Safety
    /// index is vacant and ownership of pair transfers to this table.
    #[inline]
    pub(crate) unsafe fn insert_at(&mut self, index: usize, hash: u64, pair: (K, V)) {
        unsafe {
            if self.control(index) == EMPTY {
                self.growth = Checked::new(sub(self.growth.get(), 1));
            }
            self.slot(index).write(pair);
            self.set_control(index, tag(hash));
        }
        self.len = Checked::new(add(self.len.get(), 1));
    }
    /// # Safety
    /// Initialized index; exclusive access with no surviving value references.
    #[inline]
    pub(crate) unsafe fn erase(&mut self, index: usize) -> (K, V) {
        let before = sub(mirror(index, add(self.mask, 1)), WIDTH);
        // SAFETY: both groups are initialized, key/value belongs to this slot.
        unsafe {
            let a = group::matching(self.ctrl.as_ptr().add(before), EMPTY).leading_zeros();
            let b = group::matching(self.ctrl.as_ptr().add(index), EMPTY).trailing_zeros();
            let value = if add(a as usize, b as usize) >= WIDTH {
                DELETED
            } else {
                self.growth = Checked::new(add(self.growth.get(), 1));
                EMPTY
            };
            self.set_control(index, value);
            self.len = Checked::new(sub(self.len.get(), 1));
            self.slot(index).read()
        }
    }
    pub(crate) fn clear(&mut self) {
        if !mem::needs_drop::<(K, V)>() {
            if self.mask != 0 {
                // SAFETY: admitted initialized control span; values need no drop.
                unsafe {
                    ptr::write_bytes(self.ctrl.as_ptr(), EMPTY, add(self.buckets(), WIDTH));
                }
            }
            self.len = Checked::new(0);
            self.growth = Checked::new(capacity_for(self.mask));
            return;
        }
        let mut guard = Clear {
            table: self,
            index: 0,
            pending: false,
            active: true,
        };
        guard.run();
    }
    pub(crate) fn rehash(&mut self, mut hash: impl FnMut(&K) -> u64) -> Result<(), Error> {
        // No fallible operations or callbacks during the tag transformation.
        for i in 0..self.buckets() {
            // SAFETY: metadata initialized; mark exactly initialized pairs pending.
            unsafe {
                let value = if self.control(i) & 0x80 == 0 {
                    DELETED
                } else {
                    EMPTY
                };
                self.set_control(i, value)
            }
        }
        let mut guard = Clear {
            table: self,
            index: 0,
            pending: true,
            active: true,
        };
        for index in 0..guard.table.buckets() {
            // SAFETY: DELETED uniquely identifies a still initialized pending pair.
            unsafe {
                if guard.table.control(index) != DELETED {
                    continue;
                }
                loop {
                    let value = hash(&*ptr::addr_of!((*guard.table.slot(index)).0));
                    let target = guard.table.vacant(value).ok_or(Error::InvalidStructure)?;
                    let start = (value as usize) & guard.table.mask;
                    let a = sub(add(index, guard.table.buckets()), start) & guard.table.mask;
                    let b = sub(add(target, guard.table.buckets()), start) & guard.table.mask;
                    if a / WIDTH == b / WIDTH {
                        guard.table.set_control(index, tag(value));
                        break;
                    }
                    let previous = guard.table.control(target);
                    guard.table.set_control(target, tag(value));
                    if previous == EMPTY {
                        guard
                            .table
                            .slot(target)
                            .write(guard.table.slot(index).read());
                        guard.table.set_control(index, EMPTY);
                        break;
                    }
                    // Two distinct, initialized pending slots; swap resolves one
                    // pending owner and leaves the other at index for this loop.
                    ptr::swap(guard.table.slot(index), guard.table.slot(target));
                }
            }
        }
        guard.table.growth =
            Checked::new(sub(capacity_for(guard.table.mask), guard.table.len.get()));
        guard.active = false;
        Ok(())
    }
}
struct Clear<'a, K, V> {
    table: &'a mut RawTable<K, V>,
    index: usize,
    pending: bool,
    active: bool,
}
impl<K, V> Clear<'_, K, V> {
    fn run(&mut self) {
        if !self.active {
            return;
        }
        // Full-table retirement scans whole groups: one mask of FULL slots per
        // 16 control bytes instead of one data-dependent branch per bucket.
        // `index` stays on the current group, and every retired slot is marked
        // EMPTY before its destructor runs, so a resumed run after an unwind
        // re-reads the group and sees only the still-owned slots.
        while !self.pending && self.index < self.table.buckets() {
            let base = self.index;
            // SAFETY: base is a multiple of WIDTH below buckets; the allocation
            // holds buckets + WIDTH initialized control bytes, so the 16-byte
            // read is in bounds. For tables smaller than a group the bytes past
            // buckets are EMPTY padding, never mirrors, so each set bit is a
            // real FULL slot below buckets owning one initialized pair.
            unsafe {
                let mut bits = !group::vacant(self.table.ctrl.as_ptr().add(base));
                while bits != 0 {
                    let index = add(base, group::first(bits));
                    bits = group::clear_first(bits);
                    self.table.set_control(index, EMPTY);
                    self.table.len = Checked::new(sub(self.table.len.get(), 1));
                    ptr::drop_in_place(self.table.slot(index));
                }
            }
            self.index = add(base, WIDTH);
        }
        while self.index < self.table.buckets() {
            let index = self.index;
            self.index = add(index, 1);
            // SAFETY: flag determines exactly which slots own values. Clear the
            // ownership flag/count before user Drop, so unwinding resumes once.
            unsafe {
                let c = self.table.control(index);
                if (self.pending && c == DELETED) || (!self.pending && c & 0x80 == 0) {
                    self.table.set_control(index, EMPTY);
                    self.table.len = Checked::new(sub(self.table.len.get(), 1));
                    ptr::drop_in_place(self.table.slot(index));
                }
            }
        }
        if self.table.mask != 0 {
            if self.pending {
                self.table.growth =
                    Checked::new(sub(capacity_for(self.table.mask), self.table.len.get()));
            } else {
                // SAFETY: all pairs retired; reset tombstones and mirror/padding.
                unsafe {
                    ptr::write_bytes(
                        self.table.ctrl.as_ptr(),
                        EMPTY,
                        add(self.table.buckets(), WIDTH),
                    );
                }
                self.table.growth = Checked::new(capacity_for(self.table.mask));
            }
        }
        self.active = false;
    }
}
impl<K, V> Drop for Clear<'_, K, V> {
    fn drop(&mut self) {
        self.run()
    }
}

pub(crate) struct Free<'a> {
    pub(crate) pointer: NonNull<u8>,
    pub(crate) layout: Layout,
    pub(crate) budget: Accounting<'a>,
}
impl Drop for Free<'_> {
    fn drop(&mut self) {
        // SAFETY: the guard receives one exclusively owned, already detached allocation.
        unsafe { storage::deallocate(self.pointer, self.layout, self.budget) }
    }
}

/// An owning temporary; storage is freed even if one payload destructor unwinds.
pub(crate) struct Owner<'a, K, V> {
    pub(crate) table: RawTable<K, V>,
    pub(crate) budget: Accounting<'a>,
}
impl<K, V> Drop for Owner<'_, K, V> {
    fn drop(&mut self) {
        if self.table.mask == 0 {
            return;
        }
        // SAFETY: this temporary owns its table's original admitted allocation.
        let (pointer, layout) = unsafe { self.table.allocation() };
        let _free = Free {
            pointer,
            layout,
            budget: self.budget,
        };
        if mem::needs_drop::<(K, V)>() {
            self.table.clear();
        }
    }
}
/// Owns a table's allocation but never its pairs: dropping frees storage
/// only. A resize destination holds unpublished bitwise copies whose owner is
/// still the source; after publication it holds the retired source, whose pairs
/// moved. Neither may be dropped here.
pub(crate) struct Storage<'a, K, V> {
    pub(crate) table: RawTable<K, V>,
    pub(crate) budget: Accounting<'a>,
}
impl<K, V> Drop for Storage<'_, K, V> {
    fn drop(&mut self) {
        if self.table.mask == 0 {
            return;
        }
        // SAFETY: this guard owns its table's original admitted allocation.
        let (pointer, layout) = unsafe { self.table.allocation() };
        drop(Free {
            pointer,
            layout,
            budget: self.budget,
        });
    }
}
