// Calls to the runtime numeric operations, without an executable model copy.
use vstd::prelude::*;
verus! {
fn checked_array_bytes(count: usize, size: usize) -> (r: Option<usize>)
    ensures match r {
        Some(bytes) => bytes as int == count as int * size as int && bytes <= isize::MAX
            && (size == 0 || count as int <= isize::MAX as int / size as int),
        None => count as int * size as int > isize::MAX,
    },
{ crate::layout::numeric::array_bytes(count, size).ok() }
fn capacity_bytes(cap: usize, size: usize) -> (bytes: usize)
    requires cap as int * size as int <= isize::MAX as int,
    ensures bytes as int == cap as int * size as int, bytes <= isize::MAX,
{ crate::arithmetic::bounded::wrapping_mul(cap, size) }
}
