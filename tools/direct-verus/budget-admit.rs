//! Check actual product admission and its actual error type, unchanged.
#![feature(proc_macro_hygiene)]
#[path = "../../src/error.rs"]
mod error;
pub use error::Error;
#[path = "../../src/budget/admission.rs"]
mod admission;
