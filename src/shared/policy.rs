//! Sealed accounting policies, stored once in the shared allocation.
use crate::budget::AllocationPolicy;
/// Shared allocations accept the same sealed policies as Box/Vec/String.
/// Handle cloning preserves this policy and never duplicates its charge.
pub trait SharedPolicy: AllocationPolicy {}
impl<P: AllocationPolicy> SharedPolicy for P {}
