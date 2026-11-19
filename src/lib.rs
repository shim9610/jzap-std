//! Fallible standard-library counterparts. Experimental: see docs/support.md.
#![no_std]
#![cfg_attr(verus_keep_ghost, feature(proc_macro_hygiene))]
#![deny(clippy::arithmetic_side_effects)]
#![cfg_attr(
    all(kani, jzap_loop_contracts),
    feature(stmt_expr_attributes, proc_macro_hygiene)
)]
#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;
pub mod arithmetic;
#[cfg(feature = "alloc")]
pub mod boxed;
pub mod budget;
mod budget_numeric;
#[cfg(feature = "alloc")]
mod capacity;
pub mod clone;
#[cfg(feature = "alloc")]
pub mod collections;
#[cfg(feature = "alloc")]
mod compaction;
pub mod default;
#[cfg(feature = "alloc")]
pub mod deque;
pub mod error;
#[cfg(feature = "alloc")]
pub mod fmt;
pub mod init;
pub mod iter;
pub mod layout;
#[cfg(feature = "alloc")]
pub mod rc;
pub mod scratch;
#[cfg(feature = "alloc")]
#[doc(hidden)]
pub mod shared;
pub mod slice;
#[cfg(feature = "alloc")]
mod span;
#[cfg(feature = "alloc")]
mod storage;
#[cfg(feature = "alloc")]
pub mod string;
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
pub mod sync;
#[cfg(feature = "alloc")]
pub mod vec;
pub use arithmetic::{
    ArithmeticError, Checked, DivisionError, DivisionError64, TryArithmetic, WrappingArithmetic,
};
pub use clone::{CopyWitness, TryClone, TryCloneIn};
pub use default::TryDefault;
pub use error::{Error, Rejected};
pub use iter::{
    TryDoubleEndedIterator, TryExactSizeIterator, TryFusedIterator, TryIntoIterator, TryIterator,
};
#[cfg(all(kani, jzap_loop_contracts))]
mod panic_proofs;
#[cfg(kani)]
mod proofs;
#[cfg(all(verus_keep_ghost, jzap_proof_composition))]
mod proof_composition {
    include!(env!("JZAP_PROOF_COMPOSITION"));
}
#[cfg(all(kani, feature = "alloc"))]
mod wrapping_memory_proofs;
