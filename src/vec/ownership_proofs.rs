//! Cursor induction step on the real owned iterator, for every machine word.
use super::*;

#[kani::proof]
#[kani::solver(z3)]
fn vec_iterator_preserves_every_valid_cursor_state() {
    let front: usize = kani::any();
    let back: usize = kani::any();
    kani::assume(front <= back);
    // SAFETY: an initialized unit range requires no storage or charge. These
    // are exactly the states reachable by consuming a unit Vec of length back.
    let owner =
        unsafe { Vec::from_raw_parts(NonNull::dangling(), 0, usize::MAX, Unmetered, Unlimited) };
    let mut it = IntoIter::<(), Unmetered, Unlimited> {
        owner,
        span: crate::span::Span::try_new(front, back).unwrap(),
    };
    let _ = it.as_slice();
    let _ = it.as_mut_slice();
    let _ = it.next();
    let _ = it.next_back();
    assert!(
        front <= it.span.front() && it.span.front() <= it.span.back() && it.span.back() <= back
    );
    assert!(it.len() == it.span.back() - it.span.front());
    assert!(it.owner.extent.len() == 0);
    drop(it);
}
