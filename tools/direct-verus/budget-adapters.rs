// Actual numeric calls for remaining caller contracts. No product body copy.
use vstd::prelude::*;
verus! {
fn reserved(limit: usize, occupied: usize, available: usize) -> (r: usize)
    requires occupied as int + available as int <= limit as int,
    ensures r as int == limit as int - available as int - occupied as int,
{ crate::budget_numeric::pending(limit, occupied, available) }

fn reserve(limit: usize, occupied: usize, available: usize, bytes: usize) -> (r: Option<usize>)
    requires occupied as int + available as int <= limit as int,
    ensures r.is_some() <==> bytes <= available,
        r.is_some() ==> r.unwrap() as int == available as int - bytes as int,
        r.is_some() ==> occupied as int + r.unwrap() as int <= limit as int,
{
    crate::budget_numeric::reserve(available, bytes).ok()
}
fn commit(limit: usize, occupied: usize, available: usize, bytes: usize) -> (r: usize)
    requires occupied as int + available as int <= limit as int,
        bytes as int <= limit as int - available as int - occupied as int,
    ensures r as int == occupied as int + bytes as int,
        r as int + available as int <= limit as int,
        limit as int - available as int - r as int ==
            limit as int - available as int - occupied as int - bytes as int,
{ crate::budget_numeric::credit(occupied, bytes) }
fn cancel(limit: usize, occupied: usize, available: usize, bytes: usize) -> (r: usize)
    requires occupied as int + available as int <= limit as int,
        bytes as int <= limit as int - available as int - occupied as int,
    ensures r as int == available as int + bytes as int,
        occupied as int + r as int <= limit as int,
{ crate::budget_numeric::credit(available, bytes) }
fn release(limit: usize, occupied: usize, available: usize, bytes: usize) -> (r: (usize, usize))
    requires occupied as int + available as int <= limit as int, bytes <= occupied,
    ensures r.0 as int == occupied as int - bytes as int,
        r.1 as int == available as int + bytes as int,
        r.0 as int + r.1 as int == occupied as int + available as int,
        r.0 as int + r.1 as int <= limit as int,
{
    let new_occupied = crate::budget_numeric::retire(occupied, bytes);
    let new_available = crate::budget_numeric::credit(available, bytes);
    (new_occupied, new_available)
}
}
