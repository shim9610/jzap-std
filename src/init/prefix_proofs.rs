use super::*;
#[kani::proof]
fn every_word_leaf_index_and_normalized_reply() {
    let index: usize = kani::any();
    let mode: u8 = kani::any();
    kani::assume(mode < 4);
    let mut seen = false;
    let mut backing = 0u8;
    let mut callback = |actual: usize, slot: &mut InitSlot<'_, ()>| {
        assert!(actual == index);
        seen = true;
        if mode == 0 {
            return Err(17u8);
        }
        if mode != 2 {
            assert!(slot.write(()).is_ok());
        }
        if mode == 3 {
            return Err(17u8);
        }
        Ok(())
    };
    let result = unsafe {
        initialize_one(
            NonNull::from(&mut backing).cast::<()>(),
            index,
            &mut callback,
        )
    };
    assert!(seen && backing == 0);
    match mode {
        0 | 3 => assert!(matches!(result, Err(InitError::Initialize(17)))),
        1 => assert!(result.is_ok()),
        _ => assert!(matches!(
            result,
            Err(InitError::Storage(Error::Uninitialized))
        )),
    }
}
#[kani::proof]
#[kani::unwind(5)]
fn cleanup_owns_exactly_the_completed_prefix_and_current_slot() {
    use core::{cell::Cell, mem::MaybeUninit};
    struct Value<'a> {
        id: u8,
        dropped: &'a Cell<u8>,
    }
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            assert!(self.id < 3);
            let bit = 1u8 << self.id;
            assert!(self.dropped.get() & bit == 0);
            self.dropped.set(self.dropped.get() | bit);
        }
    }
    let length: usize = kani::any();
    kani::assume(length <= 3);
    let fail_at: usize = kani::any();
    let mode: u8 = kani::any();
    kani::assume(mode < 3);
    let dropped = Cell::new(0);
    let mut storage: [MaybeUninit<Value<'_>>; 3] = [const { MaybeUninit::uninit() }; 3];
    let ptr = NonNull::from(&mut storage).cast::<Value<'_>>();
    let result = unsafe {
        initialize(ptr, length, &mut |index, slot| {
            if index == fail_at && mode == 0 {
                return Err(19u8);
            }
            if index == fail_at && mode == 1 {
                return Ok(());
            }
            assert!(
                slot.write(Value {
                    id: index as u8,
                    dropped: &dropped
                })
                .is_ok()
            );
            if index == fail_at {
                return Err(19u8);
            }
            Ok(())
        })
    };
    let completed = if fail_at < length {
        assert!(result.is_err());
        if mode == 1 {
            assert!(matches!(
                result,
                Err(InitError::Storage(Error::Uninitialized))
            ));
        } else {
            assert!(matches!(result, Err(InitError::Initialize(19))));
        }
        fail_at + usize::from(mode == 2)
    } else {
        assert!(result.is_ok() && dropped.get() == 0);
        for i in 0..length {
            assert!(unsafe { ptr.as_ptr().add(i).as_ref().unwrap() }.id == i as u8);
        }
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(ptr.as_ptr(), length));
        }
        length
    };
    assert!(dropped.get() == (1u8 << completed) - 1);
}
