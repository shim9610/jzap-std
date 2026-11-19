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

// Above the selected child, the frame stores the next sibling slot.
// A slot equal to width means that the parent has no unvisited child.
proof fn range_save_sibling(depth: usize, height: usize, cap: usize, index: usize, width: usize)
    requires depth < height <= cap, index < width,
    ensures depth < cap, depth as int + 1 <= height, index as int + 1 <= width,
{}

proof fn range_resume_height(depth: usize, height: usize)
    requires 0 < depth <= height,
    ensures 0 <= (height as int - depth as int), (height as int - depth as int) < (height as int),
            (height as int) - (depth as int - 1) == (height as int) - (depth as int) + 1,
{}

proof fn range_advance_leaf_slot(index: usize, width: usize)
    requires index < width,
    ensures index < width, index as int + 1 <= width,
{}

fn main() {}
}
