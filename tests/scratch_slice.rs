use jzap::{
    Error, TryClone, TryCloneIn, TryIterator,
    budget::{Budget, MemoryBudget},
    error::InitError,
    iter::from_std,
    scratch::{Scratch, ScratchBuf},
    slice::{Slice, SliceMut},
};
use std::{cell::Cell, mem::MaybeUninit, ops::Bound, rc::Rc};
#[allow(dead_code)]
mod common;
#[global_allocator]
static A: common::FaultAllocator = common::FaultAllocator;

#[test]
fn fixed_owned_storage_and_failure_injection() {
    let budget = MemoryBudget::new(32);
    let (_, state) = common::track(1, || {
        assert!(matches!(
            ScratchBuf::<u64, _>::try_with_capacity_in(4, Budget::Limited(&budget)),
            Err(Error::Allocation { .. })
        ));
    });
    assert_eq!(state.calls, 1);
    assert_eq!(budget.occupied(), 0);
    assert_eq!(budget.reserved(), 0);
    let mut s = ScratchBuf::try_with_capacity_in(4, Budget::Limited(&budget)).unwrap();
    let ptr = s.as_slice().as_ptr();
    let (_, state) = common::track(usize::MAX, || {
        for _ in 0..8 {
            s.try_extend_copy(&[1u64, 2]).unwrap();
            s.try_extend_cloned(&[3, 4]).unwrap();
            let rejected = s.try_push(5).unwrap_err();
            assert_eq!(rejected.value, 5);
            assert_eq!(rejected.error, Error::Capacity);
            assert_eq!(s.try_extend_copy(&[5]), Err(Error::Capacity));
            assert_eq!(s.try_resize(5, &0), Err(Error::Capacity));
            assert_eq!(s.as_slice(), [1, 2, 3, 4]);
            s.clear();
            s.try_resize(4, &7).unwrap();
            assert_eq!(s.pop(), Some(7));
            s.truncate(0);
            assert_eq!(s.as_slice().as_ptr(), ptr);
            assert_eq!(s.capacity(), 4);
        }
    });
    assert_eq!(state.calls, 0);
    assert_eq!(budget.occupied(), 32);
    let v = s.into_vec();
    assert_eq!(v.as_ptr(), ptr);
    let s = ScratchBuf::from_vec(v);
    assert_eq!(s.as_slice().as_ptr(), ptr);
    drop(s);
    assert_eq!(budget.occupied(), 0);
}

