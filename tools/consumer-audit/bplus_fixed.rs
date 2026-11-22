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

// FixedTree::get indexes the pools without checks. The records are leaves
// first, then branches; `heights` are the summary heights that try_new
// computes, and `children` the child IDs of each branch.
spec fn validated(leaves: int, heights: Seq<int>, children: Seq<Seq<int>>, width: int) -> bool {
    &&& 0 < width
    &&& 0 <= leaves <= heights.len()
    &&& children.len() == heights.len() - leaves
    &&& forall|i: int| 0 <= i < leaves ==> #[trigger] heights[i] == 0
    &&& forall|j: int| 0 <= j < children.len() ==> {
        &&& #[trigger] children[j].len() == width
        &&& heights[leaves + j] >= 1
        &&& forall|s: int| 0 <= s < width ==> {
            &&& 0 <= #[trigger] children[j][s] < leaves + j
            &&& heights[children[j][s]] == heights[leaves + j] - 1
        }
    }
}

// try_new pushes every leaf with height zero before the first branch.
proof fn push_leaf_preserves_validation(leaves: int, heights: Seq<int>, width: int)
    requires validated(leaves, heights, Seq::empty(), width), leaves == heights.len(),
    ensures validated(leaves + 1, heights.push(0), Seq::empty(), width),
{
    assert forall|i: int| 0 <= i < leaves + 1 implies #[trigger] heights.push(0)[i] == 0 by {
        if i < leaves {
            assert(heights.push(0)[i] == heights[i]);
        }
    }
}

// summarize_branch looks every child up in the completed prefix, requires
// equal child heights and records the checked successor height.
proof fn push_branch_preserves_validation(
    leaves: int, heights: Seq<int>, children: Seq<Seq<int>>, width: int, ids: Seq<int>,
)
    requires
        validated(leaves, heights, children, width),
        ids.len() == width,
        forall|s: int| 0 <= s < width ==> 0 <= #[trigger] ids[s] < heights.len(),
        forall|s: int| 0 <= s < width ==> heights[#[trigger] ids[s]] == heights[ids[0]],
    ensures validated(leaves, heights.push(heights[ids[0]] + 1), children.push(ids), width),
{
    let h = heights.push(heights[ids[0]] + 1);
    let c = children.push(ids);
    assert(heights[ids[0]] >= 0) by {
        lemma_heights_nonnegative(leaves, heights, children, width, ids[0]);
    }
    assert forall|i: int| 0 <= i < leaves implies #[trigger] h[i] == 0 by {
        assert(h[i] == heights[i]);
    }
    assert forall|j: int| 0 <= j < c.len() implies {
        &&& #[trigger] c[j].len() == width
        &&& h[leaves + j] >= 1
        &&& forall|s: int| 0 <= s < width ==> {
            &&& 0 <= #[trigger] c[j][s] < leaves + j
            &&& h[c[j][s]] == h[leaves + j] - 1
        }
    } by {
        if j < children.len() {
            assert(c[j] == children[j]);
            assert(h[leaves + j] == heights[leaves + j]);
            assert forall|s: int| 0 <= s < width implies {
                &&& 0 <= #[trigger] c[j][s] < leaves + j
                &&& h[c[j][s]] == h[leaves + j] - 1
            } by {
                assert(h[c[j][s]] == heights[c[j][s]]);
            }
        } else {
            assert(c[j] == ids);
            assert(leaves + j == heights.len());
            assert forall|s: int| 0 <= s < width implies {
                &&& 0 <= #[trigger] c[j][s] < leaves + j
                &&& h[c[j][s]] == h[leaves + j] - 1
            } by {
                assert(h[ids[s]] == heights[ids[s]]);
            }
        }
    }
}

proof fn lemma_heights_nonnegative(
    leaves: int, heights: Seq<int>, children: Seq<Seq<int>>, width: int, id: int,
)
    requires validated(leaves, heights, children, width), 0 <= id < heights.len(),
    ensures heights[id] >= 0,
{
    if id >= leaves {
        assert(children[id - leaves].len() == width);
    }
}

// One loop step of FixedTree::get: a node of height `remaining + 1` is a
// branch whose address lies in the branch pool, and every child slot holds a
// record of height `remaining`.
proof fn descend_validated(
    leaves: int, heights: Seq<int>, children: Seq<Seq<int>>, width: int,
    id: int, remaining: int, slot: int,
)
    requires
        validated(leaves, heights, children, width),
        0 <= id < heights.len(), heights[id] == remaining + 1, remaining >= 0,
        0 <= slot < width,
    ensures
        leaves <= id,
        0 <= id - leaves < children.len(),
        0 <= children[id - leaves][slot] < id,
        heights[children[id - leaves][slot]] == remaining,
{
    assert(children[id - leaves].len() == width);
}

// After the loop, a record of height zero is a leaf.
proof fn height_zero_is_leaf(
    leaves: int, heights: Seq<int>, children: Seq<Seq<int>>, width: int, id: int,
)
    requires validated(leaves, heights, children, width), 0 <= id < heights.len(), heights[id] == 0,
    ensures id < leaves,
{
    if id >= leaves {
        assert(children[id - leaves].len() == width);
    }
}

// Descending exactly height(root) times from the root of a validated tree
// visits only branches and ends at a leaf, for any choice of child slots.
proof fn every_descent_ends_at_a_leaf(
    leaves: int, heights: Seq<int>, children: Seq<Seq<int>>, width: int,
    id: int, remaining: int, slots: Seq<int>,
)
    requires
        validated(leaves, heights, children, width),
        0 <= id < heights.len(), heights[id] == remaining, remaining >= 0,
        slots.len() == remaining,
        forall|k: int| 0 <= k < slots.len() ==> 0 <= #[trigger] slots[k] < width,
    ensures follow(leaves, children, id, slots) < leaves,
        0 <= follow(leaves, children, id, slots),
    decreases remaining,
{
    if remaining == 0 {
        height_zero_is_leaf(leaves, heights, children, width, id);
    } else {
        descend_validated(leaves, heights, children, width, id, remaining - 1, slots[0]);
        let child = children[id - leaves][slots[0]];
        assert forall|k: int| 0 <= k < slots.drop_first().len()
            implies 0 <= #[trigger] slots.drop_first()[k] < width by {
            assert(slots.drop_first()[k] == slots[k + 1]);
        }
        every_descent_ends_at_a_leaf(leaves, heights, children, width, child, remaining - 1,
            slots.drop_first());
    }
}

spec fn follow(leaves: int, children: Seq<Seq<int>>, id: int, slots: Seq<int>) -> int
    decreases slots.len(),
{
    if slots.len() == 0 {
        id
    } else {
        follow(leaves, children, children[id - leaves][slots[0]], slots.drop_first())
    }
}

// floor_slot keeps base + remaining <= width, so its checked middle position,
// its bound check and its subtraction never fall back, and the result is a
// slot below the width.
proof fn floor_slot_step(width: int, base: int, remaining: int, go: bool)
    requires 0 < width, 0 <= base, 1 < remaining, base + remaining <= width,
    ensures ({
        let half = remaining / 2;
        let middle = base + half;
        let next = if go { middle } else { base };
        &&& 0 < half < remaining
        &&& 0 <= middle < width
        &&& 1 <= remaining - half
        &&& next + (remaining - half) <= width
        &&& 0 <= next < width
    }),
{}

proof fn floor_slot_start(width: int)
    requires 0 < width,
    ensures 0 <= 0 < width, 0 + width <= width,
{}

fn main() {}
}
