// Maintained callers for the binary heap. Each function calls the product
// kernels in src/collections/binary_heap/geometry.rs and the bounded add/sub
// with the arguments of the product body named in scripts/heap_callers.py.
// `end` is the slice length; `zst` is `size_of::<T>() == 0`. Comparison
// results are read from the arbitrary sequences `replies`, `choices` and
// `stops` through `pick`, which has no precondition.
// Each element pointer of those bodies is passed to the storage model
// (storage.verus.rs, placed in front of this file by storage_kernel.py), whose
// postcondition is the documented condition of the core call. The callers
// that the product enters with a Vec (issue #16, unit 7, rule A) require the
// Vec invariant and the storage pairing of rule B (`paired_storage`) and derive
// `raw_valid` from them (`paired_raw`); the kernels below them require
// `raw_valid` of the slice storage, established by those callers. The Hole
// value on the stack is a separate object outside this model.
use vstd::prelude::*;
verus! {
// Rule B (issue #16; approved by the owner on 2026-11-28). The storage of a
// Vec of capacity `cap`: the pointer is the one returned by the allocation of
// `layout::array::<T>(cap)`, or the dangling pointer (address `align`) when
// `cap == 0` or the element is zero-sized. Alignment and size are the
// compiler's facts (trust boundary item 1).
// `from_allocation` is the trust-boundary item: what a successful allocation
// of a valid layout is documented to return (non-null, aligned, no address
// wrap). It is a definition, not an assumption: the claim that a product
// pointer from the allocator satisfies it is the stated trust boundary, and
// the claim that the owner's pointer is that pointer (or dangling) is the
// pairing proved by the owner closure.
spec fn from_allocation(s:RawStorage)->bool {
    s.base>0 && s.base as int%s.align as int==0
        && s.base as int+s.cap as int*s.size as int<=usize::MAX as int
}
spec fn paired_storage(s:RawStorage,cap:usize,size:usize)->bool {
    s.cap==cap && s.size==size && is_pow2(s.align as int) && s.align>0
        && s.size as int%s.align as int==0
        && if cap==0 || size==0 { s.base==s.align } else { from_allocation(s) }
}
proof fn paired_raw(s:RawStorage,cap:usize,size:usize)
    requires paired_storage(s,cap,size),byte_valid(cap,size),
    ensures raw_valid(s),s.cap==cap,size==0 || cap<=isize::MAX,
{
    if cap==0 || size==0 {
        lemma_fundamental_div_mod(s.align as int,s.align as int);
        assert(cap as int*size as int==0) by(nonlinear_arith) requires cap==0 || size==0;
    }
    assert(size==0 || cap as int<=cap as int*size as int) by(nonlinear_arith) requires size>=0,cap>=0;
}
// A comparison oracle: the value at `index` of an arbitrary sequence (false
// past its end). Every sequence of comparison results is one choice of `o`.
fn pick(o:&[bool],index:usize)->bool {
    if index<o.len() { o[index] } else { false }
}
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
 requires raw_valid(s),end<=s.cap,pos<end,
 ensures hole<=pos,
{
 let mut hole=pos;
 let _taken=heap_element_caller(s,pos,end);
 while hole>start
  invariant hole<=pos,pos<end,raw_valid(s),end<=s.cap,
  decreases hole,
 {
  let p=crate::collections::binary_heap::geometry::parent(hole);
  let _read=heap_element_caller(s,p,end);
  if pick(replies,p) {
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
 requires raw_valid(s),end<=s.cap,pos<end,!zst ==> end<=isize::MAX,
 ensures pos<=hole,hole<end,
{
 let mut hole=pos;
 let _taken=heap_element_caller(s,pos,end);
 let last=crate::arithmetic::bounded::wrapping_sub(end,1);
 let mut child=heap_first_child(pos,end,zst);
 while child<last
  invariant hole<child,hole<end,pos<=hole,last+1==end,!zst ==> end<=isize::MAX,
    raw_valid(s),end<=s.cap,
  decreases end-hole,
 {
  let right=crate::arithmetic::bounded::wrapping_add(child,1);
  let _left_read=heap_element_caller(s,child,end);
  let _right_read=heap_element_caller(s,right,end);
  child=crate::collections::binary_heap::geometry::selected(child,end,pick(choices,child));
  if early {
   let _read=heap_element_caller(s,child,end);
   if pick(stops,child) {
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
  if !early || pick(stops,child) {
   heap_move_caller(s,child,hole,end);
   hole=child;
  }
 }
 hole
}
// core::sift_bottom: full descent, then sift_up back towards the start.
fn heap_sift_bottom_caller(s:RawStorage,pos:usize,end:usize,zst:bool,choices:&[bool],stops:&[bool])
 requires raw_valid(s),end<=s.cap,pos<end,!zst ==> end<=isize::MAX,
{
 let hole=heap_descend_caller(s,pos,end,zst,false,choices,stops);
 let _final=heap_sift_up_caller(s,pos,hole,end,stops);
}
// core::rebuild
fn heap_rebuild_caller(s:RawStorage,end:usize,zst:bool,choices:&[bool],stops:&[bool])
 requires raw_valid(s),end<=s.cap,!zst ==> end<=isize::MAX,
{
 let mut pos=end/2;
 while pos>0
  invariant pos<=end/2,!zst ==> end<=isize::MAX,raw_valid(s),end<=s.cap,
  decreases pos,
 {
  pos=crate::arithmetic::bounded::wrapping_sub(pos,1);
  let _hole=heap_descend_caller(s,pos,end,zst,true,choices,stops);
 }
}
// core::rebuild_tail: both branches. The cost estimate that chooses between
// them never controls a memory bound; it is an arbitrary choice here.
fn heap_rebuild_tail_caller(s:RawStorage,start:usize,len:usize,zst:bool,choices:&[bool],stops:&[bool])
 requires raw_valid(s),len<=s.cap,!zst ==> len<=isize::MAX,
{
 if start>=len {
  return;
 }
 let _tail=crate::arithmetic::bounded::wrapping_sub(len,start);
 if pick(choices,len) {
  heap_rebuild_caller(s,len,zst,choices,stops);
 } else {
  let mut pos=start;
  while pos<len
   invariant pos<=len,raw_valid(s),len<=s.cap,
   decreases len-pos,
  {
   let _hole=heap_sift_up_caller(s,0,pos,len,stops);
   pos=crate::arithmetic::bounded::wrapping_add(pos,1);
  }
 }
}
// The callers the product enters with a Vec (rule A). Each requires the Vec
// invariant over the words (len, cap, limit) of the Vec at the modeled point,
// the storage pairing of rule B, and the listed local facts only.
//
// core::pop_nonempty, entered with `data: &mut Vec` (local fact: len > 0).
fn heap_pop_caller(s:RawStorage,len:usize,cap:usize,limit:usize,size:usize,choices:&[bool],stops:&[bool])->(last:usize)
 requires len<=cap<=limit,byte_valid(cap,size),paired_storage(s,cap,size),len>0,
 ensures last+1==len,
{
 proof { paired_raw(s,cap,size); }
 let zst=size==0;
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
// PeekMut::drop after the restoring `set_len`: the Vec of length `vlen` is
// sifted from the root over `len` elements (local facts: 0 < len <= vlen).
fn heap_peek_drop_caller(s:RawStorage,len:usize,vlen:usize,cap:usize,limit:usize,size:usize,choices:&[bool],stops:&[bool])
 requires vlen<=cap<=limit,byte_valid(cap,size),paired_storage(s,cap,size),0<len,len<=vlen,
{
 proof { paired_raw(s,cap,size); }
 let _hole=heap_descend_caller(s,0,len,size==0,true,choices,stops);
}
// BinaryHeap::try_push after the successful Vec push (local fact: len > 0).
fn heap_push_caller(s:RawStorage,len:usize,cap:usize,limit:usize,size:usize,replies:&[bool])
 requires len<=cap<=limit,byte_valid(cap,size),paired_storage(s,cap,size),len>0,
{
 proof { paired_raw(s,cap,size); }
 let pos=crate::arithmetic::bounded::wrapping_sub(len,1);
 let _hole=heap_sift_up_caller(s,0,pos,len,replies);
}
// BinaryHeap::into_sorted_vec, entered with `self` (no local fact).
fn heap_sorted_caller(s:RawStorage,len:usize,cap:usize,limit:usize,size:usize,choices:&[bool],stops:&[bool])
 requires len<=cap<=limit,byte_valid(cap,size),paired_storage(s,cap,size),
{
 proof { paired_raw(s,cap,size); }
 let zst=size==0;
 let mut end=len;
 while end>1
  invariant end<=len,len<=s.cap,raw_valid(s),!zst ==> len<=isize::MAX,
  decreases end,
 {
  end=crate::arithmetic::bounded::wrapping_sub(end,1);
  // `ptr::swap(ptr, ptr.add(end))`: the first element and the element at `end`.
  let _first=heap_element_caller(s,0,len);
  let _last=heap_element_caller(s,end,len);
  let _hole=heap_descend_caller(s,0,end,zst,true,choices,stops);
 }
}
// BinaryHeap::from_vec: `core::rebuild(data.as_mut_slice())` on the moved Vec.
fn heap_from_vec_caller(s:RawStorage,len:usize,cap:usize,limit:usize,size:usize,choices:&[bool],stops:&[bool])
 requires len<=cap<=limit,byte_valid(cap,size),paired_storage(s,cap,size),
{
 proof { paired_raw(s,cap,size); }
 heap_rebuild_caller(s,len,size==0,choices,stops);
}
// BinaryHeap::try_append after the successful Vec append:
// `core::rebuild_tail(self.data.as_mut_slice(), start)`.
fn heap_append_caller(s:RawStorage,start:usize,len:usize,cap:usize,limit:usize,size:usize,choices:&[bool],stops:&[bool])
 requires len<=cap<=limit,byte_valid(cap,size),paired_storage(s,cap,size),
{
 proof { paired_raw(s,cap,size); }
 heap_rebuild_tail_caller(s,start,len,size==0,choices,stops);
}
// The Repair guard of try_retain, after the Vec retain:
// `if self.start < self.heap.len() { core::rebuild_tail(..) }`.
fn heap_retain_repair_caller(s:RawStorage,start:usize,len:usize,cap:usize,limit:usize,size:usize,choices:&[bool],stops:&[bool])
 requires len<=cap<=limit,byte_valid(cap,size),paired_storage(s,cap,size),
{
 proof { paired_raw(s,cap,size); }
 if start<len {
  heap_rebuild_tail_caller(s,start,len,size==0,choices,stops);
 }
}
// The Repair guard of try_extend, after the Vec pushes:
// `core::rebuild_tail(self.heap.data.as_mut_slice(), self.start)`.
fn heap_extend_repair_caller(s:RawStorage,start:usize,len:usize,cap:usize,limit:usize,size:usize,choices:&[bool],stops:&[bool])
 requires len<=cap<=limit,byte_valid(cap,size),paired_storage(s,cap,size),
{
 proof { paired_raw(s,cap,size); }
 heap_rebuild_tail_caller(s,start,len,size==0,choices,stops);
}
}
fn main() {}
