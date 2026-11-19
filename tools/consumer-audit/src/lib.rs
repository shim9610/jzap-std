//! Test-only runtime for concrete user-defined iterator implementations.
#![no_std]
use core::alloc::{GlobalAlloc, Layout};
unsafe extern "C" {
    fn audit_alloc(size: usize, align: usize) -> *mut u8;
    fn audit_free(ptr: *mut u8);
    fn audit_realloc(ptr: *mut u8, old_size: usize, align: usize, size: usize) -> *mut u8;
    fn jzap_link_audit_panic_reachable() -> !;
}
struct Allocator;
// SAFETY: the supplied C fixture implements matching allocation and null refusal.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { audit_alloc(layout.size(), layout.align()) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _: Layout) {
        unsafe { audit_free(ptr) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        unsafe { audit_realloc(ptr, layout.size(), layout.align(), size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    // Undefined by design. This test runtime is never linked into the product.
    unsafe { jzap_link_audit_panic_reachable() }
}
mod consumer {
    include!(env!("JZAP_CONSUMER_SOURCE"));
}
#[unsafe(no_mangle)]
pub extern "C" fn consumer_audit(seed: usize) -> u32 {
    match consumer::run(core::hint::black_box(seed)) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
