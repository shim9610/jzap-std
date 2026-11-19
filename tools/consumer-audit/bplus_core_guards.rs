use vstd::prelude::*;

verus! {
proof fn binary_search_init(len: usize)
    requires len > 0,
    ensures 0 < len, 0 + len as int <= len as int,
{}

proof fn binary_search_iteration(len: usize, base: usize, size: usize, choose_mid: bool)
    requires
        size > 1,
        base as int + size as int <= len as int,
    ensures
        (base as int + (size / 2) as int) < len as int,
        0 < (size as int - (size / 2) as int) < size as int,
        (if choose_mid { base as int + (size / 2) as int } else { base as int })
            + (size as int - (size / 2) as int) <= len as int,
{
    assert(1 <= size / 2 < size);
}

proof fn binary_search_finish(len: usize, base: usize, size: usize, less: bool)
    requires
        size == 1,
        base as int + size as int <= len as int,
    ensures
        base < len,
        base as int + (if less { 1int } else { 0int }) <= len as int,
{}

proof fn checked_split_subslices(len: usize, mid: usize)
    requires mid <= len,
    ensures
        mid as int <= len as int,
        0 <= len as int - mid as int <= len as int,
        mid as int + (len as int - mid as int) == len as int,
{}
// is_sorted_by constructs ArrayWindows<T, 2>. A successful first_chunk
// permits both reading the pair and advancing by the suffix starting at 1.
proof fn pair_window_advance(len: usize)
    requires len >= 2,
    ensures 1 <= len, 0 < len as int - 1 < len as int,
{}
// Projection of the private MaybeUninit path in Lookup. The checked slice
// split gives history [0, depth) and the write slot at depth. The reviewed
// implementation writes the slot before publishing depth, with every
// fallible access and callback before either update.
spec fn initialized_ancestors(path: Seq<bool>, depth: int) -> bool {
    0 <= depth <= path.len()
        && forall|index: int| 0 <= index < depth ==> #[trigger] path[index]
}

proof fn lookup_path_new(capacity: usize)
    ensures initialized_ancestors(Seq::new(capacity as nat, |_: int| false), 0),
{}

proof fn lookup_path_read(path: Seq<bool>, depth: usize, index: usize)
    requires
        initialized_ancestors(path, depth as int),
        index < depth,
    ensures index < path.len(), path[index as int],
{}

proof fn lookup_path_write(path: Seq<bool>, depth: usize)
    requires
        initialized_ancestors(path, depth as int),
        depth < path.len(),
        depth < usize::MAX,
    ensures initialized_ancestors(path.update(depth as int, true), depth as int + 1),
{
    assert forall|index: int| 0 <= index < depth as int + 1 implies
        #[trigger] path.update(depth as int, true)[index] by {
        if index < depth { assert(path[index]); }
    }
}

}
fn main() {}
