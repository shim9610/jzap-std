//! Executable excess collection control; the caller owns allocation and writes.
use crate::arithmetic::bounded::wrapping_add;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
verus! {
pub(crate) open spec fn byte_valid(cap: usize, size: usize) -> bool {
    (cap as int) * (size as int) <= isize::MAX as int
}
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires initial_cap <= limit, byte_valid(initial_cap, size),
        forall|i:usize,c:usize,v:T|
            i < limit && i <= c <= limit && byte_valid(c,size)
            ==> call_requires(step,(i,c,v)),
    ensures out.1 <= out.2 <= limit, byte_valid(out.2,size),
))]
pub(crate) fn run<T, E, F>(
    first: T,
    limit: usize,
    size: usize,
    initial_cap: usize,
    mut step: F,
) -> (Result<bool, E>, usize, usize)
where
    F: FnMut(usize, usize, T) -> (Result<Option<T>, E>, usize, usize),
{
    let mut len = 0;
    let mut cap = initial_cap;
    let mut pending = first;
    #[cfg(verus_keep_ghost)]
    let mut history = verus_exec_expr!(Ghost(Seq::<(
        usize,
        usize,
        T,
        F,
        (Result<Option<T>, E>, usize, usize)
    )>::empty()));
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant len <= cap <= limit, byte_valid(cap,size),
            history@.len() == 0 ==> len == 0,
            history@.len() > 0 ==> history@.last().4.1 == len
                && history@.last().4.2 == cap,
            forall|j:int| 0 <= j < history@.len() ==>
                history@[j].0 == (if j == 0 { 0usize } else { history@[j-1].4.1 })
                && history@[j].0 < history@[j].4.1
                && call_ensures(history@[j].3,
                    (history@[j].0,history@[j].1,history@[j].2),history@[j].4),
            forall|i:usize,c:usize,v:T|
                i < limit && i <= c <= limit && byte_valid(c,size)
                ==> call_requires(step,(i,c,v)),
        decreases limit-len,
    ))]
    loop {
        if len >= limit {
            return (Ok(false), len, cap);
        }
        let next = wrapping_add(len, 1);
        #[cfg(verus_keep_ghost)]
        let before_step = verus_exec_expr!(Ghost(step));
        #[cfg(verus_keep_ghost)]
        let before_value = verus_exec_expr!(Ghost(pending));
        let batch = step(len, cap, pending);
        #[cfg(verus_keep_ghost)]
        let observed = verus_exec_expr!(Ghost(batch));
        let (reply, new_len, new_cap) = batch;
        if new_len < len
            || new_len > new_cap
            || new_cap > limit
            || crate::layout::numeric::array_bytes(new_cap, size).is_err()
        {
            return (Ok(false), len, cap);
        }
        match reply {
            Err(error) => return (Err(error), new_len, new_cap),
            Ok(None) => return (Ok(true), new_len, new_cap),
            Ok(Some(value)) => {
                if new_len < next {
                    return (Ok(false), new_len, new_cap);
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(call_ensures(before_step@,(len,cap,before_value@),observed@));
                    *history = history@.push((len,cap,before_value@,before_step@,observed@));
                }
                len = new_len;
                cap = new_cap;
                pending = value;
            }
        }
    }
}
