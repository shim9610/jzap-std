//! All diagnostics are fixed-size and allocation-free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, vstd::prelude::verus_verify)]
pub enum Error {
    Allocation {
        size: usize,
        align: usize,
    },
    Layout,
    Budget {
        requested: usize,
        available: usize,
    },
    Capacity,
    RefCount,
    /// OS entropy refusal, or code zero when no default provider is available.
    Entropy {
        code: i32,
    },
    Bounds,
    CharBoundary,
    InvalidStructure,
    Uninitialized,
    AlreadyInitialized,
}
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Allocation { .. } => "allocation refused",
            Self::Layout => "invalid allocation layout",
            Self::Budget { .. } => "memory budget exceeded",
            Self::Capacity => "capacity exceeded",
            Self::Entropy { .. } => "entropy source unavailable",
            Self::RefCount => "reference count limit exceeded",
            Self::Bounds => "invalid range",
            Self::CharBoundary => "invalid UTF-8 character boundary",
            Self::InvalidStructure => "invalid tree structure",
            Self::Uninitialized => "initializer did not write a value",
            Self::AlreadyInitialized => "slot already initialized",
        })
    }
}
impl core::error::Error for Error {}
#[derive(Debug, PartialEq, Eq)]
pub struct Rejected<T, E = Error> {
    pub value: T,
    pub error: E,
}
/// Preserve an initializer's own error without boxing it.
#[derive(Debug, PartialEq, Eq)]
pub enum InitError<E> {
    Storage(Error),
    Initialize(E),
}
impl<E> From<Error> for InitError<E> {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}