#[derive(Debug)]
struct Item {
    value: u8,
    drops: Rc<Cell<usize>>,
    fail: bool,
}
impl Drop for Item {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl TryClone for Item {
    fn try_clone(&self) -> Result<Self, Error> {
        if self.fail {
            return Err(Error::Capacity);
        }
        Ok(Self {
            value: self.value,
            drops: self.drops.clone(),
            fail: self.fail,
        })
    }
    fn try_extend_vec<P: jzap::budget::AllocationPolicy, L: jzap::vec::LengthPolicy>(
        _: &[Self],
        _: &mut jzap::vec::Vec<Self, P, L>,
    ) -> Option<Result<(), Error>> {
        panic!("fixed scratch must never expose its Vec to this hook")
    }
}
fn item(value: u8, drops: &Rc<Cell<usize>>) -> Item {
    Item {
        value,
        drops: drops.clone(),
        fail: false,
    }
}

#[test]
fn borrowed_initialization_rolls_back_each_slot_once() {
    for failure in 1..4 {
        for after_write in [false, true] {
            let drops = Rc::new(Cell::new(0));
            let mut cells: [MaybeUninit<Item>; 4] = [const { MaybeUninit::uninit() }; 4];
            let mut s = Scratch::new(&mut cells);
            s.try_push(item(9, &drops)).unwrap();
            let result = s.try_resize_with_init(4, |index, slot| {
                if index == failure && !after_write {
                    return Err(42);
                }
                slot.write(item(index as u8, &drops)).unwrap();
                if index == failure { Err(42) } else { Ok(()) }
            });
            assert_eq!(result, Err(InitError::Initialize(42)));
            assert_eq!(s.len(), 1);
            assert_eq!(s.get(0).unwrap().value, 9);
            assert_eq!(drops.get(), failure - 1 + usize::from(after_write));
            assert_eq!(
                s.try_resize_with_init(2, |_, _| Ok::<_, ()>(())),
                Err(InitError::Storage(Error::Uninitialized))
            );
            s.try_resize_with_init(4, |i, slot| {
                slot.write(item(i as u8, &drops)).map_err(|r| r.error)
            })
            .unwrap();
            s.clear();
            assert_eq!(drops.get(), failure - 1 + usize::from(after_write) + 4);
        }
    }
}

#[test]
fn clone_hook_is_not_a_backing_escape_and_staging_is_transactional() {
    let drops = Rc::new(Cell::new(0));
    let input = [item(1, &drops), item(2, &drops)];
    let mut a = ScratchBuf::try_with_capacity(5).unwrap();
    a.try_push(item(9, &drops)).unwrap();
    let mut b = ScratchBuf::try_with_capacity(3).unwrap();
    b.try_extend_cloned(&input).unwrap();
    let ptr = a.as_slice().as_ptr();
    a.try_clone_from(&b).unwrap();
    assert_eq!(
        a.as_slice().iter().map(|v| v.value).collect::<Vec<_>>(),
        [1, 2]
    );
    b.as_mut_slice()[1].fail = true;
    assert_eq!(a.try_clone_from(&b), Err(Error::Capacity));
    assert_eq!(a.len(), 2);
    assert_eq!(a.as_slice()[0].value, 1);
    assert_eq!(a.as_slice().as_ptr(), ptr);
    let mut cells: [MaybeUninit<Item>; 3] = [const { MaybeUninit::uninit() }; 3];
    let mut borrowed = Scratch::new(&mut cells);
    borrowed.try_extend_cloned(&input).unwrap();
    assert_eq!(borrowed.len(), 2);
    let before = drops.get();
    assert_eq!(borrowed.try_extend_cloned(&input), Err(Error::Capacity));
    assert_eq!(drops.get(), before);
    let mut full = ScratchBuf::try_with_capacity(2).unwrap();
    full.try_extend_cloned(&input).unwrap();
    assert_eq!(full.try_clone_from(&b), Err(Error::Capacity));
    assert_eq!(drops.get(), before);
}

#[test]
fn copy_clone_from_uses_exhausted_budget_and_clone_has_separate_admission() {
    let budget = MemoryBudget::new(4);
    let mut destination = ScratchBuf::try_with_capacity_in(4, Budget::Limited(&budget)).unwrap();
    destination.try_extend_copy(&[9u8; 4]).unwrap();
    let mut source = ScratchBuf::try_with_capacity_in(2, Budget::Unmetered).unwrap();
    source.try_extend_copy(&[1, 2]).unwrap();
    let pointer = destination.as_slice().as_ptr();
    let (_, state) = common::track(1, || destination.try_clone_from(&source).unwrap());
    assert_eq!(state.calls, 0);
    assert_eq!(destination.as_slice(), [1, 2]);
    assert_eq!(destination.as_slice().as_ptr(), pointer);
    assert!(matches!(destination.try_clone(), Err(Error::Budget { .. })));
    let other = MemoryBudget::new(4);
    let clone = destination.try_clone_in(Budget::Limited(&other)).unwrap();
    assert_eq!(clone.capacity(), 4);
    assert_eq!(clone.as_slice(), [1, 2]);
    drop(clone);
    assert_eq!(other.occupied(), 0);
}

#[test]
fn bytes_remain_valid_after_failure_pop_and_clear() {
    let mut bytes = [17u8; 6];
    let (_, state) = common::track(usize::MAX, || {
        let mut s = Scratch::from_bytes(&mut bytes);
        s.try_extend_copy(&[1, 2]).unwrap();
        assert_eq!(
            s.try_resize_with_init(5, |i, slot| {
                slot.write(i as u8).unwrap();
                if i == 4 { Err(()) } else { Ok(()) }
            }),
            Err(InitError::Initialize(()))
        );
        assert_eq!(s.as_slice(), [1, 2]);
        s.try_extend_copy(&[8]).unwrap();
        assert_eq!(s.as_slice(), [1, 2, 8]);
        assert_eq!(s.pop(), Some(8));
        assert_eq!(s.pop(), Some(2));
        s.clear();
    });
    assert_eq!(state.calls, 0);
    assert_eq!(bytes, [1, 2, 8, 3, 4, 17]);
}

struct Source {
    at: usize,
    fail: usize,
}
impl TryIterator for Source {
    type Item = u8;
    type Error = u8;
    fn try_next(&mut self) -> Result<Option<u8>, u8> {
        if self.at == self.fail {
            return Err(77);
        }
        if self.at == 4 {
            return Ok(None);
        }
        self.at += 1;
        Ok(Some(self.at as u8))
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), u8> {
        Ok((usize::MAX, Some(0)))
    }
}
#[test]
fn iterator_source_and_capacity_errors_restore_prefix() {
    let mut cells = [MaybeUninit::uninit(); 4];
    let mut a = Scratch::new(&mut cells);
    let mut b = ScratchBuf::try_with_capacity(4).unwrap();
    a.try_push(9).unwrap();
    b.try_push(9).unwrap();
    for fail in [0, 1, 2] {
        assert_eq!(
            a.try_extend(Source { at: 0, fail }),
            Err(InitError::Initialize(77))
        );
        assert_eq!(
            b.try_extend(Source { at: 0, fail }),
            Err(InitError::Initialize(77))
        );
        assert_eq!(a.as_slice(), [9]);
        assert_eq!(b.as_slice(), [9]);
    }
    assert_eq!(
        a.try_extend(from_std([1, 2, 3, 4].into_iter())),
        Err(InitError::Storage(Error::Capacity))
    );
    assert_eq!(
        b.try_extend(from_std([1, 2, 3, 4].into_iter())),
        Err(InitError::Storage(Error::Capacity))
    );
    assert_eq!(a.as_slice(), [9]);
    assert_eq!(b.as_slice(), [9]);
}

#[test]
fn zst_alignment_and_element_limit() {
    let mut a = ScratchBuf::<()>::new();
    a.try_resize(usize::MAX, &()).unwrap();
    assert_eq!(a.try_push(()).unwrap_err().error, Error::Capacity);
    assert_eq!(a.pop(), Some(()));
    a.clear();
    a.try_resize(usize::MAX, &()).unwrap();
    a.try_extend_copy(&[]).unwrap();
    a.clear();
    // Valid zero-byte backing for every logical slot; no fixture Clone loop.
    let slots = unsafe {
        std::slice::from_raw_parts_mut(
            std::ptr::NonNull::<MaybeUninit<()>>::dangling().as_ptr(),
            usize::MAX,
        )
    };
    let mut full = Scratch::new(slots);
    full.try_resize(usize::MAX, &()).unwrap();
    full.try_extend_copy(&[]).unwrap();
    assert_eq!(full.try_push(()).unwrap_err().error, Error::Capacity);
    full.clear();
    let mut units = vec![(); usize::MAX];
    let mut other = vec![(); usize::MAX];
    let mut view = SliceMut::new(&mut units);
    view.swap(0, usize::MAX - 1).unwrap();
    view.copy_within(.., 0).unwrap();
    view.copy_from_slice(&other).unwrap();
    view.swap_with_slice(&mut other).unwrap();
    let mut b = ScratchBuf::try_with_capacity_and_limit(2, 3).unwrap();
    assert_eq!(b.capacity(), 3);
    b.try_resize(3, &()).unwrap();
    assert_eq!(b.try_resize(4, &()), Err(Error::Capacity));
    let clone = b.try_clone().unwrap();
    assert_eq!(clone.capacity(), 3);
    let mut cells = [MaybeUninit::uninit(); 3];
    let mut s = Scratch::new(&mut cells);
    s.try_resize(3, &()).unwrap();
    assert_eq!(s.try_resize(4, &()), Err(Error::Capacity));
    #[repr(align(4096))]
    struct Aligned(u8);
    let mut cells = [const { MaybeUninit::uninit() }; 2];
    let mut s = Scratch::new(&mut cells);
    s.try_push(Aligned(7)).ok().unwrap();
    assert_eq!(s.as_slice().as_ptr() as usize % 4096, 0);
    assert_eq!(s.pop().unwrap().0, 7);
}

#[test]
fn exhaustive_checked_slices_and_overlapping_copy() {
    let values = [0u8, 1, 2, 3, 4];
    let indices = [0, 1, 2, 3, 4, 5, 6, usize::MAX];
    for start in indices {
        for end in indices {
            for destination in indices {
                let mut actual = values;
                let mut expected = values;
                let valid =
                    start <= end && end <= 5 && destination <= 5 && end - start <= 5 - destination;
                let result = SliceMut::new(&mut actual).copy_within(start..end, destination);
                if valid {
                    expected.copy_within(start..end, destination);
                    assert_eq!(result, Ok(()));
                } else {
                    assert_eq!(result, Err(Error::Bounds));
                }
                assert_eq!(actual, expected);
            }
            let mut actual = values;
            let result = SliceMut::new(&mut actual).swap(start, end);
            if start < 5 && end < 5 {
                let mut expected = values;
                expected.swap(start, end);
                assert_eq!(actual, expected);
                assert_eq!(result, Ok(()));
            } else {
                assert_eq!(result, Err(Error::Bounds));
                assert_eq!(actual, values);
            }
        }
    }
    let view = Slice::new(&values);
    assert_eq!(view.range(..=usize::MAX), Err(Error::Bounds));
    assert_eq!(
        view.range((Bound::Excluded(usize::MAX), Bound::Unbounded)),
        Err(Error::Bounds)
    );
    for n in indices {
        assert_eq!(view.get(n).ok(), values.get(n));
        assert_eq!(view.split_at(n).is_ok(), n <= values.len());
        let mut a = values;
        let mut view = SliceMut::new(&mut a);
        if n <= 5 {
            let (mut l, mut r) = view.split_at_mut(n).unwrap();
            if let Ok(x) = l.get_mut(0) {
                *x = 8;
            }
            if let Ok(x) = r.get_mut(0) {
                *x = 9;
            }
        } else {
            assert!(view.split_at_mut(n).is_err());
        }
    }
    let mut a = values;
    let before = a;
    assert_eq!(
        SliceMut::new(&mut a).copy_from_slice(&[1, 2]),
        Err(Error::Bounds)
    );
    assert_eq!(a, before);
    let mut b = [9; 5];
    SliceMut::new(&mut a).swap_with_slice(&mut b).unwrap();
    assert_eq!(a, [9; 5]);
    assert_eq!(b, values);
    SliceMut::new(&mut a).copy_from_slice(&values).unwrap();
    let mut it = Slice::new(&a).iter();
    for v in values {
        assert_eq!(it.try_next().unwrap(), Some(&v));
    }
    assert_eq!(it.try_next().unwrap(), None);
}

#[test]
#[cfg(not(miri))]
fn destructor_unwind_retires_suffix_before_cleanup() {
    struct D<'a>(usize, &'a Cell<usize>);
    impl Drop for D<'_> {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
            if self.0 == 1 {
                panic!("one destructor unwinds");
            }
        }
    }
    let count = Cell::new(0);
    let mut cells = [const { MaybeUninit::uninit() }; 4];
    let mut s = Scratch::new(&mut cells);
    for i in 0..4 {
        s.try_push(D(i, &count)).ok().unwrap();
    }
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.truncate(1))).is_err());
    assert_eq!(s.len(), 1);
    assert_eq!(count.get(), 3);
    drop(s);
    assert_eq!(count.get(), 4);
}

