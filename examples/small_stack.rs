//! Run as an isolated process: stack-overflow regressions must fail the command.
use jzap::{
    TryClone, TryCloneIn, TryDefault,
    boxed::Box,
    budget::{Budget, MemoryBudget, Unmetered},
};
fn main() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            let source = Box::<[[u64; 256]; 128]>::try_init_array_in(Unmetered, |row, slot| {
                slot.try_init_array(|col, element| {
                    element.write((row * 256 + col) as u64).map_err(|e| e.error)
                })
            })
            .unwrap();
            let mut copy = source.try_clone().unwrap();
            copy.try_clone_from(&source).unwrap();
            let budget = MemoryBudget::new(256 * 1024);
            let metered = source.try_clone_in(Budget::Limited(&budget)).unwrap();
            assert_eq!(budget.occupied(), 256 * 1024);
            for (a, (b, c)) in source.iter().zip(copy.iter().zip(metered.iter())) {
                assert_eq!(a, b);
                assert_eq!(a, c);
            }
            struct Large {
                data: [[u64; 256]; 128],
                tail: u64,
            }
            let large = Box::<Large>::try_init_in(
                Unmetered,
                |slot: &mut jzap::init::InitSlot<'_, Large>| {
                    jzap::try_init_struct!(
                        slot,
                        Large {
                            data: <[[u64; 256]; 128]>::try_default_into,
                            tail: |slot: &mut jzap::init::InitSlot<'_, u64>| slot
                                .write(7)
                                .map_err(|e| e.error),
                        }
                    )
                },
            )
            .unwrap();
            assert_eq!(large.tail, 7);
            assert!(large.data.iter().flatten().all(|v| *v == 0));
            let defaults = Box::<[[u64; 256]; 128]>::try_default().unwrap();
            assert!(defaults.iter().flatten().all(|v| *v == 0));
            let dynamic = Box::<[u64]>::try_init_slice(32768, |i, slot| {
                slot.write(i as u64).map_err(|e| e.error)
            })
            .unwrap();
            assert_eq!(*dynamic.get(32767).unwrap(), 32767);
            drop(metered);
            assert_eq!(budget.occupied(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}
