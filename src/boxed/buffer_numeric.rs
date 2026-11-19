//! Original private Buffer reservation arithmetic, isolated for direct verification.
use crate::{Checked, Error};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires len <= cap,
    ensures match out {
        Ok((needed, proposed)) => needed as int == len as int + additional as int
            && len <= needed <= proposed && cap <= proposed
            && (needed <= cap ==> proposed == cap)
            && (needed > cap && cap == 0 ==> proposed == (if hint > needed { hint } else { needed }))
            && (needed > cap && cap > 0 ==> proposed as int ==
                if cap as int * 2 <= usize::MAX as int {
                    if cap as int * 2 > needed as int {cap as int * 2} else {needed as int}
                } else { needed as int }),
        Err(error) => len as int + additional as int > usize::MAX
            && matches!(error, Error::Capacity),
    },
))]
#[inline]
pub(crate) fn reservation(
    len: usize,
    cap: usize,
    additional: usize,
    hint: usize,
) -> Result<(usize, usize), Error> {
    let needed = (Checked::new(len) + Checked::new(additional))
        .map_err(
            #[cfg_attr(verus_keep_ghost, verus_spec(error: Error => ensures error == Error::Capacity,))]
            |_error| Error::Capacity,
        )?
        .get();
    if needed <= cap {
        return Ok((needed, cap));
    }
    let proposed = if cap == 0 {
        hint.max(needed)
    } else {
        let doubled = match Checked::new(cap) * Checked::new(2) {
            Ok(value) => value.get(),
            Err(_) => needed,
        };
        doubled.max(needed)
    };
    Ok((needed, proposed))
}
