// Pending caller compatibility: invoke the actual initialization loop.
use vstd::prelude::*;
verus! {
fn initialized_box_prefix(target: usize, fail_at: usize, callback_ok: bool, slot_written: bool)
    -> (r: (bool, usize))
    ensures r.1 <= target, r.0 ==> r.1 == target, !r.0 ==> r.1 < target,
{
    let mut state = crate::Checked::new(0usize);
    let result = crate::init::driver::run(&mut state, target, |index: usize| -> Result<(), ()> {
        if index == fail_at && (!callback_ok || !slot_written) { Err(()) } else { Ok(()) }
    });
    (result.is_ok(), state.get())
}
// The increments of `init::driver::run` and `vec::staging_driver::run` follow a
// guard on the same word (`while state.get() < end`, `if len >= limit {
// return }`). Below the bound the increment cannot wrap and stays within the
// bound, whatever the caller passed; verify_wrapping_memory.py pins both
// guards next to their increments.
fn guarded_increment(current: usize, bound: usize) -> (next: Option<usize>)
    ensures current < bound ==> next == Some((current + 1) as usize) && current + 1 <= bound,
        current >= bound ==> next.is_none(),
{
    if current < bound { Some(crate::arithmetic::bounded::wrapping_add(current, 1)) } else { None }
}
fn admitted_cursor<E, F: FnMut(usize) -> Result<(), E>>(
    state: &mut crate::Checked<usize>, end: usize, step: F,
) -> (out: Result<(), E>)
    requires old(state)@ <= end,
        forall|i:usize| old(state)@ <= i < end ==> call_requires(step,(i,)),
    ensures old(state)@ <= final(state)@ <= end,
        out.is_ok() ==> final(state)@ == end,
        out.is_err() ==> final(state)@ < end,
{ crate::init::driver::run(state,end,step) }

fn collection_cursor<F: Fn(usize) -> Result<bool, ()>>(
    initial: usize, end: usize, source: &F,
) -> (out: (Result<bool, ()>, usize))
    requires initial <= end, forall|i:usize| call_requires(*source,(i,)),
    ensures initial <= out.1 <= end,
        out.0 == Ok(true) ==> out.1 == end,
        out.0 != Ok(true) ==> out.1 < end,
{
    let mut state=crate::Checked::new(initial);
    let result=crate::init::driver::run(&mut state,end,|index:usize| {
        match source(index) {
            Ok(true) => Ok(()),
            Ok(false) => Err(crate::init::collection::Stop::End),
            Err(error) => Err(crate::init::collection::Stop::Source(error)),
        }
    });
    (crate::init::collection::finished(result),state.get())
}

}