#[test]
fn random_fixed_storage_matches_vec_and_borrowed_state() {
    let mut cells = [MaybeUninit::uninit(); 16];
    let mut borrowed = Scratch::new(&mut cells);
    let mut owned = ScratchBuf::try_with_capacity(16).unwrap();
    let mut expected = Vec::with_capacity(16);
    let mut seed = 0x38a6_179du64;
    for _ in 0..if cfg!(miri) { 80 } else { 5000 } {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let byte = seed as u8;
        let n = (seed >> 8) as usize % 19;
        match seed % 6 {
            0 => {
                let a = borrowed.try_push(byte).is_ok();
                let b = owned.try_push(byte).is_ok();
                assert_eq!(a, expected.len() < 16);
                assert_eq!(a, b);
                if a {
                    expected.push(byte);
                }
            }
            1 => {
                let value = expected.pop();
                assert_eq!(borrowed.pop(), value);
                assert_eq!(owned.pop(), value);
            }
            2 => {
                borrowed.truncate(n);
                owned.truncate(n);
                expected.truncate(n);
            }
            3 => {
                let a = borrowed.try_resize(n, &byte);
                let b = owned.try_resize(n, &byte);
                assert_eq!(a, b);
                if n <= 16 {
                    expected.resize(n, byte);
                    assert!(a.is_ok());
                } else {
                    assert_eq!(a, Err(Error::Capacity));
                }
            }
            4 => {
                let source = [byte; 3];
                let fits = expected.len() <= 13;
                assert_eq!(borrowed.try_extend_cloned(&source).is_ok(), fits);
                assert_eq!(owned.try_extend_cloned(&source).is_ok(), fits);
                if fits {
                    expected.extend_from_slice(&source);
                }
            }
            _ => {
                borrowed.clear();
                owned.clear();
                expected.clear();
            }
        }
        assert_eq!(borrowed.as_slice(), expected);
        assert_eq!(owned.as_slice(), expected);
    }
}

