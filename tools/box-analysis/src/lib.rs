#![cfg_attr(feature = "nightly", feature(allocator_ext))]
use jzap::{
    TryClone,
    boxed::Box as JBox,
    budget::{Budget, MemoryBudget},
};
#[unsafe(no_mangle)]
#[inline(never)]
pub fn std_new(x: u64) -> Box<u64> {
    Box::new(x)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn varlit_new(x: u64) -> varlit_core::Result<Box<u64>> {
    varlit_core::try_box(x)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_new(x: u64) -> Result<JBox<u64>, jzap::Rejected<u64>> {
    JBox::try_new(x)
}
#[unsafe(no_mangle)]
#[inline(never)]
#[expect(
    clippy::borrowed_box,
    reason = "Measure Box clone itself, including its allocation."
)]
pub fn std_clone(x: &Box<u64>) -> Box<u64> {
    x.clone()
}
#[unsafe(no_mangle)]
#[inline(never)]
#[expect(
    clippy::borrowed_box,
    reason = "Measure Box clone itself, including its allocation."
)]
pub fn varlit_clone(x: &Box<u64>) -> varlit_core::Result<Box<u64>> {
    varlit_core::TryClone::try_clone(x)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_clone(x: &JBox<u64>) -> Result<JBox<u64>, jzap::Error> {
    x.try_clone()
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn std_drop(x: Box<u64>) {
    drop(x)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn varlit_drop(x: Box<u64>) {
    drop(x)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_drop(x: JBox<u64>) {
    drop(x)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_metered_new(
    x: u64,
    b: &MemoryBudget,
) -> Result<JBox<u64, Budget<'_>>, jzap::Rejected<u64>> {
    JBox::try_new_in(x, Budget::Limited(b))
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_metered_drop(x: JBox<u64, Budget<'_>>) {
    drop(x)
}
// Normalize errors to Option to distinguish payload operations from public error ABI.
#[unsafe(no_mangle)]
#[inline(never)]
pub fn varlit_new_option(x: u64) -> Option<Box<u64>> {
    varlit_core::try_box(x).ok()
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_new_option(x: u64) -> Option<JBox<u64>> {
    JBox::try_new(x).ok()
}
#[unsafe(no_mangle)]
#[inline(never)]
#[expect(
    clippy::borrowed_box,
    reason = "Measure Box clone itself, including its allocation."
)]
pub fn varlit_clone_option(x: &Box<u64>) -> Option<Box<u64>> {
    varlit_core::TryClone::try_clone(x).ok()
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_clone_option(x: &JBox<u64>) -> Option<JBox<u64>> {
    x.try_clone().ok()
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn std_copy(a: &[u8], b: &mut [u8]) {
    b.copy_from_slice(a)
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_copy(a: &[u8], b: &mut [u8]) -> Result<(), jzap::Error> {
    jzap::clone::try_copy_slice(a, b)
}

#[unsafe(no_mangle)]
#[inline(never)]
pub fn std_new_4k(x: u8) -> Box<[u8; 4096]> {
    Box::new([x; 4096])
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn varlit_new_4k(x: u8) -> varlit_core::Result<Box<[u8; 4096]>> {
    varlit_core::try_box_with(|| Ok([x; 4096]))
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_new_4k(x: u8) -> Result<JBox<[u8; 4096]>, jzap::error::InitError<()>> {
    JBox::try_new_with_in(jzap::budget::Unmetered, || Ok([x; 4096]))
}
#[cfg(feature = "nightly")]
#[unsafe(no_mangle)]
#[inline(never)]
pub fn nightly_try_new(x: u64) -> Result<Box<u64>, std::alloc::AllocError> {
    Box::try_new(x)
}
#[cfg(feature = "nightly")]
#[unsafe(no_mangle)]
#[inline(never)]
pub fn nightly_try_new_4k(x: u8) -> Result<Box<[u8; 4096]>, std::alloc::AllocError> {
    let storage = Box::try_new_uninit()?;
    Ok(Box::write(storage, [x; 4096]))
}

#[unsafe(no_mangle)]
#[inline(never)]
#[expect(
    clippy::borrowed_box,
    reason = "Measure Box clone itself, including its allocation."
)]
pub fn std_clone_array_4k(x: &Box<[u8; 4096]>) -> Box<[u8; 4096]> {
    x.clone()
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_clone_array_4k(x: &JBox<[u8; 4096]>) -> Result<JBox<[u8; 4096]>, jzap::Error> {
    x.try_clone()
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_init_array_4k(x: u8) -> Result<JBox<[u8; 4096]>, jzap::error::InitError<jzap::Error>> {
    JBox::try_init_array_in(jzap::budget::Unmetered, |_, slot| {
        slot.write(x).map_err(|e| e.error)
    })
}

#[unsafe(no_mangle)]
#[inline(never)]
pub fn std_collect_bytes(input: &[u8]) -> Box<[u8]> {
    input.iter().copied().collect()
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_collect_bytes(input: &[u8]) -> Result<JBox<[u8]>, jzap::Error> {
    JBox::<[u8]>::try_from_iter(input.iter().copied())
}

// Exercise the actual checked layout helper, including a non-power-of-two
// element size. Equality of emitted wrappers guards its preflight fast path.
macro_rules! layout_pair {
    ($standard:ident, $fallible:ident, $ty:ty) => {
        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $standard(count: usize) -> Result<core::alloc::Layout, jzap::Error> {
            core::alloc::Layout::array::<$ty>(count).map_err(|_| jzap::Error::Layout)
        }
        #[unsafe(no_mangle)]
        #[inline(never)]
        pub fn $fallible(count: usize) -> Result<core::alloc::Layout, jzap::Error> {
            jzap::layout::array::<$ty>(count)
        }
    };
}
layout_pair!(std_layout_u8, jzap_layout_u8, u8);
layout_pair!(std_layout_u64, jzap_layout_u64, u64);
layout_pair!(std_layout_three, jzap_layout_three, [u8; 3]);
layout_pair!(std_layout_unit, jzap_layout_unit, ());

// Same fallible input and wrapping reduction; the open trait must not add work
// merely because its failure channel is separate from its item type.
fn checked_iteration_item(value: u64) -> Result<u64, u8> {
    if value == u64::MAX { Err(1) } else { Ok(value) }
}
struct IterationSource<'a>(core::slice::Iter<'a, u64>);
impl jzap::TryIterator for IterationSource<'_> {
    type Item = u64;
    type Error = u8;
    fn try_next(&mut self) -> Result<Option<u64>, u8> {
        self.0.next().copied().map(checked_iteration_item).transpose()
    }
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn std_iteration(input: &[u64]) -> Result<u64, u8> {
    input.iter().copied().map(checked_iteration_item)
        .try_fold(0u64, |sum, item| Ok(sum.wrapping_add(item?)))
}
#[unsafe(no_mangle)]
#[inline(never)]
pub fn jzap_iteration(input: &[u64]) -> Result<u64, u8> {
    use jzap::{TryIterator, iter::TryFoldError};
    IterationSource(input.iter()).try_fold(0u64, |sum, item| {
        Ok::<_, core::convert::Infallible>(sum.wrapping_add(item))
    }).map_err(|error| match error {
        TryFoldError::Source(error) => error,
        TryFoldError::Operation(never) => match never {},
    })
}
