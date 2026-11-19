use vstd::prelude::*;
verus! {
// Raw strong word = owned live credits + rejected reservations not yet rolled
// back. This proof is conditional on the outstanding-reservation bound; it does
// not by itself establish the platform's native execution bound.
spec fn buffered_valid(live:int,pending:int,maximum:int)->bool {
    0<maximum<=isize::MAX as int && 0<=live<=maximum &&
    0<=pending<maximum && (pending>0 ==> live>0)
}
proof fn buffered_reservation_extent(live:int,pending:int,maximum:int)
    requires buffered_valid(live,pending,maximum), live>0,
    ensures 0<live+pending+1<=2*maximum<=usize::MAX as int,
{}
proof fn buffered_admission(live:int,pending:int,maximum:int)
    requires buffered_valid(live,pending,maximum),live>0,live+pending<maximum,
    ensures buffered_valid(live+1,pending,maximum),live+1<=maximum,
{}
proof fn buffered_rejection_and_rollback(live:int,pending:int,maximum:int)
    requires buffered_valid(live,pending,maximum),live>0,
        live+pending>=maximum,pending+1<maximum,
    ensures buffered_valid(live,pending+1,maximum),
        (live+(pending+1))-1==live+pending,
{}
// Other owners may change between our increment and our rollback. Subtract
// only our one reservation from the CURRENT word, never restore a stale word.
proof fn buffered_rollback_preserves_intervening_credits(live:int,others:int,maximum:int)
    requires buffered_valid(live,others+1,maximum),others>=0,
    ensures buffered_valid(live,others,maximum),
        live+others+1-1==live+others,live>0,
{}
proof fn buffered_zero_is_final(live:int,pending:int,maximum:int)
    requires buffered_valid(live,pending,maximum), live+pending==1,
    ensures live==1,pending==0,
{}
proof fn buffered_reservation_pins_an_owner(live:int,pending:int,maximum:int)
    requires buffered_valid(live,pending,maximum),pending>0,
    ensures live+pending>=2,
{}
// Linux x86-64 (four/five-level paging) native ABI refinement. Kernel task and
// virtual-address bounds are platform axioms, not claims that Verus proved the
// Linux kernel or arbitrary foreign context-switching assembly.
proof fn buffered_linux_native_headroom(live:int,running:int,suspended:int)
    requires 9223372036854775807<=isize::MAX as int,
        1<=live<=9223372036854775807,
        0<=running<=2147483647,0<=suspended<72057594037927936,
    ensures buffered_valid(live,running+suspended,9223372036854775807),
        live+running+suspended+1<18446744073709551616,
{}
// Retired weak flags and the transient strong guard are different state
// variables. Reserving one weak count keeps flag|credits away from LOCK.
proof fn shared_flagged_weak_extent(credits:int,tag:int)
    requires 1<=credits<tag-1,tag>1,
    ensures tag< tag+credits < 2*tag-1,
{}
// Ghost credits model exactly the two private counters and lifecycle phases.
// 0 constructing, 1 live, 2 payload destruction, 3 retired (weak-only).
spec fn shared_valid(strong:int, weak:int, phase:int, maximum:int)->bool {
    0<maximum<=isize::MAX as int && 0<=strong<=maximum && 0<=weak<=maximum &&
    ((phase==0 && strong==0 && weak>=1) ||
     (phase==1 && strong>=1 && weak>=1) ||
     (phase==2 && strong==0 && weak>=1) ||
     (phase==3 && strong==0))
}
// Construction exposes its one weak credit. During payload destruction the
// implicit weak is private; in the retired phase every surviving weak is public.
spec fn shared_weak_creator(strong:int,weak:int,phase:int)->bool {
    phase==0 || strong>0 || (if phase==3 {weak>0} else {weak>1})
}
proof fn shared_sole_weak_retirement(weak:int,maximum:int)
    requires shared_valid(0,weak,2,maximum),weak<maximum,
    ensures weak==1 ==> !shared_weak_creator(0,weak,2),
        0<maximum+1+weak<2*(maximum+1)-1,
{}
proof fn shared_count_credit(strong:int,weak:int,phase:int,maximum:int)
    requires shared_valid(strong,weak,phase,maximum),phase==1,strong<maximum,
    ensures shared_valid(strong+1,weak,phase,maximum),strong+1<=usize::MAX as int,
{}
proof fn shared_saturation_preserves(strong:int,weak:int,phase:int,maximum:int)
    requires shared_valid(strong,weak,phase,maximum),strong==maximum,
    ensures phase==1, shared_valid(strong,weak,phase,maximum),
{}
proof fn shared_weak_credit(strong:int,weak:int,phase:int,maximum:int)
    requires shared_valid(strong,weak,phase,maximum),weak>0,weak<maximum,
        shared_weak_creator(strong,weak,phase),
    ensures shared_valid(strong,weak+1,phase,maximum),weak+1<=usize::MAX as int,
{}
proof fn shared_upgrade(strong:int,weak:int,phase:int,maximum:int)
    requires shared_valid(strong,weak,phase,maximum),strong>0,strong<maximum,
    ensures phase==1,shared_valid(strong+1,weak,phase,maximum),
{}
proof fn shared_release_credit(strong:usize)
    requires strong>0,
    ensures strong as int-1>=0,strong as int-1< strong,
{}
proof fn shared_nonfinal_strong(strong:int,weak:int,maximum:int)
    requires shared_valid(strong,weak,1,maximum),strong>1,
    ensures shared_valid(strong-1,weak,1,maximum),
{}
proof fn shared_last_strong(weak:int,maximum:int)
    requires shared_valid(1,weak,1,maximum),
    ensures shared_valid(0,weak,2,maximum),weak>=1,
{}
proof fn shared_last_weak(weak:int,maximum:int)
    requires shared_valid(0,weak,2,maximum),
    ensures shared_valid(0,weak-1,3,maximum),weak-1==0 <==> weak==1,
{}
proof fn shared_explicit_weak_release(strong:int,weak:int,phase:int,maximum:int)
    requires shared_valid(strong,weak,phase,maximum),
        if phase==3 {weak>0} else {weak>1},
    ensures shared_valid(strong,weak-1,phase,maximum),weak-1>=0,
{}
proof fn shared_cyclic_publish(weak:int,maximum:int)
    requires shared_valid(0,weak,0,maximum),
    ensures shared_valid(1,weak,1,maximum),
{}
proof fn shared_cyclic_refusal(weak:int,maximum:int)
    requires shared_valid(0,weak,0,maximum),
    ensures shared_valid(0,weak-1,3,maximum),
{}
proof fn shared_unique_access(strong:int,weak:int,maximum:int)
    requires shared_valid(strong,weak,1,maximum),strong==1,weak==1,
    ensures strong-1==0,weak-1==0,
{}
proof fn shared_cow_retirement(weak:int,maximum:int)
    requires shared_valid(1,weak,1,maximum),
    ensures shared_valid(0,weak-1,3,maximum),shared_valid(1,1,1,maximum),
{}
proof fn shared_typed_transfer(strong:int,weak:int,phase:int,maximum:int)
    requires shared_valid(strong,weak,phase,maximum),
    ensures shared_valid(strong,weak,phase,maximum),
{}
// Actual Layout::extend admission is separately checked by native Kani.
proof fn shared_layout_extent(header:usize,offset:usize,payload:usize,total:usize)
    requires header<=offset,offset as int+payload as int<=total,total<=isize::MAX as usize,
    ensures offset<=total, payload<=total,offset as int+payload as int<=isize::MAX as int,
{}
// Slice payloads: `<[T]>::clone_into` and `try_init_slice_in` call
// `init::prefix::initialize(ptr, length, callback)`. This runs the product
// driver itself, so `index < length` in the callback is the driver's verified
// call condition. `initialize_one` forms `ptr.add(index)`; the clone callback
// reads `get_unchecked(index)` from a source of the same `length` elements.
fn shared_prefix_driver(p:RawStorage,source:RawStorage,length:usize)
    requires raw_valid(p),p.cap==length,raw_valid(source),source.cap==length,
{
    let mut state=crate::Checked::new(0usize);
    let _=crate::init::driver::run(&mut state,length,|index:usize| -> (out:Result<(),()>)
        requires index<length,raw_valid(p),p.cap==length,raw_valid(source),source.cap==length,
    {
        // `ptr.add(index)` and the slot written there; `self.get_unchecked(index)`.
        let _place=offset_storage(p,index);
        let _slot=slice_caller(p,index,1);
        let _read=initialized_caller(source,index,length);
        Ok(())
    });
}
proof fn shared_payload_extent(offset:usize,payload:usize,total:usize)
    requires offset as int+payload as int<=total,total<=isize::MAX as usize,
    ensures offset<=total,offset as int+payload as int<=isize::MAX as int,
{}
// One authoritative counter is the sum of exclusive reservation/charge tokens.
fn concurrent_admit(limit:usize,used:usize,bytes:usize)->(next:Option<usize>)
    ensures next.is_some() ==> used<=limit && bytes<=limit-used && next.unwrap()==used+bytes && next.unwrap()<=limit,
            used<=limit && bytes<=limit-used ==> next.is_some(),
{
    // ConcurrentMemoryBudget::reserve: `admit(self.limit, used, bytes)?` is the
    // only arithmetic between the load and the compare-exchange.
    match crate::budget::admission::admit(limit,used,bytes) {
        Ok(next) => Some(next),
        Err(_) => None,
    }
}
proof fn concurrent_commit_preserves_tokens(reserved:int,occupied:int,bytes:int,limit:int)
    requires 0<=bytes<=reserved,0<=occupied, reserved+occupied<=limit,
    ensures 0<=reserved-bytes,0<=occupied+bytes,
        (reserved-bytes)+(occupied+bytes)<=limit,
{}
proof fn concurrent_release_token(reserved:int,occupied:int,bytes:int,limit:int)
    requires 0<=bytes<=occupied,0<=reserved,reserved+occupied<=limit,
    ensures 0<=occupied-bytes,reserved+(occupied-bytes)<=limit,
{}
proof fn concurrent_cancel_token(reserved:int,occupied:int,bytes:int,limit:int)
    requires 0<=bytes<=reserved,0<=occupied,reserved+occupied<=limit,
    ensures 0<=reserved-bytes,(reserved-bytes)+occupied<=limit,
{}
}

