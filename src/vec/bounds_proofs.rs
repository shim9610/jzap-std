//! Execute the real bound adapter with arbitrary endpoints and observable getters.
use crate::Error;
use core::{
    cell::Cell,
    ops::{Bound, RangeBounds},
};

struct Observed<'a> {
    start: Bound<usize>,
    end: Bound<usize>,
    starts: &'a Cell<u8>,
    ends: &'a Cell<u8>,
}
impl RangeBounds<usize> for Observed<'_> {
    fn start_bound(&self) -> Bound<&usize> {
        self.starts.set(self.starts.get() + 1);
        self.start.as_ref()
    }
    fn end_bound(&self) -> Bound<&usize> {
        assert!(self.starts.get() == 1);
        self.ends.set(self.ends.get() + 1);
        self.end.as_ref()
    }
}
fn bound(value: usize) -> Bound<usize> {
    match kani::any::<u8>() {
        0 => Bound::Included(value),
        1 => Bound::Excluded(value),
        _ => Bound::Unbounded,
    }
}
#[kani::proof]
fn every_endpoint_and_getter_order() {
    let length: usize = kani::any();
    let start = bound(kani::any());
    let end = bound(kani::any());
    let lower = match start {
        Bound::Included(n) => n as u128,
        Bound::Excluded(n) => n as u128 + 1,
        Bound::Unbounded => 0,
    };
    let upper = match end {
        Bound::Included(n) => n as u128 + 1,
        Bound::Excluded(n) => n as u128,
        Bound::Unbounded => length as u128,
    };
    let starts = Cell::new(0);
    let ends = Cell::new(0);
    let actual = super::edit::checked_range(
        Observed {
            start,
            end,
            starts: &starts,
            ends: &ends,
        },
        length,
    );
    if lower <= upper && upper <= length as u128 {
        assert!(actual == Ok((lower as usize, upper as usize)));
    } else {
        assert!(actual == Err(Error::Bounds));
    }
    assert!(starts.get() == 1);
    assert!(ends.get() == u8::from(lower <= usize::MAX as u128));
}
