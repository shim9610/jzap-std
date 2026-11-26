//! Admission for Vec length transitions and array-to-element conversion.
use crate::{Checked, Error};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires needed <= limit,
    ensures match out {
        Ok(capacity) => needed <= capacity <= limit
            && capacity as int <= if size == 0 { usize::MAX as int }
                else { isize::MAX as int / size as int }
            && (exact ==> capacity == needed),
        Err(Error::Layout) => needed as int > if size == 0 { usize::MAX as int }
                else { isize::MAX as int / size as int },
        _ => false,
    },
))]
pub(crate) fn growth_for_layout(
    current: usize,
    needed: usize,
    limit: usize,
    size: usize,
    exact: bool,
) -> Result<usize, Error> {
    let layout_ceiling = crate::layout::numeric::array_ceiling(size);
    if needed > layout_ceiling {
        return Err(Error::Layout);
    }
    let minimum = if size == 1 {
        8
    } else if size <= 1024 {
        4
    } else {
        1
    };
    let capacity = if exact {
        needed
    } else {
        super::growth_capacity(current, needed, limit.min(layout_ceiling), minimum)
    };
    Ok(capacity)
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(next) => len@ < cap && next@ as int == len@ as int + 1 && next@ <= cap,
        Err(Error::Capacity) => len@ >= cap,
        _ => false,
    },
))]
pub(crate) fn append_cursor(len: Checked<usize>, cap: usize) -> Result<Checked<usize>, Error> {
    if len.get() >= cap {
        return Err(Error::Capacity);
    }
    match len + Checked::new(1) {
        Ok(next) => Ok(next),
        Err(_) => Err(Error::Capacity),
    }
}

/// The elements at and after `index`, which an insertion moves up by one.
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(shifted) => index <= len@ && shifted@ as int == len@ as int - index as int,
        Err(Error::Bounds) => index > len@,
        _ => false,
    },
))]
pub(crate) fn insertion(len: Checked<usize>, index: usize) -> Result<Checked<usize>, Error> {
    if index > len.get() {
        return Err(Error::Bounds);
    }
    match len - Checked::new(index) {
        Ok(shifted) => Ok(shifted),
        Err(_) => Err(Error::Bounds),
    }
}

/// The length after removing the initialized element at `index`.
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(next) => index < len@ && next@ as int == len@ as int - 1,
        Err(Error::Bounds) => index >= len@,
        _ => false,
    },
))]
pub(crate) fn shortened(len: Checked<usize>, index: usize) -> Result<Checked<usize>, Error> {
    if index >= len.get() {
        return Err(Error::Bounds);
    }
    match len - Checked::new(1) {
        Ok(next) => Ok(next),
        Err(_) => Err(Error::Bounds),
    }
}

/// The shortened length, and the elements after `index` that a removal moves
/// down by one.
#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((next, shifted)) => index < len@ && next@ as int == len@ as int - 1
            && shifted@ as int == next@ as int - index as int,
        Err(Error::Bounds) => index >= len@,
        _ => false,
    },
))]
pub(crate) fn removal(
    len: Checked<usize>,
    index: usize,
) -> Result<(Checked<usize>, Checked<usize>), Error> {
    let next = shortened(len, index)?;
    match next - Checked::new(index) {
        Ok(shifted) => Ok((next, shifted)),
        Err(_) => Err(Error::Bounds),
    }
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(removed) => requested@ < current@ && removed@ as int == current@ as int - requested@ as int,
        None => requested@ >= current@,
    },
))]
pub(crate) fn removed_suffix(
    current: Checked<usize>,
    requested: Checked<usize>,
) -> Option<Checked<usize>> {
    let Ok(removed) = current - requested else {
        return None;
    };
    if removed.get() == 0 {
        return None;
    }
    Some(removed)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires len <= cap <= limit,
        cap as int * width as int * size as int <= isize::MAX as int,
    ensures match out {
        Some((n, c)) => n <= c <= limit
            && (size == 0 ==> c == limit)
            && (size != 0 ==> c as int == cap as int * width as int)
            && n as int == len as int * width as int
            && c as int * size as int == cap as int * width as int * size as int
            && c as int * size as int <= isize::MAX as int,
        None => len as int * width as int > limit as int
            || (size != 0 && cap as int * width as int > limit as int),
    },
))]
#[inline]
pub(crate) fn flatten(
    len: usize,
    cap: usize,
    limit: usize,
    size: usize,
    width: usize,
) -> Option<(usize, usize)> {
    let Ok(length) = Checked::new(len) * Checked::new(width) else {
        return None;
    };
    let capacity = if size == 0 {
        limit
    } else if width == 0 {
        0
    } else {
        let Ok(value) = Checked::new(cap) * Checked::new(width) else {
            return None;
        };
        value.get()
    };
    let length = length.get();
    if length > limit || capacity > limit {
        return None;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        if width == 0 {
            assert(length == 0) by(nonlinear_arith)
                requires length as int == len as int * width as int, width == 0;
        }
        if size > 0 && width > 0 {
            assert(length <= capacity) by(nonlinear_arith)
                requires length as int == len as int * width as int,
                    capacity as int == cap as int * width as int, len <= cap, width > 0;
        }
        assert(capacity as int * size as int == cap as int * width as int * size as int) by(nonlinear_arith)
            requires size == 0 ==> capacity == limit,
                size > 0 && width == 0 ==> capacity == 0,
                size > 0 && width > 0 ==> capacity as int == cap as int * width as int;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(length as int == len as int * width as int);
        assert(length <= capacity);
        assert(capacity <= limit);
        assert(capacity as int * size as int <= isize::MAX as int);
    }
    Some((length, capacity))
}
