//! Include original product bodies without a text projection.
#![feature(proc_macro_hygiene)]
#[path = "../../src/arithmetic/bounded.rs"]
pub mod bounded;
mod arithmetic { pub use crate::bounded; }
#[path = "../../src/collections/binary_heap/geometry.rs"]
mod geometry;
