#![cfg_attr(box_nightly, feature(coerce_unsized, unsize))]
mod custom;
use custom::Custom;
use std::{marker::Unsize, ops::CoerceUnsized};
impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Custom<U>> for Custom<T> {}
fn main() {
    let concrete = Custom(Box::new(42_u64));
    let erased: Custom<dyn std::fmt::Display> = concrete;
    assert_eq!(erased.to_string(), "42");
}
