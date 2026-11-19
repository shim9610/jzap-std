use vstd::prelude::*;
verus! {
// Logical allocation identities are never reused while a credit remains live.
// Owner/accounting composition is proved by owner-credit.verus.rs; actual Rust
// uniqueness is supplied by the ownership/compiler/Miri verification boundary.
ghost struct Credit { budget: nat, bytes: usize }
spec fn weight(c: Credit, budget: nat) -> nat {
    if c.budget == budget { c.bytes as nat } else { 0 }
}
spec fn credit_sum(m: Map<nat, Credit>, budget: nat) -> nat
    decreases m.dom().len(),
{
    if m.dom().is_empty() { 0 }
    else {
        let key = choose|key: nat| m.dom().contains(key);
        weight(m[key], budget) + credit_sum(m.remove(key), budget)
    }
}
proof fn sum_remove(m: Map<nat, Credit>, key: nat, budget: nat)
    requires m.dom().contains(key),
    ensures credit_sum(m, budget) == credit_sum(m.remove(key), budget) + weight(m[key], budget),
    decreases m.dom().len(),
{
    let first = choose|k: nat| m.dom().contains(k);
    if first != key {
        sum_remove(m.remove(first), key, budget);
        sum_remove(m.remove(key), first, budget);
        assert(m.remove(first).remove(key) =~= m.remove(key).remove(first));
    }
}
proof fn sum_insert(m: Map<nat, Credit>, key: nat, c: Credit, budget: nat)
    requires !m.dom().contains(key),
    ensures credit_sum(m.insert(key, c), budget) == credit_sum(m, budget) + weight(c, budget),
{
    assert(m.insert(key, c).remove(key) =~= m);
    sum_remove(m.insert(key, c), key, budget);
}
ghost struct Ledger {
    budget: nat, limit: usize, occupied: usize, available: usize,
    pending: Map<nat, Credit>, charged: Map<nat, Credit>,
}
impl Ledger {
    spec fn wf(self) -> bool {
        self.pending.dom().disjoint(self.charged.dom())
        && self.occupied as nat == credit_sum(self.charged, self.budget)
        && self.limit as nat == self.occupied as nat + self.available as nat
            + credit_sum(self.pending, self.budget)
    }
}
proof fn owned_credit_bound(m: Map<nat, Credit>, key: nat, budget: nat, total: usize)
    requires m.dom().contains(key), m[key].budget == budget,
        credit_sum(m, budget) == total as nat,
    ensures m[key].bytes <= total,
{
    sum_remove(m, key, budget);
}
proof fn ledger_new(budget: nat, limit: usize) -> (post: Ledger)
    ensures post.wf(), post.budget == budget, post.limit == limit,
{
    Ledger { budget, limit, occupied: 0, available: limit,
        pending: Map::empty(), charged: Map::empty() }
}
proof fn ledger_reserve(pre: Ledger, key: nat, bytes: usize) -> (post: Ledger)
    requires pre.wf(), bytes <= pre.available,
        !pre.pending.dom().contains(key), !pre.charged.dom().contains(key),
    ensures post.wf(), post.budget == pre.budget, post.limit == pre.limit,
        post.charged==pre.charged,
        post.pending==pre.pending.insert(key,Credit{budget:pre.budget,bytes}),
        post.pending.dom().contains(key), post.pending[key].budget == pre.budget,
        post.pending[key].bytes == bytes,
{
    let credit = Credit { budget: pre.budget, bytes };
    sum_insert(pre.pending, key, credit, pre.budget);
    let available = match crate::budget_numeric::reserve(pre.available, bytes) {
        Ok(value) => value,
        Err(_) => { assert(false); return pre; },
    };
    Ledger { available,
        pending: pre.pending.insert(key, credit), ..pre }
}
proof fn ledger_commit(pre: Ledger, key: nat) -> (post: Ledger)
    requires pre.wf(), pre.pending.dom().contains(key), pre.pending[key].budget == pre.budget,
    ensures post.wf(), post.budget == pre.budget, post.limit == pre.limit,
        post.pending==pre.pending.remove(key),
        post.charged==pre.charged.insert(key,pre.pending[key]),
        post.charged.dom().contains(key), post.charged[key] == pre.pending[key],
{
    let credit = pre.pending[key];
    let bytes = credit.bytes;
    sum_remove(pre.pending, key, pre.budget);
    sum_insert(pre.charged, key, credit, pre.budget);
    let occupied = crate::budget_numeric::credit(pre.occupied, bytes);
    Ledger { occupied,
        pending: pre.pending.remove(key), charged: pre.charged.insert(key, credit), ..pre }
}
proof fn ledger_cancel(pre: Ledger, key: nat) -> (post: Ledger)
    requires pre.wf(), pre.pending.dom().contains(key), pre.pending[key].budget == pre.budget,
    ensures post.wf(), post.budget == pre.budget, post.limit == pre.limit,
        post.charged==pre.charged,post.pending==pre.pending.remove(key),
        !post.pending.dom().contains(key),
{
    let bytes = pre.pending[key].bytes;
    sum_remove(pre.pending, key, pre.budget);
    let available = crate::budget_numeric::credit(pre.available, bytes);
    Ledger { available,
        pending: pre.pending.remove(key), ..pre }
}
proof fn ledger_release(pre: Ledger, key: nat) -> (post: Ledger)
    requires pre.wf(), pre.charged.dom().contains(key), pre.charged[key].budget == pre.budget,
    ensures post.wf(), post.budget == pre.budget, post.limit == pre.limit,
        post.pending==pre.pending,post.charged==pre.charged.remove(key),
        !post.charged.dom().contains(key),
{
    owned_credit_bound(pre.charged, key, pre.budget, pre.occupied);
    let bytes = pre.charged[key].bytes;
    sum_remove(pre.charged, key, pre.budget);
    let occupied = crate::budget_numeric::retire(pre.occupied, bytes);
    let available = crate::budget_numeric::credit(pre.available, bytes);
    Ledger { occupied, available,
        charged: pre.charged.remove(key), ..pre }
}
}
