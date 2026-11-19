mod custom;
use custom::Custom;
impl<T> Custom<T> {
    fn into_inner(self) -> T { *self.0 }
}
impl<T: std::fmt::Display + 'static> Custom<T> {
    fn into_display(self) -> Custom<dyn std::fmt::Display> { Custom(self.0) }
}
fn invoke_once(f: impl FnOnce() -> usize) -> usize { f() }
fn main() {
    let value = Custom(Box::new(String::from("owned")));
    assert_eq!(value.into_inner(), "owned");
    let erased = Custom(Box::new(42_u64)).into_display();
    assert_eq!(erased.to_string(), "42");
    let text = String::from("once");
    let callback = Custom(Box::new(move || { let owned = text; owned.len() }));
    // The consuming adapter implements FnOnce; Custom itself still does not.
    assert_eq!(invoke_once(move || (callback.into_inner())()), 4);
}
