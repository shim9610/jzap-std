// Maintained callers for the ring buffer. Each function calls the product
// kernels in src/deque/geometry.rs and the bounded add/sub with the arguments
// of the product body named in scripts/deque_callers.py and proves the
// physical range used there. `zst` stands for `size_of::<T>() == 0`.
// Stated premise of every caller: the owner state satisfies `ring`.
// Each pointer operation of those bodies is passed to the storage model
// (storage.verus.rs, placed in front of this file by storage_kernel.py), whose
// postcondition is the documented condition of the core call. Stated premises:
// `s` is a `raw_valid` storage of the ring capacity; a borrowed source slice is
// a `raw_valid` storage of its own length, of the same element type and
// `apart` from the ring storage; so is the storage of another owner.
use vstd::prelude::*;
verus! {
// `ptr.add(physical)` and the access to that one slot.
fn deque_slot_caller(s:RawStorage,physical:usize,cap:usize)->(address:usize)
 requires raw_valid(s),s.cap==cap,physical<cap,
 ensures doc_ptr_add(allocation_of(s),s.base,physical,s.size),
{
 let _start=offset_storage(s,physical);
 slice_caller(s,physical,1)
}
pub open spec fn ring(head:usize,len:usize,cap:usize,zst:bool)->bool {
 len<=cap && (cap==0 ==> head==0) && (cap>0 ==> head<cap)
 && (!zst ==> cap<=isize::MAX) && (zst ==> head==0)
}
// VecDeque::physical restated; the product body is compared as text.
fn deque_physical(head:usize,index:usize,cap:usize,zst:bool)->(out:usize)
 requires zst || (cap>0 && cap<=isize::MAX && head<cap && index<=cap),
 ensures zst ==> out==0,!zst ==> out<cap,
    !zst ==> (head as int+index as int==out as int || head as int+index as int==out as int+cap as int),
{
 if zst {
  0
 } else {
  crate::deque::geometry::allocated_physical(head,index,cap)
 }
}
// as_slices, as_mut_slices and Drop: the two initialized segments.
fn deque_slices_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool)->(out:(usize,usize))
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),
 ensures head+out.0<=cap,out.1<=cap,out.0+out.1==len,
{
 let (a,b)=crate::deque::geometry::split(head,len,cap);
 // `from_raw_parts(ptr.add(head), a)` and `from_raw_parts(ptr, b)`.
 let _first=offset_storage(s,head);
 let _front=slice_caller(s,head,a);
 let _back=slice_caller(s,0,b);
 (a,b)
}
// get, get_mut and swap: a checked logical index.
fn deque_get_caller(s:RawStorage,head:usize,index:usize,len:usize,cap:usize,zst:bool)->(out:usize)
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),index<len,
 ensures out<cap,zst ==> out==0,
{
 let out=deque_physical(head,index,cap,zst);
 let _slot=deque_slot_caller(s,out,cap);
 out
}
// front, front_mut and try_push_front_mut: the head of a nonempty ring.
fn deque_front_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool)->(out:usize)
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len>0,
 ensures out==head,out<cap,zst ==> out==0,
{
 let _slot=deque_slot_caller(s,head,cap);
 head
}
// try_push_back after the capacity test (and growth) left a free slot;
// try_push_back_mut reads the same slot back through the new length.
fn deque_push_back_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool)->(out:(usize,usize))
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len<cap,
 ensures out.1==len+1,out.1<=cap,zst ==> out.0==0,!zst ==> out.0<cap,
{
 let slot=deque_physical(head,len,cap,zst);
 let next=crate::arithmetic::bounded::wrapping_add(len,1);
 let last=crate::arithmetic::bounded::wrapping_sub(next,1);
 let again=deque_physical(head,last,cap,zst);
 assert(again==slot);
 // `slot(len).write(value)` and the reference read back through the new length.
 let _written=deque_slot_caller(s,slot,cap);
 (slot,next)
}
// try_push_front after the capacity test.
fn deque_push_front_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool)->(out:(usize,usize))
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len<cap,
 ensures out.1==len+1,ring(out.0,out.1,cap,zst),
{
 let mut head=head;
 if !zst {
  head=crate::deque::geometry::retreat(head,1,cap);
 }
 // `ptr.add(head).write(value)` at the published head.
 let _written=deque_slot_caller(s,head,cap);
 (head,crate::arithmetic::bounded::wrapping_add(len,1))
}
// pop_front: the product asserts `spare` to the optimizer unchecked.
fn deque_pop_front_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool)->(out:(usize,usize,usize))
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len>0,
 ensures out.0==head,ring(out.1,out.2,cap,zst),out.2<cap,
{
 let old_head=head;
 let len=crate::deque::geometry::after_pop(len,cap);
 let mut head=head;
 if !zst {
  head=crate::deque::geometry::allocated_physical(head,1,cap);
 }
 let spare=len<cap;
 assert(spare);
 // `ptr.add(old_head).read()`.
 let _read=deque_slot_caller(s,old_head,cap);
 (old_head,head,len)
}
// pop_back and the dropping loop of truncate.
fn deque_pop_back_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool)->(out:(usize,usize))
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len>0,
 ensures out.1+1==len,out.1<cap,zst ==> out.0==0,!zst ==> out.0<cap,
{
 let last=crate::deque::geometry::after_pop(len,cap);
 let spare=last<cap;
 assert(spare);
 let slot=deque_physical(head,last,cap,zst);
 // `slot(last).read()`.
 let _read=deque_slot_caller(s,slot,cap);
 (slot,last)
}
// make_contiguous: every copy and slice of each rotation kind, and the final
// slice at the published head.
fn deque_contiguous_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool)->(out:usize)
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),
 ensures out+len<=cap,ring(out,len,cap,zst),
{
 let (a,b)=crate::deque::geometry::split(head,len,cap);
 let kind=crate::deque::geometry::contiguous_kind(head,len,cap);
 let mut head=head;
 if kind!=0 {
  let free=crate::arithmetic::bounded::wrapping_sub(cap,len);
  if kind==1 {
   // `copy(p, p.add(a), b)`, `copy_nonoverlapping(p.add(head), p, a)`.
   overlapping_copy_caller(s,0,a,b);
   unordered_nonoverlap_caller(s,head,0,a);
   head=0;
  } else if kind==2 {
   // `copy(p.add(head), p.add(b), a)`, `copy_nonoverlapping(p, p.add(b + a), b)`.
   overlapping_copy_caller(s,head,b,a);
   let third=crate::arithmetic::bounded::wrapping_add(b,a);
   unordered_nonoverlap_caller(s,0,third,b);
   head=b;
  } else if kind==3 {
   if free!=0 {
    // `copy(p, p.add(free), b)`.
    overlapping_copy_caller(s,0,free,b);
   }
   // `from_raw_parts_mut(p.add(free), len).rotate_left(b)`.
   let _rotated=slice_caller(s,free,len);
   assert(b<=len);
   head=free;
  } else {
   if free!=0 {
    // `copy(p.add(head), p.add(b), a)`.
    overlapping_copy_caller(s,head,b,a);
   }
   // `from_raw_parts_mut(p, len).rotate_right(a)`.
   let _rotated=slice_caller(s,0,len);
   assert(a<=len);
   head=0;
  }
 }
 // `from_raw_parts_mut(ptr.add(head), len)` at the published head.
 let _start=offset_storage(s,head);
 let _whole=slice_caller(s,head,len);
 head
}
// normalize: the contiguous ring is moved to offset zero.
fn deque_normalize_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool)
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),
{
 let head=deque_contiguous_caller(s,head,len,cap,zst);
 if head!=0 {
  // `copy(ptr.add(head), ptr, len)`.
  overlapping_copy_caller(s,head,0,len);
 }
}
// try_shrink_to: the move decision, the published head and the restoration
// after a refused reallocation (the old capacity is still owned).
pub open spec fn shrink_target(min:usize,len:usize)->usize { if min>len {min} else {len} }
fn deque_shrink_caller(s:RawStorage,head:usize,len:usize,cap:usize,min:usize,refused:bool)->(out:usize)
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,false),
 ensures shrink_target(min,len)>=cap || refused ==> out==head,
  shrink_target(min,len)<cap && !refused ==> ring(out,len,shrink_target(min,len),false),
{
 let new=if min>len {min} else {len};
 if new>=cap {
  return head;
 }
 let (a,b)=crate::deque::geometry::split(head,len,cap);
 let moved=head>crate::arithmetic::bounded::wrapping_sub(new,len);
 if moved {
  deque_normalize_caller(s,head,len,cap,false);
 }
 if refused {
  if moved {
   if b==0 {
    // `copy(ptr, ptr.add(head), len)`.
    overlapping_copy_caller(s,0,head,len);
   } else {
    // `from_raw_parts_mut(ptr, len).rotate_left(a)`, `copy(ptr.add(b), ptr.add(head), a)`.
    let _rotated=slice_caller(s,0,len);
    assert(a<=len);
    overlapping_copy_caller(s,b,head,a);
   }
  }
  // `self.head = Checked::new(head)`, or the head left unchanged.
  head
 } else if moved || len==0 {
  // `if moved || len == 0 { self.head = Checked::new(0) }`.
  0
 } else {
  head
 }
}
// grow: relocation of the wrapped ring inside the larger allocation.
// `n` is the storage returned by the reallocation, of the new capacity.
fn deque_grow_caller(n:RawStorage,head:usize,len:usize,old:usize,new:usize)->(out:usize)
 requires raw_valid(n),n.cap==new,ring(head,len,old,false),old>0,old<new,new<=isize::MAX,
 ensures ring(out,len,new,false),
{
 let moved=crate::deque::geometry::grown_head(head,len,old,new);
 let (a,b)=crate::deque::geometry::split(head,len,old);
 if b>0 {
  if moved==head {
   // `copy_nonoverlapping(ptr, ptr.add(old), b)`.
   unordered_nonoverlap_caller(n,0,old,b);
  } else {
   // `copy(ptr.add(head), ptr.add(moved), a)`.
   overlapping_copy_caller(n,head,moved,a);
  }
 }
 moved
}
// copy_ring: chunked movement between two physical positions of a sized ring.
fn deque_copy_ring_caller(s:RawStorage,src:usize,dst:usize,count:usize,cap:usize,backward:bool)
 requires raw_valid(s),s.cap==cap,cap>0,src<cap,dst<cap,count<=cap,
{
 if count==0 || src==dst {
  return;
 }
 let mut src=src;let mut dst=dst;let mut count=count;
 if backward {
  src=crate::deque::geometry::physical(src,count,cap);
  dst=crate::deque::geometry::physical(dst,count,cap);
  while count!=0
   invariant cap>0,src<=cap,dst<=cap,count<=cap,raw_valid(s),s.cap==cap,
   decreases count,
  {
   let (chunk,next_src,next_dst,left)=crate::deque::geometry::backward_chunk(src,dst,count,cap);
   src=next_src;
   dst=next_dst;
   // `copy(p.add(src), p.add(dst), chunk)`.
   overlapping_copy_caller(s,src,dst,chunk);
   count=left;
  }
 } else {
  while count!=0
   invariant cap>0,src<cap,dst<cap,count<=cap,raw_valid(s),s.cap==cap,
   decreases count,
  {
   let (chunk,next_src,next_dst,left)=crate::deque::geometry::forward_chunk(src,dst,count,cap);
   // `copy(p.add(src), p.add(dst), chunk)`.
   overlapping_copy_caller(s,src,dst,chunk);
   count=left;
   src=next_src;
   dst=next_dst;
  }
 }
}
// try_insert_mut after the reservation left a free slot.
fn deque_insert_caller(s:RawStorage,head:usize,index:usize,len:usize,cap:usize,zst:bool)->(out:(usize,usize))
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),index<=len,len<cap,
 ensures out.1==len+1,ring(out.0,out.1,cap,zst),
{
 let tail=crate::arithmetic::bounded::wrapping_sub(len,index);
 let mut head=head;
 if index<tail {
  if !zst {
   head=crate::deque::geometry::retreat(head,1,cap);
  }
  let from=deque_physical(head,1,cap,zst);
  let to=deque_physical(head,0,cap,zst);
  if !zst {
   deque_copy_ring_caller(s,from,to,index,cap,false);
  }
 } else {
  let from=deque_physical(head,index,cap,zst);
  let to=deque_physical(head,crate::arithmetic::bounded::wrapping_add(index,1),cap,zst);
  if !zst {
   deque_copy_ring_caller(s,from,to,tail,cap,true);
  }
 }
 let slot=deque_physical(head,index,cap,zst);
 // `slot(index).write(value)`.
 let _written=deque_slot_caller(s,slot,cap);
 (head,crate::arithmetic::bounded::wrapping_add(len,1))
}
// remove of a checked index.
fn deque_remove_caller(s:RawStorage,head:usize,index:usize,len:usize,cap:usize,zst:bool)->(out:(usize,usize))
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),index<len,
 ensures out.1+1==len,ring(out.0,out.1,cap,zst),
{
 let tail=crate::arithmetic::bounded::wrapping_sub(crate::arithmetic::bounded::wrapping_sub(len,index),1);
 // `slot(index).read()`.
 let _slot=deque_get_caller(s,head,index,len,cap,zst);
 let mut head=head;
 if index<tail {
  let from=deque_physical(head,0,cap,zst);
  let to=deque_physical(head,1,cap,zst);
  if !zst {
   deque_copy_ring_caller(s,from,to,index,cap,true);
   head=crate::deque::geometry::allocated_physical(head,1,cap);
  }
 } else {
  let from=deque_physical(head,crate::arithmetic::bounded::wrapping_add(index,1),cap,zst);
  let to=deque_physical(head,index,cap,zst);
  if !zst {
   deque_copy_ring_caller(s,from,to,tail,cap,false);
  }
 }
 (head,crate::arithmetic::bounded::wrapping_sub(len,1))
}
// rotate_left and rotate_right for a sized ring after their early returns.
fn deque_rotate_caller(s:RawStorage,head:usize,n:usize,len:usize,cap:usize,right:bool)->(out:usize)
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,false),0<n,n<len,
 ensures ring(out,len,cap,false),
{
 let other=crate::arithmetic::bounded::wrapping_sub(len,n);
 if !right {
  if len!=cap {
   deque_copy_ring_caller(s,deque_physical(head,0,cap,false),deque_physical(head,len,cap,false),n,cap,false);
  }
  crate::deque::geometry::allocated_physical(head,n,cap)
 } else {
  let moved=crate::deque::geometry::retreat(head,n,cap);
  if len!=cap {
   deque_copy_ring_caller(s,deque_physical(head,other,cap,false),moved,n,cap,true);
  }
  moved
 }
}
// write_slice into the free part of the ring.
fn deque_write_slice_caller(s:RawStorage,source:RawStorage,head:usize,index:usize,count:usize,cap:usize,zst:bool)
 requires raw_valid(s),s.cap==cap,raw_valid(source),source.cap==count,same_element(source,s),apart(source,s),
  zst || count==0 || (cap>0 && cap<=isize::MAX && head<cap && index<=cap && count<=cap),
{
 if count==0 || zst {
  return;
 }
 let physical=deque_physical(head,index,cap,zst);
 let room=crate::arithmetic::bounded::wrapping_sub(cap,physical);
 let first=if count<room {count} else {room};
 let rest=crate::arithmetic::bounded::wrapping_sub(count,first);
 // `copy_nonoverlapping(source.as_ptr(), ptr.add(physical), first)`,
 // `copy_nonoverlapping(source.as_ptr().add(first), ptr, count - first)`.
 let _destination=offset_storage(s,physical);
 distinct_nonoverlap_caller(source,s,0,physical,first);
 let _remaining=offset_storage(source,first);
 distinct_nonoverlap_caller(source,s,first,0,rest);
}
// try_append after the reservation: both source segments.
// `first` and `second` are the two slices returned by `other.as_slices()`.
fn deque_append_caller(s:RawStorage,first:RawStorage,second:RawStorage,head:usize,len:usize,cap:usize,a:usize,b:usize,zst:bool)->(out:usize)
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),len as int+a as int+b as int<=cap,
  raw_valid(first),first.cap==a,same_element(first,s),apart(first,s),
  raw_valid(second),second.cap==b,same_element(second,s),apart(second,s),
 ensures out==len+a+b,
{
 deque_write_slice_caller(s,first,head,len,a,cap,zst);
 let middle=crate::arithmetic::bounded::wrapping_add(len,a);
 deque_write_slice_caller(s,second,head,middle,b,cap,zst);
 crate::arithmetic::bounded::wrapping_add(len,crate::arithmetic::bounded::wrapping_add(a,b))
}
// try_split_off: the moved suffix read from the source ring and written to
// a fresh contiguous owner of at least `count` slots.
// `result` is the storage of the fresh owner.
fn deque_split_off_caller(s:RawStorage,result:RawStorage,head:usize,at:usize,len:usize,cap:usize,target:usize,zst:bool)->(out:usize)
 requires raw_valid(s),s.cap==cap,raw_valid(result),result.cap==target,same_element(s,result),apart(s,result),
  ring(head,len,cap,zst),at<=len,target>=len-at,
 ensures out==len-at,
{
 let count=crate::arithmetic::bounded::wrapping_sub(len,at);
 let pos=if count==0 {0} else {deque_physical(head,at,cap,zst)};
 let room=crate::arithmetic::bounded::wrapping_sub(cap,pos);
 let first=if count<room {count} else {room};
 let rest=crate::arithmetic::bounded::wrapping_sub(count,first);
 // `copy_nonoverlapping(ptr.add(pos), result.ptr, first)`,
 // `copy_nonoverlapping(ptr, result.ptr.add(first), count - first)`.
 let _source=offset_storage(s,pos);
 distinct_nonoverlap_caller(s,result,pos,0,first);
 let _destination=offset_storage(result,first);
 distinct_nonoverlap_caller(s,result,0,first,rest);
 count
}
// binary_search_by and partition_point for arbitrary callback replies
// (0 less, 1 greater, anything else equal).
fn deque_search_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool,replies:&[u8])->(out:usize)
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),replies@.len()>=len,
 ensures out<=len,
{
 let mut left=0usize;
 let mut right=len;
 while left<right
  invariant left<=right,right<=len,ring(head,len,cap,zst),replies@.len()>=len,raw_valid(s),s.cap==cap,
  decreases right-left,
 {
  let mid=crate::arithmetic::bounded::wrapping_add(left,crate::arithmetic::bounded::wrapping_sub(right,left)/2);
  let _slot=deque_get_caller(s,head,mid,len,cap,zst);
  if replies[mid]==0 {
   left=crate::arithmetic::bounded::wrapping_add(mid,1);
  } else if replies[mid]==1 {
   right=mid;
  } else {
   return mid;
  }
 }
 left
}
// try_retain_mut with the product compaction state: slots read and written,
// and the tail repair in Repair::drop.
fn deque_retain_caller(s:RawStorage,head:usize,len:usize,cap:usize,zst:bool,keeps:&[bool],stop:usize)->(out:usize)
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),keeps@.len()>=len,
 ensures out<=len,
{
 let mut state=crate::compaction::Compaction::full(len);
 let mut steps=0usize;
 while steps<stop
  invariant state.wf(),state@.original==len,ring(head,len,cap,zst),keeps@.len()>=len,raw_valid(s),s.cap==cap,
  decreases stop-steps,
 {
  steps=steps+1;
  match state.peek() {
   Some(read) => {
    let from=deque_get_caller(s,head,read,len,cap,zst);
    if keeps[read] {
     let write=state.written();
     state.keep_live();
     if read!=write {
      let to=deque_get_caller(s,head,write,len,cap,zst);
      // `copy(p, slot(write), 1)`.
      overlapping_copy_caller(s,from,to,1);
     }
    } else {
     state.remove_live();
    }
   }
   None => {break;}
  }
 }
 if state.written()!=state.read() && state.remaining()!=0 {
  let from=deque_physical(head,state.read(),cap,zst);
  let to=deque_physical(head,state.written(),cap,zst);
  if !zst {
   deque_copy_ring_caller(s,from,to,state.remaining(),cap,false);
  }
 }
 state.restored_len()
}
// Drain::drop (Finish): the shorter initialized side is moved into the hole.
// `len` is the ring length when the drain started: `end + tail`.
fn deque_drain_drop_caller(s:RawStorage,head:usize,start:usize,end:usize,tail:usize,len:usize,cap:usize,zst:bool)->(out:(usize,usize))
 requires raw_valid(s),s.cap==cap,ring(head,len,cap,zst),start<=end,end<=len,tail==len-end,
 ensures out.1==start+tail,ring(out.0,out.1,cap,zst),
{
 let removed=crate::arithmetic::bounded::wrapping_sub(end,start);
 let mut head=head;
 if removed!=0 {
  if start<tail {
   let from=deque_physical(head,0,cap,zst);
   let to=deque_physical(head,removed,cap,zst);
   if !zst {
    deque_copy_ring_caller(s,from,to,start,cap,true);
    head=crate::deque::geometry::physical(head,removed,cap);
   }
  } else if tail!=0 {
   let from=deque_physical(head,end,cap,zst);
   let to=deque_physical(head,start,cap,zst);
   if !zst {
    deque_copy_ring_caller(s,from,to,tail,cap,false);
   }
  }
 }
 (head,crate::arithmetic::bounded::wrapping_add(start,tail))
}
// Drop of the allocation guard: `storage::deallocate(ptr, cap.layout(), ..)`.
fn deque_deallocate_caller(s:RawStorage)->(bytes:usize)
 requires raw_valid(s),
 ensures doc_layout_from_size_align(bytes,s.align),
{
 layout_caller(s)
}
// into_vec after `normalize`, and try_from_std_in: the storage start passed to
// `Vec::from_raw_parts` (through `NonNull::new_unchecked` for the std vector).
fn deque_raw_parts_caller(s:RawStorage)->(address:usize)
 requires raw_valid(s),
 ensures doc_non_null_new_unchecked(address),
{
 representation_caller(s)
}
// write_clone_slice: `copy_nonoverlapping(source.as_ptr(), ptr.add(index), source.len())`.
fn deque_clone_slice_caller(s:RawStorage,source:RawStorage,index:usize,count:usize,cap:usize)
 requires raw_valid(s),s.cap==cap,raw_valid(source),source.cap==count,same_element(source,s),apart(source,s),
  index as int+count as int<=cap,
{
 let _destination=offset_storage(s,index);
 distinct_nonoverlap_caller(source,s,0,index,count);
}
// clone_to and try_clone_from: both source segments written from offset
// zero of storage that holds at least the source length.
fn deque_clone_slices_caller(s:RawStorage,first:RawStorage,second:RawStorage,a:usize,b:usize,cap:usize)
 requires raw_valid(s),s.cap==cap,a as int+b as int<=cap,
  raw_valid(first),first.cap==a,same_element(first,s),apart(first,s),
  raw_valid(second),second.cap==b,same_element(second,s),apart(second,s),
{
 deque_clone_slice_caller(s,first,0,a,cap);
 deque_clone_slice_caller(s,second,a,b,cap);
}
// try_extend_copy after the reservation.
fn deque_extend_copy_caller(s:RawStorage,source:RawStorage,head:usize,len:usize,count:usize,cap:usize,zst:bool)->(out:usize)
 requires raw_valid(s),s.cap==cap,raw_valid(source),source.cap==count,same_element(source,s),apart(source,s),
  ring(head,len,cap,zst),count>0,len as int+count as int<=cap,
 ensures out==len+count,
{
 if len==0 {
  deque_clone_slice_caller(s,source,0,count,cap);
 } else {
  deque_write_slice_caller(s,source,head,len,count,cap,zst);
 }
 crate::arithmetic::bounded::wrapping_add(len,count)
}
// try_resize for bitwise-clonable sized values after the reservation.
// `value` is the one-element storage of the borrowed value.
fn deque_resize_copy_caller(s:RawStorage,value:RawStorage,head:usize,len:usize,new_len:usize,cap:usize)
 requires raw_valid(s),s.cap==cap,raw_valid(value),value.cap==1,same_element(value,s),apart(value,s),
  ring(head,len,cap,false),len<new_len,new_len<=cap,
{
 let extra=crate::arithmetic::bounded::wrapping_sub(new_len,len);
 let tail=deque_physical(head,len,cap,false);
 let (first,second)=crate::deque::geometry::split(tail,extra,cap);
 let mut i=0usize;
 while i<first
  invariant i<=first,tail+first<=cap,raw_valid(s),s.cap==cap,raw_valid(value),value.cap==1,same_element(value,s),apart(value,s),
  decreases first-i,
 {
  let at=crate::arithmetic::bounded::wrapping_add(tail,i);
  // `copy_nonoverlapping(value, base.add(tail + i), 1)`.
  let _destination=offset_storage(s,at);
  distinct_nonoverlap_caller(value,s,0,at,1);
  i=i+1;
 }
 let mut i=0usize;
 while i<second
  invariant i<=second,second<=tail,tail<cap,raw_valid(s),s.cap==cap,raw_valid(value),value.cap==1,same_element(value,s),apart(value,s),
  decreases second-i,
 {
  // `copy_nonoverlapping(value, base.add(i), 1)`.
  let _destination=offset_storage(s,i);
  distinct_nonoverlap_caller(value,s,0,i,1);
  i=i+1;
 }
}
}
fn main() {}
