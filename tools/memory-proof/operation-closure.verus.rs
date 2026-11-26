// Compositions of the source-derived kernels. Payload/reference semantics are
// checked by the existing actual-source memory suites, rather than assumed to
// follow from a scalar model. Every callback/allocator reply is unconstrained.
verus! {
#[derive(Clone,Copy)]
struct Hole { len:usize, tail:usize, tail_len:usize, cap:usize, limit:usize }
spec fn hole_valid(h:Hole,size:usize)->bool {
    h.len<=h.tail && h.tail as int+h.tail_len as int<=h.cap as int
        && h.cap<=h.limit && byte_valid(h.cap,size)
}

fn hole_available(h:Hole,size:usize)->(r:usize)
    requires hole_valid(h,size),
    ensures r as int==h.limit as int-h.len as int-h.tail_len as int,
{
    HOLE_AVAILABLE
}

fn hole_reserve(h:&mut Hole,size:usize,additional:usize,exact:bool,accepted:bool)->(ok:bool)
    requires hole_valid(*old(h),size),
    ensures hole_valid(*final(h),size),final(h).len==old(h).len,
        final(h).tail==old(h).tail,final(h).tail_len==old(h).tail_len,
        final(h).limit==old(h).limit,final(h).cap>=old(h).cap,
        ok ==> final(h).len as int+additional as int<=final(h).cap as int,
{
    let Some((needed,cap))=memory_reserve(h.len,h.cap,h.limit,size,additional,exact,accepted)
        else{return false;};
    h.cap=cap;
    true
}

fn hole_move(h:&mut Hole,size:usize,additional:usize,accepted:bool)->(ok:bool)
    requires hole_valid(*old(h),size),old(h).len==old(h).tail,
    ensures hole_valid(*final(h),size),final(h).len==old(h).len,
        final(h).tail_len==old(h).tail_len,final(h).limit==old(h).limit,
        final(h).cap>=old(h).cap,
        ok ==> final(h).tail as int==old(h).tail as int+additional as int,
        !ok ==> final(h).tail==old(h).tail,
{
    HOLE_MOVE
}

fn hole_fill<F:Fn(usize)->Result<bool,()>>(h:&mut Hole,size:usize,cb:&F)->(r:u8)
    requires hole_valid(*old(h),size),forall|i:usize| call_requires(*cb,(i,)),
    ensures hole_valid(*final(h),size),old(h).len<=final(h).len<=old(h).tail,
        final(h).tail==old(h).tail,final(h).tail_len==old(h).tail_len,
        final(h).cap==old(h).cap,final(h).limit==old(h).limit,
        r==0 ==> final(h).len==final(h).tail,
{
    let (code,len)=splice_fill_loop(h.len,h.tail,h.cap,size,|i:usize| cb(i));
    h.len=len;code
}

fn hole_push(h:&mut Hole,size:usize,accepted:bool)->(ok:bool)
    requires hole_valid(*old(h),size),old(h).tail_len==0,
    ensures hole_valid(*final(h),size),final(h).tail_len==0,
        final(h).limit==old(h).limit,final(h).len>=old(h).len,
        ok ==> final(h).len as int==old(h).len as int+1,
{
    let Some(next)=h.len.checked_add(1) else{return false;};
    if !hole_reserve(h,size,1,false,accepted){return false;}
    let _=observed_element(h.cap,size,h.len);
    h.len=next;h.tail=next;
    true
}

fn hole_restore(h:Hole,size:usize)->(r:OwnerState)
    requires hole_valid(h,size),
    ensures owner_valid(r,size),r.cap==h.cap,r.limit==h.limit,
        r.len as int==h.len as int+h.tail_len as int,
{
    drain_before_tail_restore(h.len,h.tail,h.tail_len,h.cap,size);
    let len=HOLE_RESTORE;
    OwnerState{len,cap:h.cap,limit:h.limit}
}

fn stage_commit(h:&mut Hole,size:usize,extra_len:usize,extra_cap:usize,
    initial_capacity:usize,accepted:bool)->(ok:bool)
    requires hole_valid(*old(h),size),old(h).len==old(h).tail,
        extra_len<=extra_cap,byte_valid(extra_cap,size),
        extra_len as int+old(h).len as int+old(h).tail_len as int<=old(h).limit as int,
    ensures hole_valid(*final(h),size),final(h).tail_len==old(h).tail_len,
        final(h).limit==old(h).limit,final(h).cap>=old(h).cap,
        final(h).len==final(h).tail,
        ok ==> final(h).len as int==old(h).len as int+extra_len as int,
        !ok ==> final(h).len==old(h).len,
{
    STAGE_COMMIT
}

fn stage_operation<F:Fn(usize)->Result<bool,()>,A:Fn(usize)->bool>(
    h:&mut Hole,size:usize,initial_capacity:usize,cb:&F,alloc:&A,commit_accepts:bool)->(ok:bool)
    requires hole_valid(*old(h),size),old(h).len==old(h).tail,
        forall|i:usize| call_requires(*cb,(i,)),
        forall|i:usize| call_requires(*alloc,(i,)),
    ensures hole_valid(*final(h),size),final(h).tail_len==old(h).tail_len,
        final(h).limit==old(h).limit,final(h).len==final(h).tail,
{
    let available=hole_available(*h,size);
    let (code,len,cap)=staged_collection_loop(available,size,|i:usize| cb(i),|i:usize| alloc(i));
    if code!=0 {return false;}
    stage_commit(h,size,len,cap,initial_capacity,commit_accepts)
}

fn splice_operation<F:Fn(usize)->Result<bool,()>,A:Fn(usize)->bool>(
    h:&mut Hole,size:usize,cb:&F,alloc:&A,hint:usize,hint_ok:bool,
    move_accepts:bool,stage_accepts:bool)->(ok:bool)
    requires hole_valid(*old(h),size),forall|i:usize| call_requires(*cb,(i,)),
        forall|i:usize| call_requires(*alloc,(i,)),
    ensures hole_valid(*final(h),size),final(h).tail_len==old(h).tail_len,
        final(h).limit==old(h).limit,
{
    SPLICE_OPERATION
}

fn drain_constructor(len:usize,cap:usize,limit:usize,size:usize,start:usize,end:usize)
    ->(r:Option<Hole>)
    requires len<=cap<=limit,byte_valid(cap,size),
    ensures match r {Some(h)=>hole_valid(h,size) && h.len==start && h.tail==end
        && h.cap==cap && h.limit==limit && h.len as int+h.tail_len as int<=len as int,None=>true},
{
    if !normalized_range(start,end,len){return None;}
    let tail_len=DRAIN_TAIL;
    Some(Hole{len:DRAIN_LENGTH,tail:DRAIN_START,tail_len,cap,limit})
}

fn initialized_operation<F:Fn(usize)->u8>(s:OwnerState,size:usize,target:usize,cb:&F,accepted:bool)
    ->(r:(bool,OwnerState))
    requires owner_valid(s,size),forall|i:usize| call_requires(*cb,(i,)),
    ensures owner_valid(r.1,size),r.1.limit==s.limit,
        r.0 ==> r.1.len==target,
        !r.0 ==> r.1.len==s.len,
{
    INITIALIZED_OPERATION
}

fn buffer_push(len:usize,cap:usize,size:usize,hint:usize,accepted:bool)
    ->(r:Option<(usize,usize)>)
    requires len<=cap,byte_valid(cap,size),
    ensures match r {Some((l,c))=>l as int==len as int+1 && l<=c && cap<=c
        && byte_valid(c,size),None=>true},
{
    let Some((needed,next_cap))=buffer_reserve(len,cap,size,1,hint,accepted) else{return None;};
    let _=observed_element(next_cap,size,len);
    Some((needed,next_cap))
}

fn buffer_span<F:Fn(usize)->Result<bool,()>>(initial:usize,cap:usize,size:usize,count:usize,cb:&F)
    ->(r:(u8,usize))
    requires initial<=cap,byte_valid(cap,size),count<=cap-initial,
        forall|i:usize| call_requires(*cb,(i,)),
    ensures initial<=r.1<=cap,
{
    let (reply,len)=collection_cursor(initial,initial+count,cb);
    let code=match reply {Ok(true)=>0u8,Ok(false)=>1u8,Err(_)=>2u8};
    proof {memory_span(cap,size,initial as int,len as int-initial as int);}
    (code,len)
}

fn buffer_collect_rest<F:Fn(usize)->Result<bool,()>,A:Fn(usize)->bool>(
    initial:usize,initial_cap:usize,size:usize,hint:usize,cb:&F,alloc:&A)->(r:(bool,usize,usize))
    requires initial<=initial_cap,byte_valid(initial_cap,size),
        forall|i:usize| call_requires(*cb,(i,)),forall|i:usize| call_requires(*alloc,(i,)),
    ensures initial<=r.1<=r.2,byte_valid(r.2,size),
{
    BUFFER_REST
}

fn buffer_collect<F:Fn(usize)->Result<bool,()>,A:Fn(usize)->bool>(
    initial:usize,initial_cap:usize,size:usize,hint:usize,cb:&F,alloc:&A)->(r:(bool,usize,usize))
    requires initial<=initial_cap,byte_valid(initial_cap,size),
        forall|i:usize| call_requires(*cb,(i,)),forall|i:usize| call_requires(*alloc,(i,)),
    ensures initial<=r.1<=r.2,byte_valid(r.2,size),
{
    BUFFER_COLLECT
}

fn buffer_finish(len:usize,cap:usize,size:usize,accepted:bool)->(r:Option<OwnerState>)
    requires len<=cap,byte_valid(cap,size),
    ensures match r {Some(s)=>owner_valid(s,size) && s.len==len && s.cap==len,None=>true},
{
    if cap!=len && size>0 && !accepted{return None;}
    proof {memory_span(cap,size,0,len as int);}
    Some(OwnerState{len,cap:len,limit:usize::MAX})
}

fn resized_owner(s:OwnerState,size:usize,capacity:usize)->(r:OwnerState)
    requires owner_valid(s,size),s.len<=capacity<=s.limit,byte_valid(capacity,size),
    ensures owner_valid(r,size),r.len==s.len,r.cap==capacity,r.limit==s.limit,
{ OwnerState{cap:capacity,..s} }

fn shortened_owner(s:OwnerState,size:usize,target:usize)->(r:OwnerState)
    requires owner_valid(s,size),
    ensures owner_valid(r,size),r.len==(if s.len<target{s.len}else{target}),r.cap==s.cap,r.limit==s.limit,
{
    if target>=s.len{return s;}
    proof {memory_span(s.cap,size,target as int,s.len as int-target as int);}
    OwnerState{len:target,..s}
}

fn boxed_to_owner(len:usize,limit:usize,size:usize)->(r:OwnerState)
    requires len<=limit,byte_valid(len,size),
    ensures owner_valid(r,size),r.len==len,r.limit==limit,
        r.cap as int*size as int==len as int*size as int,
{
    let cap=crate::capacity::logical_capacity(size,len,limit);
    assert(cap as int*size as int==len as int*size as int) by(nonlinear_arith)
        requires size==0 || cap==len;
    OwnerState{len,cap,limit}
}

fn owner_to_array(s:OwnerState,size:usize,count:usize)->(r:bool)
    requires owner_valid(s,size),
    ensures r ==> count==s.len && byte_valid(count,size),
{
    if s.len!=count{return false;}
    proof {memory_span(s.cap,size,0,count as int);}
    true
}

fn prefix_storage_connection<F:Fn(usize)->u8>(target:usize,capacity:usize,size:usize,cb:&F)
    ->(r:(bool,usize))
    requires target<=capacity,byte_valid(capacity,size),forall|i:usize| call_requires(*cb,(i,)),
    ensures r.1<=target,r.0 ==> r.1==target,
{
    let mut prefix=crate::Checked::new(0usize);
    let result=crate::init::driver::run(&mut prefix,target,|index:usize| -> Result<(),()> {
        if cb(index)==1 {Ok(())} else {Err(())}
    });
    proof {memory_span(capacity,size,0,prefix@ as int);}
    (result.is_ok(),prefix.get())
}

fn extend_operation<F:Fn(usize)->Result<bool,()>,A:Fn(usize)->bool>(
    initial:OwnerState,size:usize,cb:&F,alloc:&A)->(r:(bool,OwnerState))
    requires owner_valid(initial,size),forall|i:usize| call_requires(*cb,(i,)),
        forall|i:usize| call_requires(*alloc,(i,)),
    ensures owner_valid(r.1,size),r.1.limit==initial.limit,
        !r.0 ==> r.1.len==initial.len,
{
    let mut state=initial;
    while state.len<usize::MAX
        invariant owner_valid(state,size),initial.len<=state.len,state.limit==initial.limit,
            forall|i:usize| call_requires(*cb,(i,)),forall|i:usize| call_requires(*alloc,(i,)),
        decreases usize::MAX-state.len,
    {
        let present=match cb(state.len){Ok(v)=>v,Err(_)=>{state.len=initial.len;return (false,state);}};
        if !present{return (true,state);}
        let Some(next)=commit_reserved_owner(state,size,1,alloc(state.len)) else{
            state.len=initial.len;return (false,state);
        };
        state=next;
    }
    let present=match cb(state.len){Ok(v)=>v,Err(_)=>{state.len=initial.len;return (false,state);}};
    if !present{return (true,state);}
    let next=commit_reserved_owner(state,size,1,alloc(state.len));assert(next.is_none());
    state.len=initial.len;(false,state)
}

fn complete_splice<F:Fn(usize)->Result<bool,()>,A:Fn(usize)->bool>(
    original:OwnerState,size:usize,start:usize,end:usize,cb:&F,alloc:&A,
    hint:usize,hint_ok:bool,move_accepts:bool,stage_accepts:bool)->(r:Option<(bool,OwnerState)>)
    requires owner_valid(original,size),forall|i:usize| call_requires(*cb,(i,)),
        forall|i:usize| call_requires(*alloc,(i,)),
    ensures match r {Some((ok,s))=>owner_valid(s,size) && s.limit==original.limit,None=>true},
{
    let Some(mut h)=drain_constructor(original.len,original.cap,original.limit,size,start,end) else{return None;};
    let result=splice_operation(&mut h,size,cb,alloc,hint,hint_ok,move_accepts,stage_accepts);
    Some((result,hole_restore(h,size)))
}

fn empty_buffer(size:usize)->(s:OwnerState)
    ensures owner_valid(s,size),s.len==0,s.cap==0,
{ OwnerState{len:0,cap:0,limit:usize::MAX} }

fn array_import(count:usize,limit:usize,size:usize,accepted:bool)->(s:Option<OwnerState>)
    ensures match s {Some(s)=>owner_valid(s,size) && s.len==count && s.limit==limit,None=>true},
{
    let empty=empty_owner(limit,size);
    let Some((mut s,needed))=reserve_owner(empty,size,count,true,accepted) else{return None;};
    proof {memory_span(s.cap,size,0,count as int);}
    s.len=needed;Some(s)
}

// A whole-value replacement publishes exactly the replacement, whatever the
// replaced value was. The validity of each value is its own type's concern.
fn replaced_owner(old:OwnerState,replacement:OwnerState,size:usize)->(s:OwnerState)
    ensures s.len==replacement.len,s.cap==replacement.cap,s.limit==replacement.limit,
{ replacement }
}
