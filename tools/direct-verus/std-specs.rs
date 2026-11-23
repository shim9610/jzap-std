//! Verus specifications for Rust 1.95 core functions that vstd does not specify.
//!
//! Like vstd's own `std_specs`, each entry is an `assume_specification`: the
//! verifier trusts it as the documented behaviour of the pinned core function.
//! The product crate is verified with `--no-cheating`, which rejects such
//! entries inside it, so they live in this separate crate and are imported.
//! `scripts/verus_std_specs.py` checks every entry against the installed
//! Rust 1.95 core source before building this crate.
use vstd::prelude::*;

verus! {

// library/core/src/hint.rs, `select_unpredictable`: "This method is
// functionally equivalent to `fn select_unpredictable<T>(b: bool, true_val:
// T, false_val: T) -> T { if b { true_val } else { false_val } }` but might
// generate different assembly."
pub assume_specification<T>[ core::hint::select_unpredictable::<T> ](
    condition: bool,
    true_val: T,
    false_val: T,
) -> (r: T)
    ensures
        r == if condition { true_val } else { false_val },
;

} // verus!
