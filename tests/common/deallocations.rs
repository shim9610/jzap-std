use crate::common::WATCH;

/// Observe this allocation only: panic hooks/runtime may allocate independently.
pub fn deallocations_during<T>(address: usize, f: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            WATCH.with(|w| w.set((0, 0)));
        }
    }
    WATCH.with(|w| {
        assert_eq!(w.get().0, 0);
        w.set((address, 0));
    });
    let reset = Reset;
    let result = f();
    let count = WATCH.with(|w| w.get().1);
    drop(reset);
    (result, count)
}
