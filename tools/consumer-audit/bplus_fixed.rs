// Proofs for the fixed-width view's setup and lookup invariants.
use vstd::prelude::*;

verus! {
proof fn claim_parent(flags: Seq<bool>, child: int)
    requires 0 <= child < flags.len(), !flags[child],
    ensures
        flags.update(child, true)[child],
        forall|i: int| 0 <= i < flags.len() && i != child
            ==> #[trigger] flags.update(child, true)[i] == flags[i],
{}

spec fn reaches_root(parents: Seq<int>, node: int, fuel: nat) -> bool
    decreases fuel,
{
    if fuel == 0 || node < 0 || node >= parents.len() {
        false
    } else if node + 1 == parents.len() {
        true
    } else {
        reaches_root(parents, parents[node], (fuel - 1) as nat)
    }
}

proof fn all_nodes_reach_the_last_record(parents: Seq<int>, node: int)
    requires
        0 <= node < parents.len(),
        forall|i: int| 0 <= i && i + 1 < parents.len()
            ==> i < (#[trigger] parents[i]) < parents.len(),
    ensures reaches_root(parents, node, (parents.len() - node) as nat),
    decreases parents.len() - node,
{
    let fuel = (parents.len() - node) as nat;
    if node + 1 < parents.len() {
        let parent = parents[node];
        all_nodes_reach_the_last_record(parents, parent);
        extend_fuel(parents, parent, (parents.len() - parent) as nat, (fuel - 1) as nat);
        reveal_with_fuel(reaches_root, 2);
    } else {
        reveal_with_fuel(reaches_root, 1);
    }
}

proof fn extend_fuel(parents: Seq<int>, node: int, before: nat, after: nat)
    requires before <= after, reaches_root(parents, node, before),
    ensures reaches_root(parents, node, after),
    decreases before,
{
    reveal_with_fuel(reaches_root, 2);
    assert(before > 0 && 0 <= node < parents.len());
    if node + 1 < parents.len() {
        extend_fuel(parents, parents[node], (before - 1) as nat, (after - 1) as nat);
    }
}

proof fn branch_address(leaves: usize, branches: usize, id: usize)
    requires leaves <= id, (id as int) < leaves as int + branches as int,
    ensures 0 <= (id as int - leaves as int) < branches as int,
{}

proof fn descend_to_child(nodes: usize, id: usize, child: usize, parent_height: usize, height: usize)
    requires
        child < id < nodes,
        parent_height as int == height as int + 1,
    ensures
        child < nodes,
        parent_height > 0,
        parent_height as int - 1 == height as int,
        height < parent_height,
{}

proof fn ordered_child_ranges(
    left_min: int, left_max: int, right_min: int, right_max: int, separator: int,
)
    requires left_min <= left_max < separator, separator == right_min, right_min <= right_max,
    ensures left_min <= left_max < right_min <= right_max,
{}

// At each level, the validated tree has disjoint, equal-width child subtrees.
proof fn range_append_digit(prefix: int, parents: int, slot: int, fanout: int)
    requires 0 <= prefix < parents, 0 <= slot < fanout,
    ensures 0 <= prefix * fanout + slot < parents * fanout,
{
    assert(0 <= prefix * fanout + slot < parents * fanout) by (nonlinear_arith)
        requires 0 <= prefix < parents, 0 <= slot < fanout;
}

proof fn range_decode_digit(rank: int, child_width: int, fanout: int)
    requires child_width > 0, fanout > 0, 0 <= rank < fanout * child_width,
    ensures
        0 <= rank / child_width < fanout,
        0 <= rank % child_width < child_width,
        rank == child_width * (rank / child_width) + rank % child_width,
{
    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(rank, child_width);
    assert(0 <= rank / child_width < fanout) by (nonlinear_arith)
        requires child_width > 0, fanout > 0, 0 <= rank < fanout * child_width,
            0 <= rank % child_width < child_width,
            rank == child_width * (rank / child_width) + rank % child_width;
}

proof fn range_admit_leaf_span(first: usize, limit: usize, width: usize, index: usize)
    requires first <= limit <= width, index < limit as int - first as int,
    ensures 0 <= limit as int - first as int <= width,
        0 <= (first as int + index as int) < width,
{}

// Slot zero is the fallback when no separator is admitted. In a sorted
// branch, admitting any later minimum also admits the first minimum.
proof fn range_route_by_later_minima(width: int, later: int, full: int, first_admitted: bool)
    requires
        0 < width,
        0 <= later < width,
        later > 0 ==> first_admitted,
        full == if first_admitted { later + 1 } else { later },
    ensures
        0 <= later < width,
        later == if full > 0 { full - 1 } else { 0 },
{}

// The checked lower and upper bounds give first <= limit. When each
// candidate exists, being outside the upper bound is equivalent to
// having a slot at or after the end of the admitted prefix.
proof fn range_short_limit(first: int, limit: int, width: int)
    requires 0 <= first <= limit <= width,
    ensures
        first < width && limit <= first ==> limit == first,
        first + 1 < width && first < limit <= first + 1 ==> limit == first + 1,
{}

fn main() {}
}
