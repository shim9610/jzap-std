// Deliberately invalid: allocated bytes have not been initialized.
#[kani::proof]
fn uninitialized_read() {
    let layout = std::alloc::Layout::new::<u64>();
    let pointer = unsafe { std::alloc::alloc(layout) }.cast::<u64>();
    if pointer.is_null() { return; }
    std::hint::black_box(unsafe { pointer.read() });
    unsafe { std::alloc::dealloc(pointer.cast(), layout); }
}
