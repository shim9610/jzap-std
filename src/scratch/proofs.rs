use super::*;
fn ok<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("proof setup failed"),
    }
}
#[kani::proof]
#[kani::unwind(8)]
fn actual_borrowed_admission_rollback_and_bytes() {
    let values: [u8; 3] = kani::any();
    let mut cells = [MaybeUninit::uninit(); 4];
    {
        let mut s = Scratch::new(&mut cells);
        ok(s.try_push(values[0]));
        let result = s.try_resize_with_init(3, |index, slot| {
            ok(slot.write(values[index]));
            if index == 2 { Err(()) } else { Ok(()) }
        });
        assert!(matches!(result, Err(InitError::Initialize(()))));
        assert!(s.len() == 1 && s.get(0) == Ok(&values[0]));
        ok(s.try_extend_copy(&values[1..]));
        assert!(s.as_slice() == values);
        assert!(s.try_extend_copy(&[9, 10]) == Err(Error::Capacity));
        assert!(s.as_slice() == values);
        assert!(s.pop() == Some(values[2]));
    }
    let mut bytes: [u8; 4] = kani::any();
    let original = bytes;
    {
        let mut s = Scratch::from_bytes(&mut bytes);
        ok(s.try_extend_copy(&values));
        s.clear();
    }
    assert!(
        bytes[0] == values[0]
            && bytes[1] == values[1]
            && bytes[2] == values[2]
            && bytes[3] == original[3]
    );
}
#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(8)]
fn actual_owned_storage_never_grows_after_construction() {
    use crate::budget::{Budget, MemoryBudget};
    let budget = MemoryBudget::new(4);
    let mut s = ok(ScratchBuf::try_with_capacity_in(
        4,
        Budget::Limited(&budget),
    ));
    let pointer = s.as_slice().as_ptr();
    let values: [u8; 4] = kani::any();
    ok(s.try_extend_copy(&values));
    assert!(matches!(
        s.try_push(9),
        Err(Rejected {
            value: 9,
            error: Error::Capacity
        })
    ));
    assert!(s.try_extend_cloned(&[1]) == Err(Error::Capacity));
    assert!(s.as_slice() == values && s.capacity() == 4 && s.as_slice().as_ptr() == pointer);
    s.clear();
    ok(s.try_extend_cloned(&values));
    s.clear();
    let mut source = ok(ScratchBuf::try_with_capacity_in(2, Budget::Unmetered));
    ok(source.try_extend_copy(&values[..2]));
    ok(s.try_clone_from(&source));
    assert!(s.as_slice() == &values[..2] && s.as_slice().as_ptr() == pointer);
    s.clear();
    ok(s.try_resize(4, &2));
    assert!(s.as_slice() == [2, 2, 2, 2]);
    assert!(s.as_slice().as_ptr() == pointer && budget.occupied() == 4 && budget.reserved() == 0);
    drop(s);
    assert!(budget.occupied() == 0);
}
#[kani::proof]
#[kani::unwind(4)]
fn actual_zst_all_word_lengths_retire_without_wrap() {
    let (capacity, len, keep): (usize, usize, usize) = kani::any();
    kani::assume(len <= capacity);
    // A valid zero-byte backing slice for an arbitrary logical slot count.
    let storage = unsafe {
        slice::from_raw_parts_mut(NonNull::<MaybeUninit<()>>::dangling().as_ptr(), capacity)
    };
    let mut s = Scratch::new(storage);
    ok(s.try_resize(len, &()));
    assert!(s.len() == len);
    let (start, end, destination, middle, other_len): (usize, usize, usize, usize, usize) =
        kani::any();
    let other = unsafe { slice::from_raw_parts_mut(NonNull::<()>::dangling().as_ptr(), other_len) };
    let mut view = s.as_view_mut();
    let result = view.copy_within(start..end, destination);
    let admitted =
        start <= end && end <= len && destination <= len && end - start <= len - destination;
    assert!(result.is_ok() == admitted);
    let result = view.copy_from_slice(other);
    assert!(result.is_ok() == (len == other_len));
    let result = view.swap_with_slice(other);
    assert!(result.is_ok() == (len == other_len));
    let result = view.split_at_mut(middle);
    assert!(result.is_ok() == (middle <= len));
    if let Ok((a, b)) = result {
        assert!(a.len() == middle && b.len() == len - middle);
    }
    s.truncate(keep);
    assert!(s.len() == len.min(keep));
    s.clear();
    assert!(s.is_empty());
    #[cfg(feature = "alloc")]
    {
        let mut s = ScratchBuf::<()>::new();
        ok(s.try_resize(len, &()));
        s.truncate(keep);
        assert!(s.len() == len.min(keep));
        s.clear();
        assert!(s.is_empty());
    }
}

#[cfg(feature = "alloc")]
#[kani::proof]
#[kani::unwind(8)]
fn actual_noncopy_staging_and_missing_write() {
    struct C(u8, bool);
    impl TryClone for C {
        fn try_clone(&self) -> Result<Self, Error> {
            if self.1 {
                Err(Error::Capacity)
            } else {
                Ok(C(self.0, false))
            }
        }
    }
    let (old, a, b): (u8, u8, u8) = kani::any();
    let fail: bool = kani::any();
    let mut owner = ok(ScratchBuf::try_with_capacity(4));
    ok(owner.try_push(C(old, false)));
    let pointer = owner.as_slice().as_ptr();
    let mut source = ok(ScratchBuf::try_with_capacity(2));
    ok(source.try_push(C(a, false)));
    ok(source.try_push(C(b, fail)));
    let result = owner.try_clone_from(&source);
    if fail {
        assert!(result == Err(Error::Capacity));
        assert!(owner.len() == 1 && ok(owner.get(0)).0 == old);
    } else {
        assert!(result == Ok(()));
        assert!(owner.len() == 2 && ok(owner.get(0)).0 == a && ok(owner.get(1)).0 == b);
    }
    assert!(owner.capacity() == 4 && owner.as_slice().as_ptr() == pointer);
    let before = owner.len();
    let result = owner.try_resize_with_init(3, |_, _| Ok::<(), ()>(()));
    assert!(result == Err(InitError::Storage(Error::Uninitialized)) && owner.len() == before);
    let mut cells = [const { MaybeUninit::uninit() }; 4];
    let mut borrowed = Scratch::new(&mut cells);
    ok(borrowed.try_push(C(old, false)));
    let result = borrowed.try_extend_cloned(source.as_slice());
    assert!(result.is_err() == fail);
    assert!(borrowed.len() == if fail { 1 } else { 3 });
    assert!(ok(borrowed.get(0)).0 == old);
    let before = borrowed.len();
    let result = borrowed.try_resize_with_init(4, |_, _| Ok::<(), ()>(()));
    assert!(result == Err(InitError::Storage(Error::Uninitialized)) && borrowed.len() == before);
}
