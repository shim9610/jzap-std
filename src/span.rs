//! Closed integer range state for owning iterators. No payload or allocation.
use crate::Checked;
use crate::arithmetic::bounded::{wrapping_add, wrapping_sub};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

// Fields are private: only the constructors and the two transitions below can
// establish/change the ordered-endpoint invariant. There is no raw-state escape.
#[derive(Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct Span {
    front: Checked<usize>,
    back: Checked<usize>,
}
#[cfg(verus_keep_ghost)]
verus! {
pub(crate) struct SpanView { pub front: usize, pub back: usize }
impl Span {
    pub(crate) closed spec fn view(self) -> SpanView {
        SpanView { front: self.front@, back: self.back@ }
    }
    pub(crate) open spec fn wf(self) -> bool { self@.front <= self@.back }
    pub(crate) fn proof_from_state(front: usize, back: usize) -> (s: Self)
        requires front <= back,
        ensures s.wf(), s@.front == front, s@.back == back,
    { Self { front: Checked::new(front), back: Checked::new(back) } }
}
}
#[cfg_attr(verus_keep_ghost, verus_verify)]
impl Span {
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out.wf(), out@.front == 0, out@.back == end,))]
    pub(crate) const fn full(end: usize) -> Self {
        Self {
            front: Checked::new(0),
            back: Checked::new(end),
        }
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures match out {
        Ok(s) => s.wf() && s@.front == front && s@.back == back,
        Err(crate::Error::Bounds) => front > back,
        _ => false,
    },))]
    pub(crate) fn try_new(front: usize, back: usize) -> Result<Self, crate::Error> {
        if front > back {
            return Err(crate::Error::Bounds);
        }
        Ok(Self {
            front: Checked::new(front),
            back: Checked::new(back),
        })
    }
    #[cfg(kani)]
    #[inline]
    pub(crate) fn back(&self) -> usize {
        self.back.get()
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(requires old(self).wf(),
        ensures final(self).wf(), final(self)@.front == old(self)@.back, final(self)@.back == old(self)@.back,))]
    pub(crate) fn clear(&mut self) {
        self.front = self.back;
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == self@.front,))]
    pub(crate) fn front(&self) -> usize {
        self.front.get()
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => requires self.wf(), ensures out as int == self@.back as int - self@.front as int,))]
    pub(crate) fn len(&self) -> usize {
        // Invariant: front <= back. Actual constructor/transition closure and
        // this subtraction are verified over all usize states by Kani below.
        wrapping_sub(self.back.get(), self.front.get())
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => requires old(self).wf(),
        ensures final(self).wf(), final(self)@.back == old(self)@.back,
            match out {
                Some(i) => old(self)@.front < old(self)@.back && i == old(self)@.front
                    && final(self)@.front as int == old(self)@.front as int + 1,
                None => old(self)@.front == old(self)@.back && final(self)@.front == old(self)@.front,
            },))]
    pub(crate) fn next(&mut self) -> Option<usize> {
        if self.front.get() >= self.back.get() {
            return None;
        }
        let index = self.front;
        let next = Checked::new(wrapping_add(self.front.get(), 1));
        self.front = next;
        Some(index.get())
    }
    #[inline]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => requires old(self).wf(),
        ensures final(self).wf(), final(self)@.front == old(self)@.front,
            match out {
                Some(i) => old(self)@.front < old(self)@.back && i == final(self)@.back
                    && final(self)@.back as int == old(self)@.back as int - 1,
                None => old(self)@.front == old(self)@.back && final(self)@.back == old(self)@.back,
            },))]
    pub(crate) fn next_back(&mut self) -> Option<usize> {
        if self.front.get() >= self.back.get() {
            return None;
        }
        let next = Checked::new(wrapping_sub(self.back.get(), 1));
        self.back = next;
        Some(next.get())
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;
    #[kani::proof]
    fn full_constructor_establishes_order() {
        let end: usize = kani::any();
        let span = Span::full(end);
        assert!(span.front.get() <= span.back.get());
        assert!(span.len() == end);
    }
    // This arbitrary invariant state covers the inductive closure, rather than
    // limiting the number of successful iterator steps with an unwind bound.
    #[kani::proof]
    fn both_transitions_preserve_order_and_exact_length() {
        let front: usize = kani::any();
        let back: usize = kani::any();
        if front > back {
            assert!(Span::try_new(front, back).is_err());
            return;
        }
        let initial = Span::try_new(front, back).unwrap();
        assert!(initial.len() == back - front);
        let mut cleared = initial;
        cleared.clear();
        assert!(cleared.len() == 0 && cleared.front() == back && cleared.back() == back);
        let mut forward = initial;
        let mut reverse = initial;
        let (a, b) = (forward.next(), reverse.next_back());
        if front == back {
            assert!(a.is_none() && b.is_none());
            assert!(forward.len() == 0 && reverse.len() == 0);
        } else {
            assert!(a == Some(front));
            assert!(b == Some(back - 1));
            assert!(forward.front.get() == front + 1 && forward.back.get() == back);
            assert!(reverse.front.get() == front && reverse.back.get() == back - 1);
            assert!(forward.front.get() <= forward.back.get());
            assert!(reverse.front.get() <= reverse.back.get());
            assert!(forward.len() == initial.len() - 1);
            assert!(reverse.len() == initial.len() - 1);
        }
    }
}
