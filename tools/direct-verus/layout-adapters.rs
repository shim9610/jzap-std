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
// FixedTree branch position: try_new stores every branch after the leaves,
// and the descent invariants in tools/consumer-audit/bplus_fixed.rs show that
// get reads a branch only for an ID at or above the leaf count.
fn fixed_branch_offset(id: usize, leaves: usize) -> (offset: usize)
    requires leaves <= id,
    ensures offset as int == id as int - leaves as int, offset <= id,
{ crate::arithmetic::bounded::wrapping_sub(id, leaves) }
}
