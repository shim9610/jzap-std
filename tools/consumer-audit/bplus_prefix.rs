// The constructor owns a Vec of complete records. Model its existing
// try_push contract at the publication boundary, independently of its allocator.
use vstd::prelude::*;

verus! {
spec fn backward_prefix(edges: Seq<Seq<usize>>) -> bool {
    forall|i: int, j: int|
        0 <= i < edges.len() && 0 <= j < edges[i].len()
            ==> (#[trigger] edges[i][j] as int) < i
}

proof fn empty_prefix()
    ensures backward_prefix(Seq::<Seq<usize>>::empty()),
{}

proof fn append_checked_node(prefix: Seq<Seq<usize>>, children: Seq<usize>)
    requires
        backward_prefix(prefix),
        forall|j: int| 0 <= j < children.len()
            ==> (#[trigger] children[j] as int) < prefix.len(),
    ensures
        backward_prefix(prefix.push(children)),
        prefix.push(children).len() == prefix.len() + 1,
{
    let result = prefix.push(children);
    assert forall|i: int, j: int|
        0 <= i < result.len() && 0 <= j < result[i].len()
            implies (#[trigger] result[i][j] as int) < i by {
        if i < prefix.len() {
            assert(result[i] == prefix[i]);
        } else {
            assert(i == prefix.len());
            assert(result[i] == children);
        }
    };
}

proof fn accepted_root(pages: usize, published: usize, root: usize)
    requires
        published == pages,
        published > 0,
        root as int == published as int - 1,
    ensures
        root < pages,
        root as int + 1 == pages as int,
{}

proof fn pool_partition(count: usize, leaf_count: usize, copied: usize, leaves: usize, branches: usize)
    requires
        leaf_count <= count,
        copied < count,
        leaves as int + branches as int == copied as int,
        leaves <= leaf_count,
        leaves < leaf_count ==> branches == 0,
    ensures
        if leaves < leaf_count {
            leaves as int + 1 <= leaf_count as int
                && (leaves as int + 1) + branches as int == copied as int + 1
        } else {
            branches as int + 1 <= count as int - leaf_count as int
                && leaves as int + (branches as int + 1) == copied as int + 1
        },
{}

proof fn published_pools(count: usize, leaf_count: usize, leaves: usize, branches: usize)
    requires
        leaf_count <= count,
        leaves as int + branches as int == count as int,
        leaves <= leaf_count,
        branches as int <= count as int - leaf_count as int,
    ensures
        leaves == leaf_count,
        branches as int == count as int - leaf_count as int,
{}
}
fn main() {}
