use crucible::crucible_assert;
#[crux::test]
fn allocation_shim() {
    let layout = std::alloc::Layout::from_size_align(2, 1).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    if !ptr.is_null() {
        crucible_assert!(!ptr.is_null());
        unsafe { std::alloc::dealloc(ptr, layout) };
    }
}