#[test]
fn generic_resize_views_and_nested_target_budget_cleanup() {
    let drops = Rc::new(Cell::new(0));
    let mut cells = [const { MaybeUninit::uninit() }; 4];
    let mut borrowed = Scratch::new(&mut cells);
    let mut owned = ScratchBuf::try_with_capacity(4).unwrap();
    let mut value = item(7, &drops);
    borrowed.try_resize(3, &value).unwrap();
    owned.try_resize(3, &value).unwrap();
    value.fail = true;
    assert_eq!(borrowed.try_resize(4, &value), Err(Error::Capacity));
    assert_eq!(owned.try_resize(4, &value), Err(Error::Capacity));
    assert_eq!(borrowed.len(), 3);
    assert_eq!(owned.len(), 3);
    assert_eq!(
        owned.try_resize_with_init(4, |_, _| Ok::<(), ()>(())),
        Err(InitError::Storage(Error::Uninitialized))
    );
    assert_eq!(owned.len(), 3);
    let mut iter = borrowed.iter_mut();
    iter.try_next().unwrap().unwrap().value = 9;
    drop(iter);
    owned.iter_mut().try_next().unwrap().unwrap().value = 9;
    assert_eq!(borrowed.as_view().get(0).unwrap().value, 9);
    assert_eq!(owned.as_view_mut().get_mut(0).unwrap().value, 9);
    assert_eq!(borrowed.remaining_capacity(), 1);
    assert_eq!(owned.remaining_capacity(), 1);
    assert!(borrowed.get(usize::MAX).is_err());
    assert!(owned.get_mut(usize::MAX).is_err());
    borrowed.truncate(0);
    owned.truncate(0);
    assert_eq!(drops.get(), 6);

    let original = MemoryBudget::new(4);
    let target = MemoryBudget::new(128);
    let mut inner = jzap::vec::Vec::try_with_capacity_in(4, Budget::Limited(&original)).unwrap();
    inner.try_extend_copy(&[1u8, 2, 3]).unwrap();
    let mut owner = ScratchBuf::try_with_capacity(1).unwrap();
    owner.try_push(inner).unwrap();
    for failure in 1..=2 {
        let (result, state) =
            common::track(failure, || owner.try_clone_in(Budget::Limited(&target)));
        assert!(matches!(result, Err(Error::Allocation { .. })));
        assert_eq!(state.calls, failure);
        assert_eq!((target.occupied(), target.reserved()), (0, 0));
        assert_eq!(original.occupied(), 4);
        assert_eq!(owner.get(0).unwrap().as_slice(), [1, 2, 3]);
    }
    let clone = owner.try_clone_in(Budget::Limited(&target)).unwrap();
    let inner = clone.get(0).unwrap();
    assert_eq!(inner.as_slice(), [1, 2, 3]);
    assert!(matches!(inner.allocation_policy(), Budget::Limited(b) if std::ptr::eq(*b, &target)));
    assert_eq!(
        target.occupied(),
        std::mem::size_of_val(inner) + inner.capacity()
    );
    drop(clone);
    assert_eq!(target.occupied(), 0);
    assert!(matches!(
        ScratchBuf::<u64>::try_with_capacity(usize::MAX),
        Err(Error::Layout | Error::Capacity)
    ));
    assert!(matches!(
        ScratchBuf::<u8>::try_with_capacity_and_limit(4, 3),
        Err(Error::Capacity)
    ));
}

