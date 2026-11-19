#![cfg_attr(box_nightly, feature(fn_traits, unboxed_closures))]
mod custom;
use custom::Custom;
impl<F: FnOnce() -> usize> FnOnce<()> for Custom<F> {
    type Output = usize;
    extern "rust-call" fn call_once(self, _: ()) -> usize { (self.0)() }
}
fn invoke_once(f: impl FnOnce() -> usize) -> usize { f() }
fn main() {
    let text = String::from("once");
    let callback = Custom(Box::new(move || { let owned = text; owned.len() }));
    assert_eq!(invoke_once(callback), 4);
}
