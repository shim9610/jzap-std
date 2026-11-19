use std::{cell::Cell, mem::MaybeUninit};
struct Value<'a> { payload: u64, drops: &'a Cell<usize> }
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(not(kani), test)]
fn initialized_reference_write() {
    let count = Cell::new(0usize);
    let value = Value { payload: 7, drops: &count };
    let mut storage = MaybeUninit::<Value<'_>>::uninit();
    let result = storage.write(value);
    assert!(result.payload == 7);
    assert!(result.drops.get() == 0);
}
#[cfg_attr(kani, kani::proof)]
#[cfg_attr(not(kani), test)]
fn raw_reference_write() {
    let count = Cell::new(0usize);
    let value = Value { payload: 7, drops: &count };
    let mut storage = MaybeUninit::<Value<'_>>::uninit();
    // SAFETY: aligned writable storage, exclusive access, initializing once.
    unsafe { storage.as_mut_ptr().write(value); }
    // SAFETY: the write above initialized the whole value.
    let result = unsafe { storage.assume_init() };
    assert!(result.payload == 7);
    assert!(result.drops.get() == 0);
}

#[cfg_attr(kani, kani::proof)]
#[cfg_attr(not(kani), test)]
fn initialized_spare_slice() {
    let mut storage = [MaybeUninit::<u8>::uninit(); 2];
    // SAFETY: aligned, exclusive array storage; MaybeUninit permits unwritten bytes.
    let spare = unsafe { std::slice::from_raw_parts_mut(storage.as_mut_ptr(), 2) };
    for slot in spare { slot.write(7); }
    // SAFETY: every array slot was initialized above.
    assert!(unsafe { storage[0].assume_init() } == 7);
}
