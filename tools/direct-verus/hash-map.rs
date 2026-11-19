//! Original product modules, unchanged: no Python body extraction.
#![feature(proc_macro_hygiene)]
#[path = "../../src/arithmetic/bounded.rs"]
pub mod bounded;
mod arithmetic { pub use crate::bounded; }
#[path = "../../src/error.rs"]
mod error;
pub use error::Error;
#[path = "../../src/collections/hash_map/geometry.rs"]
mod geometry;
