//! Root of the VeriFast subset crate for `Capacity` (not a product file).
//! It declares only the product modules that `Capacity` needs; both the
//! original and the verified subset crate use this root unchanged.
#![no_std]
pub mod arithmetic;
mod capacity;
pub mod error;
pub mod layout;
pub use arithmetic::{
    ArithmeticError, Checked, DivisionError, DivisionError64, TryArithmetic, WrappingArithmetic,
};
pub use error::{Error, Rejected};
