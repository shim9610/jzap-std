//! Direct checks of the actual slot ownership handoff.
use super::*;
use core::{cell::Cell, mem::MaybeUninit};

#[kani::proof]
fn finish_transfers_exactly_one_owned_value() {
    struct Value<'a>(&'a Cell<usize>);
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Cell::new(0usize);
    let mut storage = MaybeUninit::<Value<'_>>::uninit();
    // SAFETY: uniquely borrowed aligned local storage, initially unwritten.
    let mut slot = unsafe { InitSlot::new(NonNull::from(&mut storage).cast()) };
    let write: bool = kani::any();
    if write {
        assert!(slot.write(Value(&drops)).is_ok());
    }
    let result = slot.finish();
    assert!(result.is_ok() == write);
    assert!(drops.get() == 0);
    if write {
        // SAFETY: successful finish transferred the initialized value to storage.
        drop(unsafe { storage.assume_init() });
    }
    assert!(drops.get() == usize::from(write));
}
