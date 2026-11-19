//! Actual nested-array trait adapter, including zero-byte word-sized extents.
use super::*;

fn array_reuse<const WIDTH: usize>(source_len: usize, destination_len: usize) {
    // SAFETY: unit arrays occupy zero bytes and every unit is initialized.
    // Distinct live local objects provide valid aligned pointers for the borrows.
    let (mut source_backing, mut destination_backing) = (0u8, 0u8);
    let source = unsafe {
        core::slice::from_raw_parts(
            core::ptr::NonNull::from(&mut source_backing)
                .cast::<[(); WIDTH]>()
                .as_ptr(),
            source_len,
        )
    };
    let destination = unsafe {
        core::slice::from_raw_parts_mut(
            core::ptr::NonNull::from(&mut destination_backing)
                .cast::<[(); WIDTH]>()
                .as_ptr(),
            destination_len,
        )
    };
    let expected = if source_len != destination_len {
        Err(Error::Bounds)
    } else {
        source_len
            .checked_mul(WIDTH)
            .map(|_| ())
            .ok_or(Error::Capacity)
    };
    assert!(<[(); WIDTH]>::try_copy_slice_from(source, destination) == Some(expected));
}

#[kani::proof]
fn nested_array_adapter_checks_arbitrary_word_lengths() {
    let source_len: usize = kani::any();
    let destination_len: usize = kani::any();
    array_reuse::<0>(source_len, destination_len);
    array_reuse::<1>(source_len, destination_len);
    array_reuse::<3>(source_len, destination_len);
    array_reuse::<{ usize::MAX }>(source_len, destination_len);
}
