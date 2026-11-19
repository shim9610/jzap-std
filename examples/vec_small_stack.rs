use jzap::{TryClone, vec::Vec};
const N: usize = 256 * 1024;
fn main() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            let mut v = Vec::<[u8; N]>::with_limit(2);
            v.try_resize_with_init(2, |_, slot| {
                slot.try_init_array(|i, element| {
                    element.write((i % 251) as u8).map_err(|e| e.error)
                })
            })
            .unwrap();
            let copy = v.try_clone().unwrap();
            assert_eq!(copy.as_slice(), v.as_slice());
            let mut target = Vec::<[u8; N]>::with_limit(2);
            target.try_clone_from(&v).unwrap();
            assert_eq!(target.as_slice(), v.as_slice());
            drop(target.try_drain(0..1).unwrap());
            target.try_extend_from_within(..).unwrap();
            target.retain(|_| true);
            target.dedup_by(|a, b| a == b);
            assert_eq!(target.len(), 1);
        })
        .unwrap()
        .join()
        .unwrap();
}
