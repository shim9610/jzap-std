mod custom;
use custom::Custom;
fn main() {
    let value = Custom(Box::new(String::from("owned")));
    let moved: String = *value;
    assert_eq!(moved, "owned");
}
