//! Shared admitted initialization cursor; the executable loop is verified directly.
use crate::{Checked, arithmetic::bounded::wrapping_add};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(state)@ <= end,
        forall|i: usize| old(state)@ <= i < end ==> call_requires(step, (i,)),
    ensures final(state)@ <= end,
        old(state)@ <= final(state)@,
        out.is_ok() ==> final(state)@ == end,
        out.is_err() ==> final(state)@ < end,
))]
#[inline]
pub(crate) fn run<E, F: FnMut(usize) -> Result<(), E>>(
    state: &mut Checked<usize>,
    end: usize,
    mut step: F,
) -> Result<(), E> {
    #[cfg(all(kani, jzap_init_loop_contracts))]
    let begin = state.get();
    #[cfg(verus_keep_ghost)]
    let mut history = verus_exec_expr!(Ghost(Seq::<(usize, F)>::empty()));
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant state@ <= end,
            old(state)@ <= state@,
            history@.len() == state@ as int - old(state)@ as int,
            forall|j:int| 0 <= j < history@.len() ==> history@[j].0 as int == old(state)@ as int + j
                && call_ensures(history@[j].1, (history@[j].0,), Ok(())),
            forall|i: usize| old(state)@ <= i < end ==> call_requires(step, (i,)),
        decreases end - state@,
    ))]
    #[cfg_attr(all(kani, jzap_init_loop_contracts), kani::loop_invariant(begin <= state.get() && state.get() <= end))]
    while state.get() < end {
        let index = state.get();
        #[cfg(verus_keep_ghost)]
        let before = verus_exec_expr!(Ghost(step));
        let result = step(index);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(call_ensures(before@, (index,), result));
            if result.is_ok() {
                *history = history@.push((index, before@));
            } else {
                assert(history@.len() == index as int - old(state)@ as int);
            }
        }
        result?;
        *state = Checked::new(wrapping_add(index, 1));
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(history@.len() == end as int - old(state)@ as int);
    }
    Ok(())
}
