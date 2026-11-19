// Maintained callers for fixed scratch storage. Each function calls the product
// endpoint kernels in src/slice/geometry.rs with the arguments of the product
// body named in scripts/scratch_callers.py and proves the slot range used
// there. Each pointer operation of that body is then passed to the storage
// model (storage.verus.rs, placed in front of this file by storage_kernel.py),
// whose postcondition is the documented condition of the core call.
// Stated premises of every caller: the owner length is at most the fixed
// capacity, and the fixed storage is a `raw_valid` storage of that capacity.
// Typed initialization, aliasing and destructors are outside Verus.
use vstd::prelude::*;
verus! {
// as_slice, as_mut_slice: `from_raw_parts(_mut)(storage, len)`.
fn scratch_prefix_caller(s:RawStorage,len:usize)->(address:usize)
 requires raw_valid(s),len<=s.cap,
 ensures doc_slice_from_raw_parts(allocation_of(s),address,len,s.size,s.align),
{
 slice_caller(s,0,len)
}
// Scratch::try_push
fn scratch_push_caller(s:RawStorage,len:usize,cap:usize)->(out:Option<(usize,usize)>)
 requires raw_valid(s),s.cap==cap,len<=cap,
 ensures match out {Some((index,end)) => index<cap && end<=cap && end==index+1,None => len==cap},
{
 match crate::slice::geometry::admitted_end(len,1,cap) {
  Some(end) => {
   // `storage.get_mut(index)` and the write through the returned cell.
   let _slot=initialized_caller(s,len,cap);
   Some((len,end))
  }
  None => None,
 }
}
// Scratch::pop
fn scratch_pop_caller(s:RawStorage,len:usize,cap:usize)->(index:usize)
 requires raw_valid(s),s.cap==cap,0<len,len<=cap,
 ensures index<cap,index+1==len,
{
 let index=crate::slice::geometry::retired_last(len);
 // `storage.get_unchecked(index)` on the slice of `cap` cells.
 let _slot=initialized_caller(s,index,cap);
 index
}
// Scratch::truncate: the dropped suffix lies inside the old prefix.
fn scratch_truncate_caller(s:RawStorage,len:usize,old:usize,cap:usize)->(count:usize)
 requires raw_valid(s),s.cap==cap,old<=cap,
 ensures len<old ==> len+count==old && len+count<=cap,len>=old ==> count==0,
{
 if len>=old {
  return 0;
 }
 let count=crate::arithmetic::bounded::wrapping_sub(old,len);
 // `storage.add(len)` and `slice_from_raw_parts_mut(.., count)`.
 let _start=offset_storage(s,len);
 let _suffix=slice_caller(s,len,count);
 count
}
// Scratch::try_extend_copy with Scratch::copy_admitted, and the owned
// ScratchBuf::copy_admitted after the same admission.
// Stated: `source` is the storage of the borrowed source slice, of the same
// element type and outside the scratch storage.
fn scratch_copy_caller(s:RawStorage,source:RawStorage,len:usize,count:usize,cap:usize)->(out:Option<usize>)
 requires raw_valid(s),s.cap==cap,len<=cap,
  raw_valid(source),source.cap==count,source.size==s.size,source.align==s.align,
  source.base as int+source.cap as int*source.size as int<=s.base as int
   || s.base as int+s.cap as int*s.size as int<=source.base as int,
 ensures match out {Some(end) => len+count==end && end<=cap,None => len+count>cap},
{
 match crate::slice::geometry::admitted_end(len,count,cap) {
  Some(end) => {
   // `copy_nonoverlapping(source.as_ptr(), storage.add(len), source.len())`.
   let _destination=offset_storage(s,len);
   distinct_nonoverlap_caller(source,s,0,len,count);
   Some(end)
  }
  None => None,
 }
}
// try_extend_cloned: the callback index of the resize loop names a source element.
fn scratch_cloned_caller(start:usize,count:usize,cap:usize,index:usize)->(out:Option<usize>)
 requires start<=cap,
 ensures match out {Some(offset) => offset<count,None => true},
{
 match crate::slice::geometry::admitted_end(start,count,cap) {
  Some(end) => {
   if start<=index && index<end {
    Some(crate::arithmetic::bounded::wrapping_sub(index,start))
   } else {None}
  }
  None => None,
 }
}
// Scratch::try_resize_with_init: every initialized index is a storage slot and
// the loop ends at the admitted length.
fn scratch_resize_init_caller(s:RawStorage,len:usize,new_len:usize,cap:usize)->(out:Option<usize>)
 requires raw_valid(s),s.cap==cap,len<=cap,
 ensures match out {Some(end) => end<=cap && (new_len>len ==> end==new_len),None => new_len>cap},
{
 if new_len<=len {
  let _dropped=scratch_truncate_caller(s,new_len,len,cap);
  return Some(new_len);
 }
 if crate::slice::geometry::admitted_end(0,new_len,cap).is_none() {
  return None;
 }
 let mut len=len;
 while len<new_len
  invariant len<=new_len,new_len<=cap,raw_valid(s),s.cap==cap,
  decreases new_len-len,
 {
  let index=len;
  let next=crate::slice::geometry::initialized_next(index,new_len);
  // `storage.get_mut(index)` and the slot formed from that cell.
  let _slot=initialized_caller(s,index,cap);
  len=next;
 }
 Some(len)
}
// try_resize for zero-sized bitwise-clonable values, borrowed and owned, and
// ScratchBuf::try_clone_from after clearing: the published length is admitted.
fn scratch_admitted_length_caller(new_len:usize,cap:usize)->(out:Option<usize>)
 ensures match out {Some(end) => end==new_len && end<=cap,None => new_len>cap},
{
 crate::slice::geometry::admitted_end(0,new_len,cap)
}
}
fn main() {}
