//! Direct original product bodies; no generated source projection.
#![feature(proc_macro_hygiene)]
#[path = "../../src/arithmetic/bounded.rs"]
pub mod bounded;
mod arithmetic { pub use crate::bounded; }
#[path = "../../src/deque/geometry.rs"]
mod geometry;
