//! Explicit consuming callable adaptation without an arity limit or allocation.
use super::*;
#[doc(hidden)]
pub use alloc::boxed::Box as StandardCallOwner;
#[doc(hidden)]
pub struct CallRelease<P: AllocationPolicy> {
    policy: P,
    bytes: usize,
}
impl<P: AllocationPolicy> CallRelease<P> {
    /// # Safety
    /// This guard must own exactly the charge of an allocation transferred to a
    /// standard Box. That allocation must be freed before the guard is dropped.
    #[doc(hidden)]
    pub unsafe fn new(policy: P, bytes: usize) -> Self {
        Self { policy, bytes }
    }
}
impl<P: AllocationPolicy> Drop for CallRelease<P> {
    fn drop(&mut self) {
        self.policy.budget().release(self.bytes);
    }
}
/// Consume a boxed callable with arbitrary arguments, including dyn FnOnce.
/// No allocation is performed. Budget accounting is retained through the call
/// and released after the standard owning-call implementation frees its storage.
#[macro_export]
macro_rules! box_call_once {
    (@argument_type $arg:expr) => { _ };
    ($value:expr; $($arg:expr),* $(,)?) => {{
        let __owner = $value;
        let __bytes = $crate::boxed::Box::allocation_layout(&__owner).size();
        let (__ptr, __policy) = $crate::boxed::Box::into_raw_parts(__owner);
        // SAFETY: the charge is transferred with its allocation; local destruction
        // order frees the standard callable owner before releasing its charge.
        let __release = unsafe { $crate::boxed::CallRelease::new(__policy, __bytes) };
        // Explicit FnOnce selection also works around concrete Box call inference
        // selecting Fn on supported stable Rust. Coercion preserves the allocation.
        let __callable: $crate::boxed::StandardCallOwner<dyn FnOnce($($crate::box_call_once!(@argument_type $arg)),*) -> _ + '_> =
            unsafe { $crate::boxed::StandardCallOwner::from_raw(__ptr.as_ptr()) };
        __callable($($arg),*)
    }};
}
