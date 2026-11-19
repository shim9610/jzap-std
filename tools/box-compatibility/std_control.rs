fn invoke_once(f: impl FnOnce() -> usize) -> usize { f() }
fn main() {
    let value = Box::new(String::from("owned"));
    let moved: String = *value;
    assert_eq!(moved, "owned");
    let concrete = Box::new(42_u64);
    let erased: Box<dyn std::fmt::Display> = concrete;
    assert_eq!(erased.to_string(), "42");
    let text = String::from("once");
    let callback = Box::new(move || { let owned = text; owned.len() });
    assert_eq!(invoke_once(callback), 4);
}
