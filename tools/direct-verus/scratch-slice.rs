//! Includes original product bodies, without extraction or rewriting.
#![feature(proc_macro_hygiene)]
#[path = "../../src/arithmetic/bounded.rs"]
pub mod bounded;
mod arithmetic { pub use crate::bounded; }
#[path = "../../src/error.rs"]
mod error;
pub use error::Error;
#[path = "../../src/slice/geometry.rs"]
mod geometry;
