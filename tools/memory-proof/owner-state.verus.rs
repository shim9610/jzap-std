// Source-connected numeric owner construction, reservation and conversion.
// Allocation uniqueness and actual reference semantics are separate contracts.
verus! {
// Copyable numeric descriptor; unique payload ownership is checked separately.
#[derive(Clone,Copy)]
struct OwnerState { len:usize, cap:usize, limit:usize }
spec fn owner_valid(s:OwnerState,size:usize)->bool {
    s.len <= s.cap <= s.limit && byte_valid(s.cap,size)
}

fn empty_owner(limit:usize,size:usize)->(r:OwnerState)
    ensures owner_valid(r,size),r.len==0,r.limit==limit,
{
    let len=0usize;
    let cap=crate::capacity::logical_capacity(size,0,limit);
    OwnerState { len,cap,limit }
}

// The public unsafe contract of `Vec::from_raw_parts`: the caller promises the
// three words. No library proof establishes a caller's promise.
fn raw_parts_owner(len:usize,cap:usize,limit:usize,size:usize)->(r:OwnerState)
    requires len<=cap<=limit,byte_valid(cap,size),
    ensures owner_valid(r,size),r.len==len,r.cap==cap,r.limit==limit,
{
    OwnerState { len,cap,limit }
}

fn reserve_owner(s:OwnerState,size:usize,additional:usize,exact:bool,accepted:bool)
    ->(r:Option<(OwnerState,usize)>)
    requires owner_valid(s,size),
    ensures match r {
        Some((next,needed))=>owner_valid(next,size) && next.len==s.len && next.limit==s.limit
            && needed as int==s.len as int+additional as int && needed<=next.cap,
        None=>true,
    },
{
    let Some((needed,cap))=memory_reserve(s.len,s.cap,s.limit,size,additional,exact,accepted)
        else { return None; };
    Some((OwnerState{cap,..s},needed))
}

fn commit_reserved_owner(s:OwnerState,size:usize,additional:usize,accepted:bool)
    ->(r:Option<OwnerState>)
    requires owner_valid(s,size),
    ensures match r {Some(next)=>owner_valid(next,size) && next.limit==s.limit
        && next.len as int==s.len as int+additional as int,None=>true},
{
    let Some((mut next,needed))=reserve_owner(s,size,additional,false,accepted) else{return None;};
    proof {memory_span(next.cap,size,next.len as int,additional as int);}
    next.len=RESERVED_COMMIT;
    Some(next)
}

fn checked_owner_length(s:OwnerState,size:usize,new_len:usize)->(r:Option<OwnerState>)
    requires owner_valid(s,size),
    ensures match r {Some(next)=>owner_valid(next,size) && next.len==new_len
        && next.cap==s.cap && next.limit==s.limit,None=>new_len>s.cap},
{
    if crate::vec::required_len(0,new_len,s.cap).is_err() {return None;}
    Some(OwnerState{len:new_len,..s})
}

fn buffer_reserve(len:usize,cap:usize,size:usize,additional:usize,hint:usize,accepted:bool)
    ->(r:Option<(usize,usize)>)
    requires len<=cap,byte_valid(cap,size),
    ensures match r {Some((needed,next_cap))=>needed as int==len as int+additional as int
        && len<=needed<=next_cap && cap<=next_cap && byte_valid(next_cap,size),None=>true},
{
    let Ok((needed,proposed))=crate::boxed::buffer_numeric::reservation(len,cap,additional,hint)
        else{return None;};
    if needed<=cap {return Some((needed,cap));}
    let Some(bytes)=checked_array_bytes(proposed,size) else{return None;};
    if bytes>0 && !accepted {return None;}
    Some((needed,proposed))
}

fn append_owner(destination:OwnerState,source:OwnerState,size:usize,accepted:bool)
    ->(r:Option<(OwnerState,OwnerState)>)
    requires owner_valid(destination,size),owner_valid(source,size),
    ensures match r {Some((a,b))=>owner_valid(a,size) && owner_valid(b,size)
        && a.len as int==destination.len as int+source.len as int && b.len==0
        && a.limit==destination.limit && b.cap==source.cap && b.limit==source.limit,None=>true},
{
    let Some(mut next)=commit_reserved_owner(destination,size,source.len,accepted) else{return None;};
    let mut other=source;
    other.len=APPEND_SOURCE_COMMIT;
    Some((next,other))
}

fn split_owner(original:OwnerState,size:usize,at:usize,accepted:bool)
    ->(r:Option<(OwnerState,OwnerState)>)
    requires owner_valid(original,size),
    ensures match r {Some((a,b))=>owner_valid(a,size)&&owner_valid(b,size)
        && a.len==at && a.len as int+b.len as int==original.len as int
        && a.limit==original.limit && b.limit==original.limit,None=>true},
{
    if SPLIT_OWNER_REJECT {return None;}
    let Some(count)=original.len.checked_sub(at) else{return None;};
    let tail=empty_owner(original.limit,size);
    let Some((mut tail,needed))=reserve_owner(tail,size,count,true,accepted) else{return None;};
    proof {memory_span(original.cap,size,at as int,count as int);memory_span(tail.cap,size,0,count as int);}
    tail.len=SPLIT_TAIL_COMMIT;
    Some((OwnerState{len:SPLIT_PREFIX_COMMIT,..original},tail))
}

// `Vec::try_reuse_larger_buffer`: the storage words (pointer, length and
// capacity) of two owners are exchanged by three `mem::swap` calls; each owner
// keeps its policy and its limit. The refusals and the reservation in the
// other owner are the product's (its body is compared whole by
// caller_bindings.py). `declined` stands for the two accounting conditions,
// which do not constrain the words. Both owners are stated valid on entry.
fn exchanged_owners(a:OwnerState,b:OwnerState,size:usize,declined:bool,accepted:bool)
    ->(r:Option<(OwnerState,OwnerState)>)
    requires owner_valid(a,size),owner_valid(b,size),
    ensures match r {Some((x,y))=>owner_valid(x,size) && owner_valid(y,size)
        && x.len==b.len && x.limit==a.limit
        && y.len==a.len && y.cap==a.cap && y.limit==b.limit
        && x.len as int+y.len as int<=x.cap as int,None=>true},
{
    if a.len>=b.len || a.limit!=b.limit || declined {return None;}
    let Ok(needed)=needed_length(a.len,b.len,a.limit) else {return None;};
    let mut other=b;
    if other.cap<needed {
        if a.cap>=needed {return None;}
        let Some((reserved,_))=reserve_owner(other,size,a.len,true,accepted) else {return None;};
        other=reserved;
    }
    Some((OwnerState{len:other.len,cap:other.cap,..a},OwnerState{len:a.len,cap:a.cap,..other}))
}

// The byte bound of the adopted capacity is not a premise: the product checks
// it (`Capacity::try_new` in `Vec::try_from_std_in`), as this function does.
fn adopted_owner(source_len:usize,source_cap:usize,limit:usize,size:usize,
    budget_limit:usize,occupied:usize,available:usize,metered:bool)
    ->(r:Option<(OwnerState,usize,usize)>)
    requires source_len<=source_cap,
        occupied as int+available as int<=budget_limit as int,
    ensures match r {Some((owner,o,a))=>owner_valid(owner,size) && owner.len==source_len
        && owner.limit==limit && o as int+a as int<=budget_limit as int
        && (metered ==> owner.cap as int*size as int<=o as int),None=>true},
{
    let Ok(cap)=crate::capacity::adopted_capacity(size,source_len,source_cap,limit) else{return None;};
    let Some(bytes)=checked_array_bytes(cap,size) else{return None;};
    let (o,a)=if metered {
        let Some(a)=reserve(budget_limit,occupied,available,bytes) else{return None;};
        (commit(budget_limit,occupied,a,bytes),a)
    }else{(occupied,available)};
    Some((OwnerState{len:source_len,cap,limit},o,a))
}

fn owner_release(owner:OwnerState,size:usize,budget_limit:usize,occupied:usize,available:usize)
    ->(r:(usize,usize))
    requires owner_valid(owner,size),occupied as int+available as int<=budget_limit as int,
        owner.cap as int*size as int<=occupied as int,
    ensures r.0 as int+r.1 as int<=budget_limit as int,
        r.0 as int==occupied as int-owner.cap as int*size as int,
{
    let bytes=capacity_bytes(owner.cap,size);
    release(budget_limit,occupied,available,bytes)
}

spec fn piece_sum(parts:Seq<usize>,n:int)->int
    decreases if n<0 {0int}else{n},
{
    if n<=0 || n>parts.len() {0int}
    else {piece_sum(parts,n-1)+parts[n-1] as int}
}
proof fn piece_sum_monotone(parts:Seq<usize>,left:int,right:int)
    requires 0<=left<=right<=parts.len(),
    ensures piece_sum(parts,left)<=piece_sum(parts,right),
    decreases right-left,
{
    if left<right {piece_sum_monotone(parts,left,right-1);}
}

fn vectored_owner(s:OwnerState,parts:&[usize],accepted:bool)
    ->(r:Option<(OwnerState,usize)>)
    requires owner_valid(s,1),
    ensures match r {Some((next,total))=>owner_valid(next,1) && next.limit==s.limit
        && next.len as int==s.len as int+total as int
        && total as int==piece_sum(parts@,parts.len() as int),None=>true},
{
    VECTORED_BODY
}
}
