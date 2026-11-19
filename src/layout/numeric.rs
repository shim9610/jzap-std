//! Array byte admission used by the runtime layout constructor.
use crate::Error;
#[cfg(verus_keep_ghost)]
use vstd::{arithmetic::div_mod::*, prelude::*};
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out as int == if size == 0 { usize::MAX as int }
        else { isize::MAX as int / size as int },
))]
#[inline]
pub(crate) fn array_ceiling(size: usize) -> usize {
    (isize::MAX as usize)
        .checked_div(size)
        .unwrap_or(usize::MAX)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bytes) => bytes as int == count as int * size as int && bytes <= isize::MAX
            && (size == 0 || count as int <= isize::MAX as int / size as int),
        Err(Error::Layout) => count as int * size as int > isize::MAX,
        _ => false,
    },
))]
#[inline]
pub(crate) fn array_bytes(count: usize, size: usize) -> Result<usize, Error> {
    let ceiling = array_ceiling(size);
    #[cfg(verus_keep_ghost)]
    proof! {
        if size > 0 {
            lemma_fundamental_div_mod(isize::MAX as int, size as int);
            assert(ceiling as int == isize::MAX as int / size as int);
        }
    }
    if count > ceiling {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(count as int * size as int > isize::MAX as int) by(nonlinear_arith)
                requires size > 0, count as int > isize::MAX as int / size as int,
                    isize::MAX as int == size as int * (isize::MAX as int / size as int)
                        + isize::MAX as int % size as int,
                    0 <= isize::MAX as int % size as int,
                    (isize::MAX as int) % (size as int) < (size as int);
        }
        return Err(Error::Layout);
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        if size > 0 {
            assert(count as int * size as int <= isize::MAX as int) by(nonlinear_arith)
                requires count as int <= isize::MAX as int / size as int, size > 0,
                    isize::MAX as int == size as int * (isize::MAX as int / size as int)
                        + isize::MAX as int % size as int,
                    0 <= isize::MAX as int % size as int;
        }
    }
    let bytes = size.checked_mul(count).ok_or(Error::Layout)?;
    Ok(bytes)
}
