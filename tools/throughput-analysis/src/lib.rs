//! Stable entry points for inspecting the actual public String API code generation.
//! Emit assembly/LLVM only under ignored build output directories.
use jzap::{Error, string::String as JString};
#[unsafe(no_mangle)]
pub fn jzap_string_append(owner: &mut JString, text: &str) -> Result<(), Error> {
    owner.try_push_str(text)
}
#[unsafe(no_mangle)]
pub fn std_string_append(owner: &mut String, text: &str) {
    owner.push_str(text)
}
#[unsafe(no_mangle)]
pub fn jzap_string_retain(owner: &mut JString) {
    owner.retain(|c| c != 'x' && c != '한')
}
#[unsafe(no_mangle)]
pub fn std_string_retain(owner: &mut String) {
    owner.retain(|c| c != 'x' && c != '한')
}
#[unsafe(no_mangle)]
pub fn jzap_string_pop(owner: &mut JString) -> Option<char> {
    owner.pop()
}
#[unsafe(no_mangle)]
pub fn std_string_pop(owner: &mut String) -> Option<char> {
    owner.pop()
}
#[unsafe(no_mangle)]
pub fn jzap_string_replace(
    owner: &mut JString,
    start: usize,
    end: usize,
    text: &str,
) -> Result<(), Error> {
    owner.try_replace_range(start..end, text)
}
#[unsafe(no_mangle)]
pub fn std_string_replace(owner: &mut String, start: usize, end: usize, text: &str) {
    owner.replace_range(start..end, text)
}
#[unsafe(no_mangle)]
pub fn jzap_string_drain(owner: &mut JString, start: usize, end: usize) -> Result<(), Error> {
    for ch in owner.try_drain(start..end)? {
        std::hint::black_box(ch);
    }
    Ok(())
}
#[unsafe(no_mangle)]
pub fn std_string_drain(owner: &mut String, start: usize, end: usize) {
    for ch in owner.drain(start..end) {
        std::hint::black_box(ch);
    }
}

#[inline(never)]
pub fn std_rc_handle_clone(value: &std::rc::Rc<u64>) -> std::rc::Rc<u64> {
    value.clone()
}
#[inline(never)]
pub fn jzap_rc_handle_clone(value: &jzap::rc::Rc<u64>) -> jzap::rc::Rc<u64> {
    jzap::rc::Rc::try_clone(value).unwrap()
}
#[inline(never)]
pub fn std_arc_handle_clone(value: &std::sync::Arc<u64>) -> std::sync::Arc<u64> {
    value.clone()
}
#[inline(never)]
pub fn jzap_arc_handle_clone(value: &jzap::sync::Arc<u64>) -> jzap::sync::Arc<u64> {
    jzap::sync::Arc::try_clone(value).unwrap()
}
#[inline(never)]
pub fn std_arc_upgrade(value: &std::sync::Weak<u64>) -> Option<std::sync::Arc<u64>> {
    value.upgrade()
}
#[inline(never)]
pub fn jzap_arc_upgrade(value: &jzap::sync::Weak<u64>) -> Option<jzap::sync::Arc<u64>> {
    value.try_upgrade().unwrap()
}
#[inline(never)]
pub fn std_arc_construct(value: u64) -> std::sync::Arc<u64> {
    std::sync::Arc::new(value)
}
#[inline(never)]
pub fn jzap_arc_construct(value: u64) -> jzap::sync::Arc<u64> {
    jzap::sync::Arc::try_new(value).unwrap()
}
#[inline(never)]
pub fn std_arc_cow(value: &mut std::sync::Arc<u64>) -> &mut u64 {
    std::sync::Arc::make_mut(value)
}
#[inline(never)]
pub fn jzap_arc_cow(value: &mut jzap::sync::Arc<u64>) -> &mut u64 {
    jzap::sync::Arc::try_make_mut(value).unwrap()
}

#[inline(never)]
pub fn std_rc_slice_clone(source: &[u64]) -> std::rc::Rc<[u64]> {
    source.into()
}
#[inline(never)]
pub fn jzap_rc_slice_clone(source: &[u64]) -> jzap::rc::Rc<[u64]> {
    jzap::rc::Rc::try_from_slice(source).unwrap()
}
#[inline(never)]
pub fn std_arc_slice_cow(source: &mut std::sync::Arc<[u64]>) -> &mut [u64] {
    std::sync::Arc::make_mut(source)
}
#[inline(never)]
pub fn jzap_arc_slice_cow(source: &mut jzap::sync::Arc<[u64]>) -> &mut [u64] {
    jzap::sync::Arc::try_make_mut(source).unwrap()
}