#[test]
fn every_range_bound_class_matches_checked_std_slices() {
    let original = [2u8, 3, 5, 7, 11];
    let points = [0, 1, 4, 5, 6, usize::MAX - 1, usize::MAX];
    for a in points {
        for b in points {
            for start in [Bound::Included(a), Bound::Excluded(a), Bound::Unbounded] {
                for end in [Bound::Included(b), Bound::Excluded(b), Bound::Unbounded] {
                    let lo = match start {
                        Bound::Included(v) => Some(v),
                        Bound::Excluded(v) => v.checked_add(1),
                        Bound::Unbounded => Some(0),
                    };
                    let hi = match end {
                        Bound::Included(v) => v.checked_add(1),
                        Bound::Excluded(v) => Some(v),
                        Bound::Unbounded => Some(5),
                    };
                    let expected = lo.zip(hi).and_then(|(a, b)| original.get(a..b));
                    assert_eq!(
                        Slice::new(&original)
                            .range((start, end))
                            .ok()
                            .map(|s| s.as_slice()),
                        expected
                    );
                    let mut values = original;
                    let mut view = SliceMut::new(&mut values);
                    let actual = view.range_mut((start, end));
                    assert_eq!(actual.as_ref().ok().map(|s| s.as_slice()), expected);
                }
            }
        }
    }
    let view = Slice::new(&original);
    let cloned = view.try_clone().unwrap();
    assert_eq!(cloned.as_slice().as_ptr(), original.as_ptr());
    assert_eq!((view.first(), view.last()), (Some(&2), Some(&11)));
    assert_eq!(view.try_to_vec().unwrap().as_slice(), original);
    assert_eq!(format!("{view:?}"), "[2, 3, 5, 7, 11]");
    let mut values = original;
    let view = SliceMut::new(&mut values);
    assert_eq!(view.try_to_vec().unwrap().as_slice(), original);
    assert_eq!(view.into_shared(), cloned);
    assert_eq!(SliceMut::new(&mut values).into_slice(), original);
}