verus! {
// Encodings: Relaxed=0, Acquire=1, Release=2, AcqRel=3, SeqCst=4.
// kind: load=0, store=1, compare-exchange failure=2, fence=3.
fn shared_core_atomic_order(kind:u8,order:u8)->(panics:bool)
    requires kind<=3,order<=4,
        kind==0 ==> order!=2 && order!=3,
        kind==1 ==> order!=1 && order!=3,
        kind==2 ==> order!=2 && order!=3,
        kind==3 ==> order!=0,
    ensures !panics,
{
    if kind==0 {order==2 || order==3}
    else if kind==1 {order==1 || order==3}
    else if kind==2 {order==2 || order==3}
    else {order==0}
}
}

verus! {
// The source-refinement caller below invokes the directly proved OR/mask kernels.
proof fn shared_retired_word(bytes:int,tag:int,state:int)
    requires 0<=bytes<tag,tag>0,state==bytes+tag,
    ensures state>=tag,state-tag==bytes,bytes<tag,
{}
}

verus! {
proof fn shared_pointer_frame(frame:usize,word:usize)
    requires word<=frame,
    ensures 0<=word as int<=frame,
{}
}


verus! {
fn shared_source_retirement(bytes:usize)->(r:(usize,usize))
    requires bytes<=isize::MAX as usize,
    ensures r.0 as int==bytes as int+isize::MAX as int+1,r.1==bytes,
{
    let state=crate::shared::count::retired_state(bytes);
    let decoded=crate::shared::count::retired_bytes(state);
    proof {
        shared_retired_word(bytes as int,isize::MAX as int+1,state as int);
        vstd::arithmetic::div_mod::lemma_mod_multiples_vanish(1,bytes as int,isize::MAX as int+1);
        vstd::arithmetic::div_mod::lemma_small_mod(bytes as nat,(isize::MAX as int+1) as nat);
    }
    (state,decoded)
}
}

