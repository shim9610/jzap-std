//! Actual scalar kernels and bounded iterator/deletion callers; no body projection.
use super::geometry;

#[kani::proof]
fn search_step_preserves_every_word_interval() {
    let low: usize = kani::any();
    let high: usize = kani::any();
    kani::assume(low < high);
    let middle = geometry::middle(low, high);
    assert!(low <= middle && middle < high);
    let next = geometry::advance(low, high, middle, kani::any());
    assert!(low <= next.0 && next.0 <= next.1 && next.1 <= high);
    assert!(next.1 - next.0 < high - low);
}

#[kani::proof]
fn cleanup_frontier_preserves_every_word_count_and_remainder() {
    let index: usize = kani::any();
    let count: usize = kani::any();
    let remaining: usize = kani::any();
    kani::assume(index < count && remaining > 0);
    let skip = geometry::drop_skip(index, count, remaining);
    let out = (
        crate::arithmetic::bounded::wrapping_add(index, skip),
        crate::arithmetic::bounded::wrapping_sub(remaining, skip),
    );
    assert!(index <= out.0 && out.0 < count && 1 <= out.1 && out.1 <= remaining);
    assert!(out.1 + (out.0 - index) == remaining);
}
#[kani::proof]
fn borrowed_leaf_edge_preserves_every_word_coordinate() {
    let index: usize = kani::any();
    let count: usize = kani::any();
    let back: bool = kani::any();
    kani::assume(index <= count && if back { index > 0 } else { index < count });
    let out = geometry::leaf_edge(index, count, back);
    assert!(out.0 < count && out.1 <= count);
    assert!(out.0 == index - usize::from(back));
    assert!(out.1 == out.0 + usize::from(!back));
}

#[kani::proof]
fn deletion_decisions_cover_every_admitted_word_input() {
    let child: usize = kani::any();
    let sibling: usize = kani::any();
    let index: usize = kani::any();
    let parent: usize = kani::any();
    kani::assume(child == 4 && sibling <= 11 && 0 < parent && parent <= 11 && index <= parent);
    let borrow = geometry::borrow_required(sibling, child);
    assert!(borrow == (sibling + child + 1 > 11));
    if geometry::consider_left(index) && borrow {
        assert!(index > 0 && sibling > 0 && child < 11);
    }
    if geometry::consider_right(index, parent) && borrow {
        assert!(index == 0 && index < parent && sibling > 0 && child < 11);
    }
    if !borrow {
        assert!(sibling + child + 1 <= 11);
    }
}

#[kani::proof]
fn node_spans_cover_every_admitted_word_coordinate() {
    let n: usize = kani::any();
    let m: usize = kani::any();
    let i: usize = kani::any();
    kani::assume(n <= 11 && m <= 11 && i <= 11);
    if n < 11 && i <= n {
        assert_eq!(geometry::insert_span(n, i), (i + 1, n - i, n + 1, i + 2));
    }
    if i < n {
        assert_eq!(geometry::split_span(n, i), (i + 1, n - i - 1, n - i));
        assert_eq!(geometry::remove_span(n, i), (i + 1, n - i - 1, n - 1));
    }
    if i <= n {
        assert_eq!(geometry::suffix_span(n, i), (n - i, i + 1));
    }
    if n + m < 11 {
        assert_eq!(geometry::merge_span(n, m), (n + 1, n + m + 1, m + 1));
    }
    if n > 0 && m < 11 {
        assert_eq!(geometry::steal_left_span(n, m), (n - 1, m + 1));
    }
    if n < 11 && m > 0 {
        assert_eq!(geometry::steal_right_span(n, m), (n + 1, m - 1));
    }
}
