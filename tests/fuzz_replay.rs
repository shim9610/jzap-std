//! Runs every libFuzzer target body on generated inputs as an ordinary test.
//!
//! The fuzz targets compare jzap with the standard containers after each
//! decoded operation. libFuzzer runs them natively; this file compiles the same
//! bodies, without copying them, so that Miri can check the pointer accesses on
//! the operation sequences they decode. The inputs are deterministic
//! pseudo-random bytes, not a coverage-guided corpus: a corpus found by
//! libFuzzer is not stored in the repository.
#![allow(unused_attributes, unused_imports)]
extern crate self as libfuzzer_sys;

/// Stands in for `libfuzzer_sys::fuzz_target!` and exposes the body as `fuzz_entry`.
#[macro_export]
macro_rules! fuzz_target {
    (|$data:ident : &[u8]| $body:block) => {
        pub fn fuzz_entry($data: &[u8]) $body
    };
}

macro_rules! targets {
    ($($name:ident = $path:literal),* $(,)?) => {$(
        #[path = $path]
        mod $name;
        #[test]
        fn $name() {
            replay($name::fuzz_entry)
        }
    )*};
}
targets!(
    binary_heap = "../fuzz/fuzz_targets/binary_heap.rs",
    btree = "../fuzz/fuzz_targets/btree.rs",
    btree_set = "../fuzz/fuzz_targets/btree_set.rs",
    deque = "../fuzz/fuzz_targets/deque.rs",
    foundation = "../fuzz/fuzz_targets/foundation.rs",
    hash_map = "../fuzz/fuzz_targets/hash_map.rs",
    hash_set = "../fuzz/fuzz_targets/hash_set.rs",
    scratch_slice = "../fuzz/fuzz_targets/scratch_slice.rs",
    shared = "../fuzz/fuzz_targets/shared.rs",
    string = "../fuzz/fuzz_targets/string.rs",
    vec = "../fuzz/fuzz_targets/vec.rs",
);

fn bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

fn replay(run: fn(&[u8])) {
    let (seeds, lengths): (u64, &[usize]) = if cfg!(miri) {
        (4, &[0, 48, 192])
    } else {
        (64, &[0, 1, 7, 48, 192, 768, 3072])
    };
    for seed in 0..seeds {
        for len in lengths {
            let input = bytes(seed * 31 + *len as u64, *len);
            run(&input);
            // Small opcodes reach the growth and removal arms more often.
            let narrow: Vec<u8> = input.iter().map(|b| b & 0x1f).collect();
            run(&narrow);
        }
    }
}