// Maintained callers for the counter words and the payload pointers. The
// counter callers run the product kernels of src/shared/count.rs on an
// arbitrary loaded word; which words a live owner can load is the protocol
// model above and the Kani and Loom schedules. The payload callers pass each
// pointer operation to the storage model (storage.verus.rs, placed in front of
// this file by storage_kernel.py). Stated premises: the allocation made by
// `Allocation::new` is a `raw_valid` byte storage of `layout.size()` bytes and
// `Layout::extend` returned an offset with `offset + payload <= size`; the
// payload is a `raw_valid` storage of its element count that starts at that
// offset; a source value or slice is a storage `apart` from the allocation.
verus! {
// LocalCount::increment (clone_strong, clone_weak, upgrade); one iteration of
// AtomicCount::increment after the LOCK test; AtomicCount::upgrade after
// `observed_live_word`. The stored word is exactly one more and admitted.
fn shared_increment_caller(old:usize,live:bool)->(out:Option<usize>)
    ensures match out {
        Some(next) => old<crate::shared::count::MAX_COUNT && next as int==old as int+1
            && next<=crate::shared::count::MAX_COUNT,
        None => true,
    },
{
    if live && (old==0 || old>crate::shared::count::MAX_COUNT) {
        return None;
    }
    match crate::shared::count::admitted_count(old) {
        Ok(next) => Some(next),
        Err(_) => None,
    }
}
// LocalCount::release (and release_weak): the caller owns one credit.
fn shared_release_caller(old:usize)->(out:(usize,bool))
    requires old>=1,
    ensures out.0 as int==old as int-1,out.1==(old==1),
{
    (crate::arithmetic::bounded::wrapping_sub(old,1),old==1)
}
// AtomicCount::buffered_clone_strong: the guard test on the word after the
// increment. `old + 1` does not wrap by `buffered_reservation_extent`.
fn shared_buffered_clone_caller(old:usize)->(refused:bool)
    requires old as int+1<2*(crate::shared::count::MAX_COUNT as int+1),
    ensures refused==(old as int+1>crate::shared::count::MAX_COUNT as int),
{
    let guard=crate::arithmetic::bounded::wrapping_add(crate::shared::count::MAX_COUNT,1);
    let next=crate::arithmetic::bounded::wrapping_add(old,1);
    let refused=next & guard != 0;
    proof {
        if crate::shared::count::MAX_COUNT==7 {
            assert((next & 8usize != 0)==(next>7)) by(bit_vector) requires next<16;
        } else {
            let half=isize::MAX as usize;
            let maximum=usize::MAX;
            assert(maximum<=!0usize) by(bit_vector);
            vstd::bits::lemma_usize_shr_is_div(usize::MAX,1);
            reveal(vstd::arithmetic::power::pow);
            assert(vstd::arithmetic::power2::pow2(1)==2) by(compute_only);
            assert(half==usize::MAX>>1);
            assert((next & guard != 0)==(next>half)) by(bit_vector)
                requires half==(!0usize)>>1,guard==half+1;
        }
    }
    refused
}
// AtomicCount::clone_weak, one iteration after the LOCK test: the retirement
// flag is kept and the credit part grows by one.
fn shared_clone_weak_caller(old:usize)->(out:Option<usize>)
    ensures match out {
        Some(next) => next as int==old as int+1
            && next & crate::shared::count::RETIRED==old & crate::shared::count::RETIRED
            && (next & !crate::shared::count::RETIRED) as int==(old & !crate::shared::count::RETIRED) as int+1,
        None => true,
    },
{
    let limit=crate::arithmetic::bounded::wrapping_sub(crate::shared::count::MAX_COUNT,1);
    if old & !crate::shared::count::RETIRED>=limit {
        return None;
    }
    let half=isize::MAX as usize;
    let flag=crate::shared::count::RETIRED;
    proof {
        let maximum=usize::MAX;
        assert(maximum<=!0usize) by(bit_vector);
        vstd::bits::lemma_usize_shr_is_div(usize::MAX,1);
        reveal(vstd::arithmetic::power::pow);
        assert(vstd::arithmetic::power2::pow2(1)==2) by(compute_only);
        assert(half==usize::MAX>>1);
        assert(flag==!half);
        assert(old<!0usize) by(bit_vector)
            requires half==(!0usize)>>1,flag==!half,old & !flag<half;
    }
    match old.checked_add(1) {
        Some(next) => {
            proof {
                assert(next & flag==old & flag && next & !flag==(old & !flag)+1) by(bit_vector)
                    requires half==(!0usize)>>1,flag==!half,old & !flag<half,next==old+1;
            }
            Some(next)
        }
        None => None,
    }
}
// Allocation::data: `ptr.add(offset)` inside the byte allocation.
fn shared_data_caller(a:RawStorage,offset:usize,payload:usize)->(address:usize)
    requires raw_valid(a),a.size==1,offset as int+payload as int<=a.cap,
    ensures doc_ptr_add(allocation_of(a),a.base,offset,a.size),
{
    offset_storage(a,offset)
}
// One object of a storage: the header written by `Allocation::new` and dropped
// by its guard, and one sized payload: `data.cast::<T>().write(value)`, the slot formed from
// `NonNull::new_unchecked(data)`, `data(ptr).read()`, `drop_in_place(data(ptr))`
// and the references `&*data(ptr)`, `&mut *data(ptr)`.
fn shared_value_caller(p:RawStorage)->(address:usize)
    requires raw_valid(p),p.cap==1,
    ensures doc_non_null_new_unchecked(address),
{
    let _start=representation_caller(p);
    slice_caller(p,0,1)
}
// A pointer to the start of a live storage: `NonNull::new_unchecked` in
// `control`, `uninit_slice`, the slice and str casts and the unsizing macros;
// the header fields reached through a control pointer (`counts`, `data`,
// the policy); the payload pointer passed on to `clone_into`.
fn shared_header_caller(h:RawStorage)->(address:usize)
    requires raw_valid(h),
    ensures doc_non_null_new_unchecked(address),
{
    representation_caller(h)
}
// `copy_nonoverlapping(source bytes, a.data(), bytes)`: try_from_box_with, the
// bitwise branch of try_make_mut, and the str, CStr and native-shape clones.
fn shared_bytes_copy_caller(a:RawStorage,source:RawStorage,offset:usize,bytes:usize)
    requires raw_valid(a),a.size==1,offset as int+bytes as int<=a.cap,
        raw_valid(source),source.cap==bytes,same_element(source,a),apart(source,a),
{
    let _destination=shared_data_caller(a,offset,bytes);
    distinct_nonoverlap_caller(source,a,0,offset,bytes);
}
// `copy_nonoverlapping(source.as_ptr(), a.data().cast::<T>(), len)`:
// try_from_std_vec_in, try_copy_from_slice_in and try_from_vec_in.
fn shared_slice_copy_caller(p:RawStorage,source:RawStorage,len:usize)
    requires raw_valid(p),p.cap==len,raw_valid(source),source.cap==len,
        same_element(source,p),apart(source,p),
{
    let _destination=representation_caller(p);
    distinct_nonoverlap_caller(source,p,0,0,len);
}
}
