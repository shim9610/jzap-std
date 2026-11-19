// Logical accounting refinement. Keys denote the unique live allocations
// established by Rust ownership; they are not integer addresses or new runtime
// tokens. These proofs establish the numeric release entitlement and frames.
verus! {
ghost struct AllocationCredit { key:nat, policy:nat, bytes:usize }
spec fn owns(l:Ledger,c:AllocationCredit)->bool {
    l.wf() && c.policy==l.budget && l.charged.dom().contains(c.key)
        && l.charged[c.key].budget==c.policy && l.charged[c.key].bytes==c.bytes
}

proof fn allocation_credit(l:Ledger,key:nat,bytes:usize,accepted:bool)
    ->(r:(Ledger,Option<AllocationCredit>))
    requires l.wf(),bytes<=l.available,
        !l.pending.dom().contains(key),!l.charged.dom().contains(key),
    ensures r.0.wf(),r.0.budget==l.budget,r.0.limit==l.limit,
        accepted ==> r.1.is_some() && owns(r.0,r.1.unwrap())
            && r.1.unwrap().bytes==bytes && r.1.unwrap().key==key,
        !accepted ==> r.1.is_none() && r.0.charged==l.charged,
        r.0.charged.dom().contains(key) <==> accepted,
{
    let reserved=ledger_reserve(l,key,bytes);
    if !accepted {
        (ledger_cancel(reserved,key),None)
    }else{
        let charged=ledger_commit(reserved,key);
        (charged,Some(AllocationCredit{key,policy:l.budget,bytes}))
    }
}

proof fn preserved_credit(before:Ledger,after:Ledger,c:AllocationCredit)
    requires owns(before,c),after.wf(),before.budget==after.budget,
        after.charged.dom().contains(c.key),after.charged[c.key]==before.charged[c.key],
    ensures owns(after,c),c.bytes<=after.occupied,
{
    owned_credit_bound(after.charged,c.key,after.budget,after.occupied);
}

proof fn reallocation_credit(l:Ledger,old:AllocationCredit,key:nat,new_bytes:usize,accepted:bool)
    ->(r:(Ledger,AllocationCredit))
    requires owns(l,old),new_bytes<=l.available,
        !l.pending.dom().contains(key),!l.charged.dom().contains(key),
    ensures owns(r.0,r.1),r.0.budget==l.budget,r.0.limit==l.limit,
        accepted ==> r.1.key==key && r.1.bytes==new_bytes && !r.0.charged.dom().contains(old.key),
        !accepted ==> r.1==old && r.0.charged==l.charged,
{
    let reserved=ledger_reserve(l,key,new_bytes);
    if !accepted {
        (ledger_cancel(reserved,key),old)
    }else{
        let charged=ledger_commit(reserved,key);
        let released=ledger_release(charged,old.key);
        (released,AllocationCredit{key,policy:old.policy,bytes:new_bytes})
    }
}

proof fn same_size_reallocation(l:Ledger,c:AllocationCredit,new_bytes:usize)
    requires owns(l,c),new_bytes==c.bytes,
    ensures owns(l,c),new_bytes<=l.occupied,
{
    owned_credit_bound(l.charged,c.key,l.budget,l.occupied);
}

proof fn transferred_credit(l:Ledger,c:AllocationCredit,new_policy:nat,new_bytes:usize)
    ->(r:AllocationCredit)
    requires owns(l,c),new_policy==c.policy,new_bytes==c.bytes,
    ensures owns(l,r),r.key==c.key,r.policy==new_policy,r.bytes==new_bytes,
{
    AllocationCredit{key:c.key,policy:new_policy,bytes:new_bytes}
}

proof fn released_credit(l:Ledger,c:AllocationCredit)->(r:Ledger)
    requires owns(l,c),
    ensures r.wf(),!r.charged.dom().contains(c.key),
        r.budget==l.budget,r.limit==l.limit,
        c.bytes<=l.occupied,
{
    owned_credit_bound(l.charged,c.key,l.budget,l.occupied);
    ledger_release(l,c.key)
}

// Any other live allocation may be released during a user callback. The
// current owner's entitlement survives because the keys are distinct.
proof fn release_other_owner(l:Ledger,c:AllocationCredit,other:AllocationCredit)->(r:Ledger)
    requires owns(l,c),owns(l,other),c.key!=other.key,
    ensures owns(r,c),r.wf(),r.budget==l.budget,r.limit==l.limit,
{
    ledger_release(l,other.key)
}
}
