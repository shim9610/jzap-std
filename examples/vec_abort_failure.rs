use jzap::{Error, vec::Vec};
#[path = "../tests/common/mod.rs"]
mod common;
#[global_allocator]
static A: common::FaultAllocator = common::FaultAllocator;
fn main() {
    let ((), s) = common::track(1, || {
        let mut v = Vec::<u64>::with_limit(4);
        let e = v.try_push(123).unwrap_err();
        assert!(matches!(e.error, Error::Allocation { .. }));
        assert_eq!(e.value, 123);
        assert!(v.is_empty());
    });
    assert_eq!(s.live, 0);
    let mut v = Vec::<u8>::with_limit(1);
    v.try_push(7).unwrap();
    assert_eq!(v.try_push(8).unwrap_err().error, Error::Capacity);
    assert_eq!(v.as_slice(), &[7]);
    assert_eq!(
        v.try_splice(.., [1, 2]).unwrap().finish(),
        Err(Error::Capacity)
    );
    assert_eq!(v.as_slice(), &[1]);
    assert!(v.try_drain(2..).is_err());
    assert_eq!(v.try_retain(|_| Err::<bool, _>(7)), Err(7));
    let ((), stats) = common::track(usize::MAX, || {
        let mut x = Vec::try_from_array([1u64, 2, 3]).unwrap();
        common::STATE.with(|state| {
            let mut s = state.get();
            s.fail = s.calls + 1;
            state.set(s);
        });
        assert!(x.try_splice(1..1, [9]).unwrap().finish().is_err());
        assert_eq!(x.as_slice(), [1, 2, 3]);
    });
    assert_eq!(stats.live, 0);
}
