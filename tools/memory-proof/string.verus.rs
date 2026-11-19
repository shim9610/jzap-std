use vstd::prelude::*;
verus! {
fn retain_next(read: usize, written: usize, original: usize, width: usize, retain: bool)
    -> (result: (usize, usize))
    requires written <= read, read < original, 1 <= width <= 4,
             width <= original - read,
    ensures result.0 == read + width,
            result.1 == if retain { written + width } else { written as int },
            result.1 <= result.0 <= original,
            read + width <= original, written + width <= original,
{
    let next_read = crate::string::numeric::advance(read,width);
    let next_written = if retain { crate::string::numeric::advance(written,width) } else { written };
    (next_read,next_written)
}
fn retain_restore(read: usize, written: usize, original: usize) -> (len: usize)
    requires written <= read <= original,
    ensures len == written + (original - read), len <= original,
            read + (original - read) == original,
            written + (original - read) <= original,
{
    crate::string::numeric::restore(read,written,original).1
}
fn pop_length(len: usize, width: usize) -> (new_len: usize)
    requires 1 <= width <= 4, width <= len,
    ensures new_len == len - width, new_len < len,
{ len-width }
}

verus! {
// The byte's UTF-8 boundary predicate is abstracted as a bool. The bounds
// decisions are projected from the pinned core body, for every word-sized index.
fn string_core_boundary(index:usize,len:usize,byte_boundary:bool)->(ok:bool)
    ensures ok ==> index<=len,
{
    if index==0 {true} else if index>=len {index==len} else {byte_boundary}
}
fn string_core_range(start:usize,end:usize,len:usize,a:bool,b:bool)->(ok:bool)
    requires a ==> start<=len, b ==> end<=len,
    ensures ok ==> end>=start && end<=len,
{
    start<=end && a && b
}
fn string_core_str_arithmetic(start:usize,end:usize,len:usize)->(width:usize)
    requires start<=end<=len, len<=isize::MAX as usize,
    ensures width==end-start, width<=len, start+width<=len,
{
    end-start
}
fn string_core_byte_cursor(front:usize,back:usize,width:usize)->(r:(usize,usize))
    requires front<=back, back<=isize::MAX as usize, 1<=width<=4, width<=back-front,
    ensures r.0<=back, front<=r.1, width<=isize::MAX,
{
    (front+width,back-width)
}
fn string_core_raw_vec(len:usize,cap:usize)->(bytes:usize)
    requires len<=cap, cap<=isize::MAX as usize,
    ensures bytes==cap, len<=cap,
{
    cap*1
}
fn string_core_negated_byte_step(step:isize)->(value:isize)
    requires 0<=step<=4,
    ensures value == -step,
{
    -step
}
}

verus! {
fn string_drain_restore(start:usize,end:usize,old:usize)->(length:usize)
    requires start<=end<=old,
    ensures length==start+(old-end), length<=old,
            start+(old-end)<=old, end+(old-end)==old,
{
    crate::string::numeric::drain(start,end,old).1
}
}

verus! {
// Vec::try_reserve with grow, grow_to and resize_storage for byte storage: the
// capacity afterwards. The same statements as `vec_try_reserve` in
// raw-owner.verus.rs; the reservation bodies are compared whole by the runner.
fn string_try_reserve(len:usize,cap:usize,limit:usize,additional:usize)->(out:Option<usize>)
    ensures match out {
        Some(after) => len as int+additional as int<=after as int && cap<=after,
        None => true,
    },
{
    let Ok(spare)=crate::Checked::new(cap)-crate::Checked::new(len) else {return None;};
    if additional>spare.get() {
        let Ok(needed)=crate::vec::required_len(len,additional,limit) else {return None;};
        let Ok(grown)=crate::vec::geometry::growth_for_layout(cap,needed,limit,1,false) else {return None;};
        Some(grown)
    } else {
        Some(cap)
    }
}

// One byte extent `offset..offset+count` of storage holding `cap` bytes.
spec fn byte_extent(offset:usize,count:usize,cap:usize)->bool {
    offset as int+count as int<=cap as int
}

// String::try_replace_range: `Edit::new`, `try_reserve(plan.additional)`, then
// `copy(ptr.add(plan.end), ptr.add(plan.destination), plan.tail)`,
// `copy_nonoverlapping(text, ptr.add(plan.start), inserted)` and
// `try_set_len(plan.len)`. `after` is the capacity after the reservation.
// Each pointer operation is passed to the storage model (storage.verus.rs,
// placed in front of this file by storage_kernel.py). Stated: `s` is the byte
// storage after the reservation and `text` the storage of the inserted text,
// outside it.
fn string_replace_caller(s:RawStorage,text:RawStorage,len:usize,cap:usize,limit:usize,start:usize,end:usize,inserted:usize,after:usize)
    requires len<=cap,raw_valid(s),s.cap==after,
        raw_valid(text),text.cap==inserted,same_element(text,s),apart(text,s),
{
    let Ok(plan)=crate::string::edit::Edit::new(len,start,end,inserted,limit) else {return;};
    let Some(reserved)=string_try_reserve(len,cap,limit,plan.additional) else {return;};
    if after!=reserved {return;}
    assert(byte_extent(plan.end,plan.tail,after));
    assert(byte_extent(plan.destination,plan.tail,after));
    assert(byte_extent(plan.start,inserted,after));
    assert(plan.start as int+inserted as int<=plan.destination as int);
    let _tail=offset_storage(s,plan.end);
    let _moved=offset_storage(s,plan.destination);
    overlapping_copy_caller(s,plan.end,plan.destination,plan.tail);
    let _hole=offset_storage(s,plan.start);
    distinct_nonoverlap_caller(text,s,0,plan.start,inserted);
    assert(plan.len<=after);
    assert(plan.len as int==plan.destination as int+plan.tail as int);
}

// String::try_push: `Edit::new(old, old, old, width, usize::MAX)`, a
// reservation only when `plan.len > capacity`, then `write_scalar` of `width`
// bytes at `ptr.add(plan.start)` and `try_set_len(plan.len)`.
fn string_push_caller(s:RawStorage,len:usize,cap:usize,limit:usize,width:usize,after:usize)
    requires len<=cap, 1<=width<=4,raw_valid(s),s.cap==after,
{
    let Ok(plan)=crate::string::edit::Edit::new(len,len,len,width,usize::MAX) else {return;};
    if plan.len>cap {
        let Some(reserved)=string_try_reserve(len,cap,limit,plan.additional) else {return;};
        if after!=reserved {return;}
    } else {
        if after!=cap {return;}
    }
    assert(plan.start==len);
    assert(byte_extent(plan.start,width,after));
    let _place=offset_storage(s,plan.start);
    let _written=slice_caller(s,plan.start,width);
    assert(plan.len as int==len as int+width as int && plan.len<=after);
}
}
