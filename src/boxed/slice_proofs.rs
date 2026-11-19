use super::*;

#[kani::proof]
#[kani::solver(z3)]
fn box_iterator_preserves_every_valid_cursor_state() {
    let front: usize = kani::any();
    let back: usize = kani::any();
    kani::assume(front <= back);
    let mut it = IntoIter::<(), Unmetered> {
        ptr: NonNull::dangling(),
        span: crate::span::Span::try_new(front, back).unwrap(),
        layout: Layout::new::<()>(),
        policy: Unmetered,
        marker: PhantomData,
    };
    let _ = it.next();
    let _ = it.next_back();
    assert!(
        front <= it.span.front() && it.span.front() <= it.span.back() && it.span.back() <= back
    );
    assert!(it.len() == it.span.back() - it.span.front());
    drop(it);
}
