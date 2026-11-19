// Appended to fresh source-generated state, byte-span and numeric proofs.
// Callback replies are unconstrained Results indexed by each visited position;
// retain/dedup/extraction and splice fill call the original runtime cursors.
// Staging composes the executable batch driver with checked reservation and fill callers. Physical ownership is checked separately.
verus! {
fn observed_element(cap: usize, size: usize, index: usize) -> (r: usize)
    requires index < cap, byte_valid(cap, size),
    ensures r == index,
{
    proof { memory_span(cap, size, index as int, 1); }
    index
}

fn retained_loop<F: Fn(usize) -> Result<bool, ()>>(
    len: usize, cap: usize, size: usize, cb: F) -> (r: (bool, usize))
    requires len <= cap, byte_valid(cap, size),
        forall|i: usize| call_requires(cb, (i,)),
    ensures r.1 <= len, r.1 <= cap,
{
        let mut state=c_full(len);
    let result=crate::vec::compaction_driver::run::<false,(),_,_>(
        &mut state,len,
        |read:usize,write:usize|
            requires read<len,write<=read,
        {
            let index=observed_element(cap,size,read);
            cb(index)
        },
        |read:usize,write:usize,keep:bool|
            requires read<len,write<=read,keep ==> write<read,
        {
            proof {
                memory_span(cap,size,read as int,1);
                if keep {
                    memory_span(cap,size,write as int,1);
                    ordered_elements_do_not_overlap(size,write,1,read);
                }
            }
        },
    );
    let length=if result.is_ok(){c_written(&state)}else{c_restored_len(&state)};
    (result.is_ok(),length)

}

fn compared<F: Fn(usize, usize) -> Result<bool, ()>>(
    cb: &F, later: usize, previous: usize) -> (r: Result<bool, ()>)
    requires later != previous,
        forall|i: usize,j: usize| call_requires(*cb, (i,j)),
{
    cb(later, previous)
}

fn dedup_loop<F: Fn(usize, usize) -> Result<bool, ()>>(
    len: usize, cap: usize, size: usize, cb: F) -> (r: (bool, usize))
    requires len <= cap, byte_valid(cap, size),
        forall|i: usize,j: usize| call_requires(cb, (i,j)),
    ensures r.1 <= len, r.1 <= cap,
{
        let mut state=c_first_kept(len);
    let result=crate::vec::compaction_driver::run::<true,(),_,_>(
        &mut state,len,
        |read:usize,write:usize|
            requires 0<write<=read<len,
        {
            let previous=crate::arithmetic::bounded::wrapping_sub(write,1);
            let later=observed_element(cap,size,read);
            let previous=observed_element(cap,size,previous);
            compared(&cb,later,previous).map(|duplicate| !duplicate)
        },
        |read:usize,write:usize,keep:bool|
            requires read<len,write<=read,keep ==> write<read,
        {
            proof {
                memory_span(cap,size,read as int,1);
                if keep {
                    memory_span(cap,size,write as int,1);
                    ordered_elements_do_not_overlap(size,write,1,read);
                }
            }
        },
    );
    let length=if result.is_ok(){c_written(&state)}else{c_restored_len(&state)};
    (result.is_ok(),length)

}

fn extracted_element(s: &E, cap: usize, size: usize, index: usize) -> (r: usize)
    requires ev(*s), s@.total <= cap, byte_valid(cap, size),
        index as int + 1 == s@.scan.read as int, s@.scan.write <= index,
    ensures r == index, r < s@.total,
{
    proof { memory_span(cap, size, index as int, 1); }
    assert(!live(s@.scan.write, s@.scan.read, s@.total, index as int));
    index
}

// Codes: 0 exhausted, 1 transferred element, 2 predicate error and stopped scan.
fn extract_next_loop<F: Fn(usize) -> Result<bool, ()>>(
    state: &mut E, cap: usize, size: usize, cb: F) -> (r: (u8, usize))
    requires ev(*old(state)), old(state)@.total <= cap, byte_valid(cap,size),
        forall|i:usize| call_requires(cb,(i,)),
    ensures ev(*final(state)), final(state)@.total == old(state)@.total,
        final(state)@.scan.read >= old(state)@.scan.read,
        r.0 == 1 ==> r.1 < final(state)@.total
            && r.1 as int + 1 == final(state)@.scan.read as int,
        r.0 != 1 ==> final(state)@.scan.read == final(state)@.scan.original,
{
    let result=crate::vec::compaction_driver::extract_next::<(),_,_>(
        state,
        |read:usize|
            requires read<cap,
        {
            let index=observed_element(cap,size,read);
            cb(index)
        },
        |read:usize,write:usize|
            requires write<read<cap,
        {
            proof {
                memory_span(cap,size,read as int,1);
                memory_span(cap,size,write as int,1);
                ordered_elements_do_not_overlap(size,write,1,read);
            }
        },
    );
    match result {
        Ok(Some(read))=>(1,extracted_element(&*state,cap,size,read)),
        Ok(None)=>(0,0),
        Err(_)=>{e_stop(state);(2,0)},
    }
}

fn splice_fill_loop<F: Fn(usize) -> Result<bool, ()>>(
    start:usize,tail_start:usize,cap:usize,size:usize,cb:F)->(r:(u8,usize))
    requires start<=tail_start<=cap,byte_valid(cap,size),
        forall|i:usize| call_requires(cb,(i,)),
    ensures start<=r.1<=tail_start,r.0==0 ==> r.1==tail_start,
{
    let mut state=crate::Checked::new(start);
    let result=crate::init::collection::finished(crate::init::driver::run(
        &mut state,tail_start,
        |index:usize|
            requires start<=index<tail_start,
        {
            let index=observed_element(cap,size,index);
            match cb(index) {
                Ok(true)=>Ok(()),
                Ok(false)=>Err(crate::init::collection::Stop::End),
                Err(())=>Err(crate::init::collection::Stop::Source(())),
            }
        },
    ));
    match result {
        Ok(true)=>(0,state.get()),
        Ok(false)=>(1,state.get()),
        Err(())=>(2,state.get()),
    }
}

fn staged_collection_loop<F:Fn(usize)->Result<bool,()>,A:Fn(usize)->bool>(
    available:usize,size:usize,cb:F,alloc:A)->(r:(u8,usize,usize))
    requires forall|i:usize| call_requires(cb,(i,)),
        forall|i:usize| call_requires(alloc,(i,)),
    ensures r.1<=r.2<=available,byte_valid(r.2,size),
{
    let initial_cap=if size==0 {available} else {0usize};
    let (result,len,cap)=crate::vec::staging_driver::run(
        (),available,size,initial_cap,
        |len:usize,cap:usize,_value:()|
            requires len<available,len<=cap<=available,byte_valid(cap,size),
        {
            let Some((needed,proposed))=memory_reserve(len,cap,available,size,1,false,alloc(len))
                else {return (Err(()),len,cap);};
            let _=observed_element(proposed,size,len);
            let (code,written)=splice_fill_loop(needed,proposed,proposed,size,|index:usize| cb(index));
            let reply=match code {
                0=>match cb(written) {Ok(true)=>Ok(Some(())),Ok(false)=>Ok(None),Err(())=>Err(())},
                1=>Ok(None),
                _=>Err(()),
            };
            (reply,written,proposed)
        },
    );
    match result {Ok(true)=>(0,len,cap),_=>(2,len,cap)}
}
}
