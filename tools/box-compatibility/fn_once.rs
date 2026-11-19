mod custom;
use custom::Custom;
fn invoke_once(f: impl FnOnce() -> usize) -> usize { f() }
fn main() {
    let text = String::from("once");
    let callback = Custom(Box::new(move || { let owned = text; owned.len() }));
    assert_eq!(invoke_once(callback), 4);
}
