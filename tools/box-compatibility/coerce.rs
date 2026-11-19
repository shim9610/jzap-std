mod custom;
use custom::Custom;
fn main() {
    let concrete = Custom(Box::new(42_u64));
    let erased: Custom<dyn std::fmt::Display> = concrete;
    assert_eq!(erased.to_string(), "42");
}
