//! Explicit wrapping primitives with a non-overflow proof obligation.
//!
//! Only for infallible internal state protocols. Fallible boundaries must use
//! Checked instead. Kani checks these preconditions as assertions, not assumptions;
//! source-linked mathematical contracts must establish the same bounds at callers.
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_verify(dual_spec, open))]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires (left as int) + (right as int) <= usize::MAX,
    ensures out as int == (left as int) + (right as int),
))]
#[inline]
pub(crate) fn wrapping_add(left: usize, right: usize) -> usize {
    #[cfg(kani)]
    assert!(
        left.checked_add(right).is_some(),
        "bounded addition overflow"
    );
    left.wrapping_add(right)
}
#[cfg_attr(verus_keep_ghost, verus_verify(dual_spec, open))]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires right <= left,
    ensures out as int == (left as int) - (right as int),
))]
#[inline]
pub(crate) fn wrapping_sub(left: usize, right: usize) -> usize {
    #[cfg(kani)]
    assert!(
        left.checked_sub(right).is_some(),
        "bounded subtraction underflow"
    );
    left.wrapping_sub(right)
}
#[cfg(feature = "alloc")]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires (left as int) * (right as int) <= usize::MAX,
    ensures out as int == (left as int) * (right as int),
))]
#[inline]
pub(crate) fn wrapping_mul(left: usize, right: usize) -> usize 
//@ req true;
//@ ens if left * right <= usize::MAX { result == left * right } else { true };
//@ on_unwind_ens false;
{
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(0 <= (left as int) * (right as int)) by(nonlinear_arith);
        vstd::arithmetic::div_mod::lemma_small_mod(
            ((left as int) * (right as int)) as nat,
            (usize::MAX as nat) + 1,
        );
    }
    #[cfg(kani)]
    assert!(
        left.checked_mul(right).is_some(),
        "bounded multiplication overflow"
    );
    left.wrapping_mul(right)
}
