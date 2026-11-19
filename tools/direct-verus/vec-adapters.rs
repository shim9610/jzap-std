// Compatibility names for caller contracts; every body calls product Rust.
use vstd::prelude::*;
verus! {
fn needed_length(len: usize, additional: usize, limit: usize) -> (r: Result<usize, ()>)
    ensures match r {
        Ok(n) => n as int == len as int + additional as int && n <= limit,
        Err(_) => len as int + additional as int > usize::MAX || len as int + additional as int > limit,
    },
{ crate::vec::required_len(len, additional, limit).map_err(|_| ()) }
fn growth(current: usize, needed: usize, ceiling: usize, minimum: usize) -> (n: usize)
    requires needed <= ceiling, current <= ceiling,
    ensures needed <= n <= ceiling,
{ crate::vec::growth_capacity(current, needed, ceiling, minimum) }
fn normalized_range(start: usize, end: usize, length: usize) -> (ok: bool)
    ensures ok == (start <= end && end <= length),
{ crate::slice::geometry::range(start, end, length).is_ok() }
fn flatten_owner(len: usize, cap: usize, limit: usize, size: usize, factor: usize)
    -> (r: Option<(usize, usize)>)
    requires len <= cap <= limit,
        cap as int * factor as int * size as int <= isize::MAX as int,
    ensures match r {
        Some((n, c)) => n <= c <= limit && c as int * size as int <= isize::MAX as int
            && n as int == len as int * factor as int
            && c as int * size as int == cap as int * factor as int * size as int,
        None => true,
    },
{ crate::vec::geometry::flatten(len, cap, limit, size, factor) }
fn owner_remaining_limit(len: usize, cap: usize, limit: usize) -> (out: usize)
    requires len <= cap <= limit,
    ensures out as int == limit as int - len as int,
{ crate::arithmetic::bounded::wrapping_sub(limit, len) }
fn owner_spare_capacity(len: usize, cap: usize) -> (out: usize)
    requires len <= cap,
    ensures out as int == cap as int - len as int,
{ crate::arithmetic::bounded::wrapping_sub(cap, len) }
// The executable staging controller admits the final extent of each batch.
fn staged_collection_extent(limit: usize, size: usize, initial_cap: usize, requested: usize)
    -> (r: (usize, usize))
    requires initial_cap <= limit,
        crate::vec::staging_driver::byte_valid(initial_cap, size),
    ensures r.0 <= r.1 <= limit,
        crate::vec::staging_driver::byte_valid(r.1, size),
{
    let (_, len, cap) = crate::vec::staging_driver::run(
        (), limit, size, initial_cap,
        |len: usize, cap: usize, _value: ()| -> (Result<Option<()>, ()>, usize, usize)
            requires len < limit,
        {
            let next = crate::arithmetic::bounded::wrapping_add(len, 1);
            (Ok(if next < requested { Some(()) } else { None }), next, cap)
        },
    );
    (len, cap)
}

}
