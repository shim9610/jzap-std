// Maintained callers for the binary heap. Each function calls the product
// kernels in src/collections/binary_heap/geometry.rs and the bounded add/sub
// with the arguments of the product body named in scripts/heap_callers.py.
// `end` is the slice length; `zst` is `size_of::<T>() == 0`. Stated premise:
// a slice of sized elements has at most isize::MAX elements. Comparison
// results are arbitrary values read from `replies`.
// Each element pointer of those bodies is passed to the storage model
// (storage.verus.rs, placed in front of this file by storage_kernel.py), whose
// postcondition is the documented condition of the core call. Stated premise:
// `s` is a `raw_valid` storage that holds at least the `end` elements of the
// slice. The Hole value on the stack is a separate object outside this model.
use vstd::prelude::*;
verus! {
// Hole::new and Hole::get: `data.as_ptr().add(index)` and the element read.
fn heap_element_caller(s:RawStorage,index:usize,end:usize)->(address:usize)
 requires raw_valid(s),index<end,end<=s.cap,
 ensures doc_ptr_add(allocation_of(s),s.base,index,s.size),
{
 let _start=offset_storage(s,index);
 slice_caller(s,index,1)
}
// Hole::move_to: `copy_nonoverlapping(data.add(index), data.add(self.pos), 1)`.
fn heap_move_caller(s:RawStorage,index:usize,pos:usize,end:usize)
 requires raw_valid(s),index<end,pos<end,index!=pos,end<=s.cap,
{
 let _source=offset_storage(s,index);
 let _destination=offset_storage(s,pos);
 unordered_nonoverlap_caller(s,index,pos,1);
}
// core::first_child restated; the product body is compared as text.
fn heap_first_child(pos:usize,end:usize,zst:bool)->(child:usize)
 requires pos<end,!zst ==> end<=isize::MAX,
 ensures child>pos,zst ==> child<=end,
{
 if zst {
  if pos>=end/2 {
   end
  } else {
   crate::collections::binary_heap::geometry::children(pos,end).0
  }
 } else {
  crate::collections::binary_heap::geometry::child_offset(pos)
 }
}
// Hole::drop: `copy_nonoverlapping(&*self.value, data.add(self.pos), 1)`.
// Stated: `value` is the one-element storage of the removed value, outside the
// slice storage, and `pos` is a position returned by the sift callers below.
fn heap_restore_caller(s:RawStorage,value:RawStorage,pos:usize,end:usize)
 requires raw_valid(s),pos<end,end<=s.cap,
  raw_valid(value),value.cap==1,value.size==s.size,value.align==s.align,
  value.base as int+value.cap as int*value.size as int<=s.base as int
   || s.base as int+s.cap as int*s.size as int<=value.base as int,
{
 let _destination=offset_storage(s,pos);
 distinct_nonoverlap_caller(value,s,0,pos,1);
}
// PeekMut::deref and deref_mut: the root of a nonempty heap.
fn heap_peek_caller(s:RawStorage,len:usize)->(address:usize)
 requires raw_valid(s),0<len,len<=s.cap,
{
 heap_element_caller(s,0,len)
}
// core::sift_up: every index read or written lies below `end`.
fn heap_sift_up_caller(s:RawStorage,start:usize,pos:usize,end:usize,replies:&[bool])->(hole:usize)
 requires raw_valid(s),end<=s.cap,pos<end,replies@.len()>=end,
 ensures hole<=pos,
{
 let mut hole=pos;
 let _taken=heap_element_caller(s,pos,end);
 while hole>start
  invariant hole<=pos,pos<end,replies@.len()>=end,raw_valid(s),end<=s.cap,
  decreases hole,
 {
  let p=crate::collections::binary_heap::geometry::parent(hole);
  let _read=heap_element_caller(s,p,end);
  if replies[p] {
   break;
  }
  heap_move_caller(s,p,hole,end);
  hole=p;
 }
 hole
}
// core::sift_down. `early` selects the product's early return (sift_down) or
// the full descent (sift_bottom); the final hole is returned.
fn heap_descend_caller(s:RawStorage,pos:usize,end:usize,zst:bool,early:bool,choices:&[bool],stops:&[bool])->(hole:usize)
 requires raw_valid(s),end<=s.cap,pos<end,!zst ==> end<=isize::MAX,choices@.len()>=end,stops@.len()>=end,
 ensures pos<=hole,hole<end,
{
 let mut hole=pos;
 let _taken=heap_element_caller(s,pos,end);
 let last=crate::arithmetic::bounded::wrapping_sub(end,1);
 let mut child=heap_first_child(pos,end,zst);
 while child<last
  invariant hole<child,hole<end,pos<=hole,last+1==end,!zst ==> end<=isize::MAX,
    choices@.len()>=end,stops@.len()>=end,raw_valid(s),end<=s.cap,
  decreases end-hole,
 {
  let right=crate::arithmetic::bounded::wrapping_add(child,1);
  let _left_read=heap_element_caller(s,child,end);
  let _right_read=heap_element_caller(s,right,end);
  child=crate::collections::binary_heap::geometry::selected(child,end,choices[child]);
  if early {
   let _read=heap_element_caller(s,child,end);
   if stops[child] {
    return hole;
   }
  }
  heap_move_caller(s,child,hole,end);
  hole=child;
  child=heap_first_child(hole,end,zst);
 }
 if child==last {
  if early {
   let _read=heap_element_caller(s,child,end);
  }
  if !early || stops[child] {
   heap_move_caller(s,child,hole,end);
   hole=child;
  }
 }
 hole
}
// core::sift_bottom: full descent, then sift_up back towards the start.
fn heap_sift_bottom_caller(s:RawStorage,pos:usize,end:usize,zst:bool,choices:&[bool],stops:&[bool])
 requires raw_valid(s),end<=s.cap,pos<end,!zst ==> end<=isize::MAX,choices@.len()>=end,stops@.len()>=end,
{
 let hole=heap_descend_caller(s,pos,end,zst,false,choices,stops);
 let _final=heap_sift_up_caller(s,pos,hole,end,stops);
}
// core::rebuild
fn heap_rebuild_caller(s:RawStorage,end:usize,zst:bool,choices:&[bool],stops:&[bool])
 requires raw_valid(s),end<=s.cap,!zst ==> end<=isize::MAX,choices@.len()>=end,stops@.len()>=end,
{
 let mut pos=end/2;
 while pos>0
  invariant pos<=end/2,!zst ==> end<=isize::MAX,choices@.len()>=end,stops@.len()>=end,raw_valid(s),end<=s.cap,
  decreases pos,
 {
  pos=crate::arithmetic::bounded::wrapping_sub(pos,1);
  let _hole=heap_descend_caller(s,pos,end,zst,true,choices,stops);
 }
}
// core::rebuild_tail, incremental branch.
fn heap_rebuild_tail_caller(s:RawStorage,start:usize,len:usize,replies:&[bool])
 requires raw_valid(s),len<=s.cap,replies@.len()>=len,
{
 if start>=len {
  return;
 }
 let _tail=crate::arithmetic::bounded::wrapping_sub(len,start);
 let mut pos=start;
 while pos<len
  invariant pos<=len,replies@.len()>=len,raw_valid(s),len<=s.cap,
  decreases len-pos,
 {
  let _hole=heap_sift_up_caller(s,0,pos,len,replies);
  pos=crate::arithmetic::bounded::wrapping_add(pos,1);
 }
}
// core::pop_nonempty
fn heap_pop_caller(s:RawStorage,len:usize,zst:bool,choices:&[bool],stops:&[bool])->(last:usize)
 requires raw_valid(s),len<=s.cap,len>0,!zst ==> len<=isize::MAX,choices@.len()>=len,stops@.len()>=len,
 ensures last+1==len,
{
 let last=crate::arithmetic::bounded::wrapping_sub(len,1);
 // `data.as_ptr().add(last).read()`.
 let _removed=heap_element_caller(s,last,len);
 if last==0 {
  return last;
 }
 // `data.as_mut_ptr().replace(value)` on the first element.
 let _root=replace_caller(s,0);
 heap_sift_bottom_caller(s,0,last,zst,choices,stops);
 last
}
// PeekMut::drop: the hidden length is restored, then the root is sifted down.
fn heap_peek_drop_caller(s:RawStorage,len:usize,zst:bool,choices:&[bool],stops:&[bool])
 requires raw_valid(s),0<len,len<=s.cap,!zst ==> len<=isize::MAX,choices@.len()>=len,stops@.len()>=len,
{
 let _hole=heap_descend_caller(s,0,len,zst,true,choices,stops);
}
// BinaryHeap::try_push after a successful Vec push.
fn heap_push_caller(s:RawStorage,len:usize,replies:&[bool])
 requires raw_valid(s),len<=s.cap,len>0,replies@.len()>=len,
{
 let pos=crate::arithmetic::bounded::wrapping_sub(len,1);
 let _hole=heap_sift_up_caller(s,0,pos,len,replies);
}
// BinaryHeap::into_sorted_vec
fn heap_sorted_caller(s:RawStorage,len:usize,zst:bool,choices:&[bool],stops:&[bool])
 requires raw_valid(s),len<=s.cap,!zst ==> len<=isize::MAX,choices@.len()>=len,stops@.len()>=len,
{
 let mut end=len;
 while end>1
  invariant end<=len,!zst ==> len<=isize::MAX,choices@.len()>=len,stops@.len()>=len,raw_valid(s),len<=s.cap,
  decreases end,
 {
  end=crate::arithmetic::bounded::wrapping_sub(end,1);
  // `ptr::swap(ptr, ptr.add(end))`: the first element and the element at `end`.
  let _first=heap_element_caller(s,0,len);
  let _last=heap_element_caller(s,end,len);
  let _hole=heap_descend_caller(s,0,end,zst,true,choices,stops);
 }
}
}
fn main() {}
