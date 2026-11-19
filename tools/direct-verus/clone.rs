//! Includes the executable product admission without extraction or rewriting.
#![feature(proc_macro_hygiene)]
#[path = "../../src/error.rs"]
mod error;
pub use error::Error;
#[path = "../../src/clone/geometry.rs"]
mod geometry;
