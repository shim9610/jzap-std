// Compatibility contracts call executable search/cursor functions unchanged.
use vstd::prelude::*;
// Node arrays (node.rs `Leaf.keys`, `Leaf.vals`, `Branch.edges`) as storages of
// the storage model, and the documented core condition of every pointer
// operation of the node primitives. Each index function below is proved for
// every valid node, so the callers after it pass only the index or span the
// product kernels computed. Stated premise: a live node is a `btree_node`, and
// two nodes of one map are `btree_same_map`. Element reads and writes are
// stated as one-element ranges. Initialization and aliasing are outside Verus.
verus! {
struct BtreeNode { keys:RawStorage, vals:RawStorage, edges:RawStorage }
spec fn btree_node(n:BtreeNode)->bool {
 raw_valid(n.keys) && n.keys.cap==11 && raw_valid(n.vals) && n.vals.cap==11
 && raw_valid(n.edges) && n.edges.cap==12
}
spec fn btree_same_map(a:BtreeNode,b:BtreeNode)->bool {
 same_element(a.keys,b.keys) && same_element(a.vals,b.vals) && same_element(a.edges,b.edges)
}
// key(node, i), val(node, i): the address alone; i may be the array length.
spec fn btree_pair_address(n:BtreeNode,i:usize)->bool {
 storage_range(n.keys,i,0) && storage_range(n.vals,i,0)
}
// key(node, i) and val(node, i) read, written or dropped in place.
spec fn btree_pair_slot(n:BtreeNode,i:usize)->bool {
 storage_range(n.keys,i,1) && storage_range(n.vals,i,1)
}
// pairs(src, from, dst, to, count): `ptr::copy` of both arrays.
spec fn btree_pair_copy(a:BtreeNode,from:usize,b:BtreeNode,to:usize,count:usize)->bool {
 doc_copy(allocation_of(a.keys),allocation_of(b.keys),slot_address(a.keys,from),slot_address(b.keys,to),count,a.keys.size,a.keys.align)
 && doc_copy(allocation_of(a.vals),allocation_of(b.vals),slot_address(a.vals,from),slot_address(b.vals,to),count,a.vals.size,a.vals.align)
 && btree_pair_address(a,from) && btree_pair_address(b,to)
}
// edge(node, i) read or written (child, set_child).
spec fn btree_edge_slot(n:BtreeNode,i:usize)->bool {
 storage_range(n.edges,i,1)
}
// edges(src, from, dst, to, count).
spec fn btree_edge_copy(a:BtreeNode,from:usize,b:BtreeNode,to:usize,count:usize)->bool {
 doc_copy(allocation_of(a.edges),allocation_of(b.edges),slot_address(a.edges,from),slot_address(b.edges,to),count,a.edges.size,a.edges.align)
 && storage_range(a.edges,from,0) && storage_range(b.edges,to,0)
}
// NodeView::keys and values: `from_raw_parts(key(node, 0), len(node))`.
spec fn btree_pair_prefix(n:BtreeNode,count:usize)->bool {
 storage_range(n.keys,0,count) && storage_range(n.vals,0,count)
}
fn btree_pair_address_index(i:usize)
 requires i<=11,
 ensures forall|n:BtreeNode| btree_node(n) ==> #[trigger] btree_pair_address(n,i),
{
 proof {
  assert forall|n:BtreeNode| btree_node(n) implies #[trigger] btree_pair_address(n,i) by {
   storage_range_condition(n.keys,i,0);storage_range_condition(n.vals,i,0);
  }
 }
}
fn btree_pair_index(i:usize)
 requires i<11,
 ensures forall|n:BtreeNode| btree_node(n) ==> #[trigger] btree_pair_slot(n,i),
{
 proof {
  assert forall|n:BtreeNode| btree_node(n) implies #[trigger] btree_pair_slot(n,i) by {
   storage_range_condition(n.keys,i,1);storage_range_condition(n.vals,i,1);
  }
 }
}
fn btree_pairs_index(from:usize,to:usize,count:usize)
 requires from as int+count as int<=11,to as int+count as int<=11,
 ensures forall|a:BtreeNode,b:BtreeNode| btree_node(a) && btree_node(b) && btree_same_map(a,b)
   ==> #[trigger] btree_pair_copy(a,from,b,to,count),
{
 proof {
  assert forall|a:BtreeNode,b:BtreeNode| btree_node(a) && btree_node(b) && btree_same_map(a,b)
    implies #[trigger] btree_pair_copy(a,from,b,to,count) by {
   storage_copy_condition(a.keys,b.keys,from,to,count);
   storage_copy_condition(a.vals,b.vals,from,to,count);
   storage_range_condition(a.keys,from,0);storage_range_condition(a.vals,from,0);
   storage_range_condition(b.keys,to,0);storage_range_condition(b.vals,to,0);
  }
 }
}
// edge(node, i): the address alone; i may be the array length.
fn btree_edge_address_index(i:usize)
 requires i<=12,
 ensures forall|n:BtreeNode| btree_node(n) ==> #[trigger] storage_range(n.edges,i,0),
{
 proof {
  assert forall|n:BtreeNode| btree_node(n) implies #[trigger] storage_range(n.edges,i,0) by {
   storage_range_condition(n.edges,i,0);
  }
 }
}
fn btree_edge_index(i:usize)
 requires i<12,
 ensures forall|n:BtreeNode| btree_node(n) ==> #[trigger] btree_edge_slot(n,i),
{
 proof {
  assert forall|n:BtreeNode| btree_node(n) implies #[trigger] btree_edge_slot(n,i) by {
   storage_range_condition(n.edges,i,1);
  }
 }
}
fn btree_edges_index(from:usize,to:usize,count:usize)
 requires from as int+count as int<=12,to as int+count as int<=12,
 ensures forall|a:BtreeNode,b:BtreeNode| btree_node(a) && btree_node(b) && btree_same_map(a,b)
   ==> #[trigger] btree_edge_copy(a,from,b,to,count),
{
 proof {
  assert forall|a:BtreeNode,b:BtreeNode| btree_node(a) && btree_node(b) && btree_same_map(a,b)
    implies #[trigger] btree_edge_copy(a,from,b,to,count) by {
   storage_copy_condition(a.edges,b.edges,from,to,count);
   storage_range_condition(a.edges,from,0);storage_range_condition(b.edges,to,0);
  }
 }
}
// node::replace_key: `ptr::replace(key(node, i), replacement)`.
fn btree_replace_index(i:usize)
 requires i<11,
 ensures forall|n:BtreeNode| btree_node(n) ==> #[trigger] doc_replace(allocation_of(n.keys),slot_address(n.keys,i),n.keys.size,n.keys.align),
{
 proof {
  assert forall|n:BtreeNode| btree_node(n) implies #[trigger] doc_replace(allocation_of(n.keys),slot_address(n.keys,i),n.keys.size,n.keys.align) by {
   storage_replace_condition(n.keys,i);
  }
 }
}
// node::set_len: the count is stored as `n as u16` and read back unchanged.
fn btree_count_index(n:usize)
 requires n<=11,
 ensures (n as u16) as usize==n,
{}
fn btree_view_index(count:usize)
 requires count<=11,
 ensures forall|n:BtreeNode| btree_node(n) ==> #[trigger] btree_pair_prefix(n,count),
{
 proof {
  assert forall|n:BtreeNode| btree_node(n) implies #[trigger] btree_pair_prefix(n,count) by {
   storage_range_condition(n.keys,0,count);storage_range_condition(n.vals,0,count);
  }
 }
}
}

verus! {
fn btree_actual_binary_step(low:usize,high:usize,less:bool)->(next:(usize,usize))
 requires low<high,
 ensures low<=next.0<=next.1<=high,next.1-next.0<high-low,
{crate::collections::btree_map::geometry::advance(low,high,
 crate::collections::btree_map::geometry::middle(low,high),less)}
fn btree_actual_cursor_pack(index:usize,height:usize)->(packed:u16)
 requires index<=11,height<usize::BITS,
 ensures packed<1024,(packed&15) as usize==index,(packed>>4) as usize==height,
{crate::collections::btree_map::geometry::pack(index,height)}
fn btree_actual_cursor_read(position:u16)->(result:(usize,usize))
 requires position<1024,position&15<=11,
 ensures result.0==(position&15) as usize,result.1==(position>>4) as usize,
{(crate::collections::btree_map::geometry::index(position),
 crate::collections::btree_map::geometry::height(position))}
fn btree_actual_cursor_update(position:u16,index:usize)->(updated:u16)
 requires position<1024,index<=11,
 ensures updated<1024,updated&15==index as u16,updated>>4==position>>4,
{crate::collections::btree_map::geometry::replace_index(position,index)}

fn btree_actual_leaves_deficit(maximum:usize,allocated:usize,retired:usize)->(next:usize)
 ensures next>=maximum,next as int+retired as int>=allocated as int,
{crate::collections::btree_map::geometry::allocation_deficit(maximum,allocated,retired)}
fn btree_actual_branches_deficit(maximum:usize,allocated:usize,retired:usize)->(next:usize)
 ensures next>=maximum,next as int+retired as int>=allocated as int,
{crate::collections::btree_map::geometry::allocation_deficit(maximum,allocated,retired)}
fn btree_actual_retired_count(count:usize,total_nodes:usize,input_length:usize)->(next:usize)
 requires count<total_nodes,total_nodes<=input_length,
 ensures next<=total_nodes,next<=input_length,next>count,
{crate::collections::btree_map::geometry::retired_successor(count)}

fn btree_actual_drop_skip(index:usize,count:usize,remaining:usize)->(next:(usize,usize))
 requires index<count,count<=11,remaining>0,
 ensures index<=next.0<count,1<=next.1<=remaining,next.1+(next.0-index)==remaining,
    (next.0 as u16) as usize==next.0,
{let skip=crate::collections::btree_map::geometry::drop_skip(index,count,remaining);
 (crate::arithmetic::bounded::wrapping_add(index,skip),
  crate::arithmetic::bounded::wrapping_sub(remaining,skip))}
fn btree_actual_leaf_edge(index:usize,count:usize,back:bool)->(next:(usize,usize))
 requires index<=count,count<=11,if back {index>0} else {index<count},
 ensures next.0<count,next.1<=count,(next.0 as u16) as usize==next.0,(next.1 as u16) as usize==next.1,
{crate::collections::btree_map::geometry::leaf_edge(index,count,back)}

fn btree_actual_delete_left(child:usize,sibling:usize,i:usize,parent:usize)->(borrow:bool)
 requires child==4,sibling<=11,0<parent<=11,i<=parent,
 ensures borrow == (i>0 && sibling as int+child as int+1>11),
    borrow ==> (i>0 && sibling>0 && child<11),
    (!borrow && i>0) ==> sibling+child+1<=11,
{crate::collections::btree_map::geometry::consider_left(i)
 && crate::collections::btree_map::geometry::borrow_required(sibling,child)}
fn btree_actual_delete_right(child:usize,sibling:usize,i:usize,parent:usize)->(borrow:bool)
 requires child==4,sibling<=11,0<parent<=11,i<=parent,
 ensures borrow == (i==0 && i<parent && sibling as int+child as int+1>11),
    borrow ==> (i==0 && i<parent && sibling>0 && child<11),
    (!borrow && i==0 && i<parent) ==> sibling+child+1<=11,
{crate::collections::btree_map::geometry::consider_right(i,parent)
 && crate::collections::btree_map::geometry::borrow_required(sibling,child)}
}

verus! {
// Compatibility contracts call the exact producers consumed by node.rs.
fn btree_actual_insert(old:usize,i:usize)
 requires old<11,i<=old,
{
 let(after,tail,length,edge_to)=crate::collections::btree_map::geometry::insert_span(old,i);
 // pairs(node, i, node, after, tail); key/val(node, i).write;
 // edges(node, after, node, edge_to, tail); edge(node, after).write.
 btree_pairs_index(i,after,tail);
 btree_pair_index(i);
 btree_edges_index(after,edge_to,tail);
 btree_edge_index(after);
 btree_count_index(length);
 btree_fix_caller(after,length);
}
fn btree_actual_split(old:usize,mid:usize)
 requires old==11,mid==4 || mid==5 || mid==6,
{
 let(start,count,edges)=crate::collections::btree_map::geometry::split_span(old,mid);
 // pairs(node, start, right, 0, n); edges(node, start, right, 0, edge_count);
 // key/val(node, mid).read.
 btree_pairs_index(start,0,count);
 btree_edges_index(start,0,edges);
 btree_pair_index(mid);
 btree_count_index(count);
 btree_count_index(mid);
 btree_fix_caller(0,count);
}
fn btree_actual_move_suffix(left_old:usize,index:usize)
 requires left_old<=11,index<=left_old,
{
 let(count,edge_start)=crate::collections::btree_map::geometry::suffix_span(left_old,index);
 // pairs(left, index, right, 0, n); edges(left, edge_start, right, 1, n).
 btree_pairs_index(index,0,count);
 btree_edges_index(edge_start,1,count);
 btree_count_index(index);
 btree_count_index(count);
 btree_fix_caller(1,count);
}
fn btree_actual_remove_leaf(old:usize,i:usize)
 requires 0<old<=11,i<old,
{
 let(after,tail,length)=crate::collections::btree_map::geometry::remove_span(old,i);
 // key/val(node, i).read; pairs(node, after, node, i, tail).
 btree_pair_index(i);
 btree_pairs_index(after,i,tail);
 btree_count_index(length);
}
fn btree_actual_merge(left_old:usize,right_old:usize,parent_old:usize,i:usize)
 requires left_old<=11,right_old<=11,left_old as int+right_old as int+1<=11,
    0<parent_old<=11,i<parent_old,
{
 let(start,length,edges)=crate::collections::btree_map::geometry::merge_span(left_old,right_old);
 let(after,tail,parent_length)=crate::collections::btree_map::geometry::remove_span(parent_old,i);
 let right_child=crate::arithmetic::bounded::wrapping_add(i,1);
 let edge_start=crate::arithmetic::bounded::wrapping_add(after,1);
 // child(parent, i), child(parent, i + 1); key/val(left, l).write(key/val(parent, i).read());
 // pairs(right, 0, left, start, r); edges(right, 0, left, start, edges);
 // pairs(parent, after, parent, i, tail); edges(parent, after + 1, parent, after, tail).
 btree_edge_index(i);
 btree_edge_index(right_child);
 btree_pair_index(left_old);
 btree_pair_index(i);
 btree_pairs_index(0,start,right_old);
 btree_edges_index(0,start,edges);
 btree_pairs_index(after,i,tail);
 btree_edges_index(edge_start,after,tail);
 btree_count_index(length);
 btree_count_index(parent_length);
 btree_fix_caller(start,length);
 btree_fix_caller(after,parent_length);
}
fn btree_actual_steal_left(left_old:usize,right_old:usize,parent_old:usize,i:usize)
 requires 0<left_old<=11,right_old<11,0<parent_old<=11,i<parent_old,
{
 let(left_length,right_length)=crate::collections::btree_map::geometry::steal_left_span(left_old,right_old);
 let right_child=crate::arithmetic::bounded::wrapping_add(i,1);
 let left_edge=crate::arithmetic::bounded::wrapping_add(left_length,1);
 // child(parent, i), child(parent, i + 1); pairs(right, 0, right, 1, r);
 // key/val(right, 0).write(key/val(parent, i).read()); key/val(parent, i).write(key/val(left, l).read());
 // edges(right, 0, right, 1, right_length); edge(right, 0).write(child(left, l + 1)).
 btree_edge_index(i);
 btree_edge_index(right_child);
 btree_pairs_index(0,1,right_old);
 btree_pair_index(0);
 btree_pair_index(i);
 btree_pair_index(left_length);
 btree_edges_index(0,1,right_length);
 btree_edge_index(0);
 btree_edge_index(left_edge);
 btree_count_index(left_length);
 btree_count_index(right_length);
 btree_fix_caller(0,right_length);
}
fn btree_actual_steal_right(left_old:usize,right_old:usize,parent_old:usize,i:usize)
 requires left_old<11,0<right_old<=11,0<parent_old<=11,i<parent_old,
{
 let(left_length,right_length)=crate::collections::btree_map::geometry::steal_right_span(left_old,right_old);
 let right_child=crate::arithmetic::bounded::wrapping_add(i,1);
 // child(parent, i), child(parent, i + 1); key/val(left, l).write(key/val(parent, i).read());
 // key/val(parent, i).write(key/val(right, 0).read()); pairs(right, 1, right, 0, right_length);
 // set_child(left, left_length, child(right, 0)); edges(right, 1, right, 0, r).
 btree_edge_index(i);
 btree_edge_index(right_child);
 btree_pair_index(left_old);
 btree_pair_index(i);
 btree_pair_index(0);
 btree_pairs_index(1,0,right_length);
 btree_edge_index(left_length);
 btree_edge_index(0);
 btree_edges_index(1,0,right_old);
 btree_count_index(left_length);
 btree_count_index(right_length);
 btree_fix_caller(0,right_length);
}
// node::fix visits child slots from..=to with the bounded add; every visited
// index is an edge slot and narrows exactly to the stored parent index.
fn btree_fix_caller(from:usize,to:usize)
 requires to<=11,
{
 let mut i=from;
 while i<=to
  invariant to<=11,
  decreases to as int+1-i as int,
 {
  // set_child(node, i, child(node, i)).
  btree_edge_index(i);
  assert((i as u16) as usize==i);
  i=crate::arithmetic::bounded::wrapping_add(i,1);
 }
}
// NodeGuard::drain: progress is published before each use, so the index used
// for destroy/drop_pair is the decremented prefix length.
fn btree_drain_caller(children:usize,pairs:usize,height:usize)
 requires children<=12,pairs<=11,children>0 ==> height>0,
{
 let mut children=children;let mut pairs=pairs;
 while children>0
  invariant children<=12,children>0 ==> height>0,
  decreases children,
 {
  children=crate::arithmetic::bounded::wrapping_sub(children,1);
  let below=crate::arithmetic::bounded::wrapping_sub(height,1);
  // destroy(child(node, children), height - 1, ..).
  btree_edge_index(children);
  assert(below<height);
 }
 while pairs>0
  invariant pairs<=11,
  decreases pairs,
 {
  pairs=crate::arithmetic::bounded::wrapping_sub(pairs,1);
  // drop_pair(node, pairs).
  btree_pair_index(pairs);
 }
}
// node::destroy builds the guard prefix from the closed node count.
fn btree_destroy_caller(n:usize,height:usize)
 requires n<=11,
{
 let children=if height==0 {0} else {crate::arithmetic::bounded::wrapping_add(n,1)};
 btree_drain_caller(children,n,height);
}
}

verus! {
// Owner-level callers (btree_map.rs). Node counts, parent indices and the
// height are arbitrary values inside the stated tree premises; every index
// expression below is the product's own bounded add/sub or geometry call.
pub open spec fn btree_cursor_word(position:u16,index:usize,height:usize)->bool {
 position<1024 && (position&15) as usize==index && (position>>4) as usize==height
}
// BTreeMap::search inside one node, for arbitrary comparison replies
// (0 equal, 1 less, anything else greater). Result tag: 0 found pair,
// 1 vacant leaf edge, 2 descent through a child edge.
fn btree_search_node_caller(count:usize,h:usize,replies:&[u8])->(out:(u8,usize,u16))
 requires count<=11,h<usize::BITS,replies@.len()>=count,
 ensures out.0<=2,
    out.0==0 ==> out.1<count && btree_cursor_word(out.2,out.1,h),
    out.0==1 ==> out.1<=count && h==0 && btree_cursor_word(out.2,out.1,0),
    out.0==2 ==> out.1<=count && out.1<12 && h>0,
{
 let mut i=0;
 while i<count
  invariant i<=count,count<=11,h<usize::BITS,replies@.len()>=count,
  decreases count-i,
 {
  btree_pair_index(i);
  let reply=replies[i];
  if reply==0 {
   return (0,i,crate::collections::btree_map::geometry::pack(i,h));
  } else if reply==1 {
   break;
  } else {
   i=crate::arithmetic::bounded::wrapping_add(i,1);
  }
 }
 if h==0 {
  return (1,i,crate::collections::btree_map::geometry::pack(i,0));
 }
 let _below=crate::arithmetic::bounded::wrapping_sub(h,1);
 (2,i,0)
}
// A found cursor is read back through the product accessors and names an
// initialized pair slot of its node.
fn btree_found_slot_caller(count:usize,h:usize,replies:&[u8])->(index:usize)
 requires count<=11,h<usize::BITS,replies@.len()>=count,
 ensures index<=11,
{
 let out=btree_search_node_caller(count,h,replies);
 if out.0==0 {
  let index=crate::collections::btree_map::geometry::index(out.2);
  assert(index<count);
  btree_pair_index(index);
  index
 } else {0}
}
// try_replace_key: `node::replace_key(c.node, c.index(), key)` on a found cursor.
fn btree_replace_key_caller(count:usize,h:usize,replies:&[u8])
 requires count<=11,h<usize::BITS,replies@.len()>=count,
{
 let out=btree_search_node_caller(count,h,replies);
 if out.0==0 {
  let index=crate::collections::btree_map::geometry::index(out.2);
  btree_replace_index(index);
 }
}
// try_insert_at/commit_fit: the vacant leaf edge from search and the product's
// `len < CAP` test satisfy node::insert.
fn btree_commit_fit_caller(count:usize,replies:&[u8])
 requires count<=11,replies@.len()>=count,
{
 let out=btree_search_node_caller(count,0,replies);
 if out.0==1 {
  let index=crate::collections::btree_map::geometry::index(out.2);
  if count<11 {
   btree_actual_insert(count,index);
  }
 }
}
// node::splitpoint restated; the product body is compared as text.
fn btree_splitpoint(edge:usize)->(mid:usize)
 ensures mid==if edge<5 {4usize} else if edge<=6 {5usize} else {6usize},
{
 if edge<5 {4} else if edge<=6 {5} else {6}
}
// plan_insert: `levels` full ancestors are climbed; the remaining arguments
// are the header values read at the top of the full chain.
fn btree_plan_caller(height:usize,levels:usize,has_parent:bool,parent_index:usize)->(reserved:usize)
 requires height<usize::BITS,levels<=height,
    has_parent ==> levels<height && parent_index<=11,
 ensures reserved<=levels+1,
{
 let mut h=0usize;
 while h<levels
  invariant h<=levels,levels<=height,height<usize::BITS,
  decreases levels-h,
 {
  h=crate::arithmetic::bounded::wrapping_add(h,1);
 }
 if !has_parent {
  if height>=crate::arithmetic::bounded::wrapping_sub(usize::BITS as usize,1) {
   return 0;
  }
  let new_height=crate::arithmetic::bounded::wrapping_add(height,1);
  let _stop=crate::collections::btree_map::geometry::pack(0,new_height);
 } else {
  let _stop=crate::collections::btree_map::geometry::pack(parent_index,
   crate::arithmetic::bounded::wrapping_add(h,1));
 }
 let mut reserved=0usize;
 loop
  invariant_except_break h<=levels,reserved+h==levels,
  invariant levels<=height,height<usize::BITS,
  ensures reserved<=levels+1,
  decreases h,
 {
  reserved=crate::arithmetic::bounded::wrapping_add(reserved,1);
  if h==0 {break;}
  h=crate::arithmetic::bounded::wrapping_sub(h,1);
 }
 reserved
}
// commit_insert, one consumed spare: the current node is full, `edge` is the
// cursor index inside it and `level` its height.
fn btree_commit_split_caller(edge:usize,level:usize,height:usize,has_parent:bool,parent_index:usize)
 requires edge<=11,level<height,height<usize::BITS,has_parent ==> parent_index<=11,
{
 let mid=btree_splitpoint(edge);
 btree_actual_split(11,mid);
 let (left,right,_)=crate::collections::btree_map::geometry::split_span(11,mid);
 if edge<=mid {
  btree_actual_insert(mid,edge);
  if level==0 {let _c=crate::collections::btree_map::geometry::pack(edge,0);}
 } else {
  let index=crate::arithmetic::bounded::wrapping_sub(edge,
   crate::arithmetic::bounded::wrapping_add(mid,1));
  btree_actual_insert(right,index);
  if level==0 {let _c=crate::collections::btree_map::geometry::pack(index,0);}
 }
 if has_parent {
  let _c=crate::collections::btree_map::geometry::pack(parent_index,
   crate::arithmetic::bounded::wrapping_add(level,1));
 } else {
  // set_child(plan.stop.node, 0, cur.node)
  btree_fix_caller(0,0);
 }
}
// commit_insert, final insertion into the node that plan_insert found not
// full (or the fresh empty root, where count and index are zero).
fn btree_commit_final_caller(count:usize,index:usize)
 requires count<11,index<=count,
{
 btree_actual_insert(count,index);
}
// remove_at up to the leaf removal. `pred_*` describe the predecessor leaf
// returned by Cursor::last when the cursor is internal.
fn btree_remove_start_caller(index:usize,count:usize,level:usize,pred_index:usize,pred_count:usize,
 watch:usize,length:usize)->(out:usize)
 requires index<count,count<=11,level<usize::BITS,pred_index<pred_count,pred_count<=11,length>0,
    watch<=11,
 ensures out as int==length as int-1,
{
 let mut leaf_index=index;let mut leaf_count=count;
 if level>0 {
  btree_edge_index(index);
  let _below=crate::arithmetic::bounded::wrapping_sub(level,1);
  btree_pair_index(index);
  btree_pair_index(pred_index);
  leaf_index=pred_index;leaf_count=pred_count;
 }
 btree_actual_remove_leaf(leaf_count,leaf_index);
 if watch>leaf_index {
  let moved=crate::arithmetic::bounded::wrapping_sub(watch,1);
  assert(moved<=11);
 }
 crate::arithmetic::bounded::wrapping_sub(length,1)
}
// remove_at, one rebalancing iteration for a node below MIN. `left`/`right`
// are the sibling counts read through the parent; `watch` is the index of a
// watch cursor in whichever node the product compares it with.
fn btree_rebalance_caller(count:usize,parent:usize,i:usize,left:usize,right:usize,level:usize,
 height:usize,watch:usize)->(next:usize)
 requires count<5,0<parent<=11,i<=parent,left<=11,right<=11,level<height,height<usize::BITS,
 ensures next as int==level as int+1,
{
 if crate::collections::btree_map::geometry::consider_left(i)
  && crate::collections::btree_map::geometry::borrow_required(left,count)
 {
  let sep=crate::arithmetic::bounded::wrapping_sub(i,1);
  btree_edge_index(sep);
  let last=crate::arithmetic::bounded::wrapping_sub(left,1);
  if watch<count {
   let moved=crate::arithmetic::bounded::wrapping_add(watch,1);
   assert(moved<=11);
  }
  let _first=crate::collections::btree_map::geometry::pack(0,level);
  let _up=crate::collections::btree_map::geometry::pack(sep,
   crate::arithmetic::bounded::wrapping_add(level,1));
  btree_actual_steal_left(left,count,parent,sep);
  return crate::arithmetic::bounded::wrapping_add(level,1);
 }
 if crate::collections::btree_map::geometry::consider_right(i,parent)
  && crate::collections::btree_map::geometry::borrow_required(right,count)
 {
  let edge=crate::arithmetic::bounded::wrapping_add(i,1);
  btree_edge_index(edge);
  let _moved=crate::collections::btree_map::geometry::pack(count,level);
  let _up=crate::collections::btree_map::geometry::pack(i,
   crate::arithmetic::bounded::wrapping_add(level,1));
  if 0<watch && watch<right {
   let moved=crate::arithmetic::bounded::wrapping_sub(watch,1);
   assert(moved<=11);
  }
  btree_actual_steal_right(count,right,parent,i);
  return crate::arithmetic::bounded::wrapping_add(level,1);
 }
 let sep=if i>0 {crate::arithmetic::bounded::wrapping_sub(i,1)} else {0};
 let edge=crate::arithmetic::bounded::wrapping_add(sep,1);
 btree_edge_index(sep);
 btree_edge_index(edge);
 let l=if i>0 {left} else {count};
 let r=if i>0 {count} else {right};
 if watch<r {
  let joined=crate::arithmetic::bounded::wrapping_add(
   crate::arithmetic::bounded::wrapping_add(l,1),watch);
  let _c=crate::collections::btree_map::geometry::pack(joined,level);
 }
 let _separator=crate::collections::btree_map::geometry::pack(l,level);
 if sep<watch && watch<parent {
  let moved=crate::arithmetic::bounded::wrapping_sub(watch,1);
  assert(moved<=11);
 }
 btree_actual_merge(l,r,parent,sep);
 crate::arithmetic::bounded::wrapping_add(level,1)
}
// shrink_root: the loop test `height > 0` admits the decrement.
fn btree_shrink_caller(height:usize)->(next:usize)
 requires height>0,
 ensures next<height,
{
 btree_edge_index(0);
 crate::arithmetic::bounded::wrapping_sub(height,1)
}
}

verus! {
// Traversal callers (iter.rs). `counts[k]` is the closed count of the node
// reached after k descents; parent indices and counts are header values.
// Cursor::first, Cursor::last and BorrowState::edge.
fn btree_descend_caller(height:usize,counts:&[usize],back:bool)->(edge:usize)
 requires height<usize::BITS,counts@.len()>height,
    forall|j:int| 0<=j<counts@.len() ==> #[trigger] counts@[j]<=11,
 ensures edge<=counts@[height as int],edge<=11,
{
 let mut h=height;let mut k=0usize;
 while h>0
  invariant k+h==height,height<usize::BITS,counts@.len()>height,
    forall|j:int| 0<=j<counts@.len() ==> #[trigger] counts@[j]<=11,
  decreases h,
 {
  let count=counts[k];
  let child=if back {count} else {0};
  btree_edge_index(child);
  h=crate::arithmetic::bounded::wrapping_sub(h,1);
  k=crate::arithmetic::bounded::wrapping_add(k,1);
 }
 let n=counts[k];
 if n!=0 {
  let _first=crate::collections::btree_map::geometry::pack(0,0);
  let last=crate::arithmetic::bounded::wrapping_sub(n,1);
  assert(last<n);
  let _last=crate::collections::btree_map::geometry::pack(last,0);
 }
 if back {n} else {0}
}
// Cursor::next, Cursor::prev and the non-final branch of State::advance_owned
// from an initialized pair cursor. `climbed` is the level of the node whose
// parent header (parent_index, parent_count) was just read.
fn btree_cursor_step_caller(index:usize,count:usize,level:usize,height:usize,back:bool,
 parent_index:usize,parent_count:usize,climbed:usize)->(next:(bool,usize))
 requires index<count,count<=11,level<=height,height<usize::BITS,
    parent_index<=parent_count,parent_count<=11,climbed<height,
 ensures next.0 ==> next.1<11,
{
 if level>0 {
  let child=if back {index} else {crate::arithmetic::bounded::wrapping_add(index,1)};
  btree_edge_index(child);
  let _below=crate::arithmetic::bounded::wrapping_sub(level,1);
  return (false,0);
 }
 if back && index>0 {
  let at=crate::arithmetic::bounded::wrapping_sub(index,1);
  let _c=crate::collections::btree_map::geometry::pack(at,0);
  return (true,at);
 }
 if !back && crate::arithmetic::bounded::wrapping_add(index,1)<count {
  let at=crate::arithmetic::bounded::wrapping_add(index,1);
  let _c=crate::collections::btree_map::geometry::pack(at,0);
  return (true,at);
 }
 let h=crate::arithmetic::bounded::wrapping_add(climbed,1);
 if back && parent_index>0 {
  let at=crate::arithmetic::bounded::wrapping_sub(parent_index,1);
  let _c=crate::collections::btree_map::geometry::pack(at,h);
  assert(at<parent_count);
  return (true,at);
 }
 if !back && parent_index<parent_count {
  let _c=crate::collections::btree_map::geometry::pack(parent_index,h);
  return (true,parent_index);
 }
 (false,0)
}
// State::take and the head of State::advance_owned: one pair leaves a
// nonempty interval.
fn btree_state_take_caller(remaining:usize)->(next:usize)
 requires remaining>0,
 ensures next as int==remaining as int-1,
{
 crate::arithmetic::bounded::wrapping_sub(remaining,1)
}
// State::advance_owned, retirement of the ancestor chain: a node that has a
// parent lies below the root.
fn btree_retire_chain_caller(level:usize,height:usize)->(next:usize)
 requires level<height,height<usize::BITS,
 ensures next<=height,
{
 crate::arithmetic::bounded::wrapping_add(level,1)
}
// BorrowState::remaining: a back pair cursor in a leaf becomes the edge after
// it; the packed word is decoded by the product accessors.
fn btree_borrow_remaining_caller(index:usize,count:usize,level:usize)->(edge:(usize,usize))
 requires index<count,count<=11,level<usize::BITS,
 ensures edge.1==level,level==0 ==> edge.0==index+1 && edge.0<=count,level>0 ==> edge.0==index,
{
 let mut position=crate::collections::btree_map::geometry::pack(index,level);
 if crate::collections::btree_map::geometry::height(position)==0 {
  position=crate::collections::btree_map::geometry::replace_index(position,
   crate::arithmetic::bounded::wrapping_add(crate::collections::btree_map::geometry::index(position),1));
 }
 (crate::collections::btree_map::geometry::index(position),
  crate::collections::btree_map::geometry::height(position))
}
// BorrowState::interval: an internal endpoint pair is replaced by the leaf
// edge next to it.
fn btree_borrow_interval_caller(index:usize,count:usize,level:usize,front:bool)
 requires index<count,count<=11,level>0,
{
 let child=if front {index} else {crate::arithmetic::bounded::wrapping_add(index,1)};
 btree_edge_index(child);
 let _below=crate::arithmetic::bounded::wrapping_sub(level,1);
}
// BorrowState::take: the yielded position names an initialized pair.
fn btree_borrow_take_caller(index:usize,count:usize,level:usize,height:usize,remaining:usize,
 back:bool,last:bool,exact:bool,parent_index:usize,parent_count:usize,climbed:usize)->(out:(bool,usize,usize))
 requires remaining>0,count<=11,level<=height,height<usize::BITS,
    level==0 ==> index<=count,level>0 ==> index<count,
    parent_index<=parent_count,parent_count<=11,climbed<height,
 ensures out.0 ==> out.1<11,exact ==> out.2 as int==remaining as int-1,
{
 let mut yielded=(false,0usize);
 if level>0 {
  if !last {
   let child=if back {index} else {crate::arithmetic::bounded::wrapping_add(index,1)};
   btree_edge_index(child);
   let _below=crate::arithmetic::bounded::wrapping_sub(level,1);
  }
  yielded=(true,index);
 } else if (back && index>0) || (!back && index<count) {
  let (i,edge)=crate::collections::btree_map::geometry::leaf_edge(index,count,back);
  assert(i<count && edge<=count);
  yielded=(true,i);
 } else {
  let h=crate::arithmetic::bounded::wrapping_add(climbed,1);
  if (back && parent_index>0) || (!back && parent_index<parent_count) {
   let at=if back {crate::arithmetic::bounded::wrapping_sub(parent_index,1)} else {parent_index};
   assert(at<parent_count);
   if !last {
    let child=if back {at} else {crate::arithmetic::bounded::wrapping_add(at,1)};
    btree_edge_index(child);
    let _below=crate::arithmetic::bounded::wrapping_sub(h,1);
   }
   yielded=(true,at);
  }
 }
 let left=if exact {crate::arithmetic::bounded::wrapping_sub(remaining,1)} else {remaining};
 (yielded.0,yielded.1,left)
}
}

verus! {
// Callers for operations.rs.
// ordered: doubled in-order coordinates and height alignment.
fn btree_ordered_caller(a:usize,b:usize,ah:usize,bh:usize,parent:usize)->(out:(usize,usize))
 requires a<=11,b<=11,ah<usize::BITS,bh<usize::BITS,parent<=11,
 ensures out.0<=23,out.1<=23,
{
 let ai=crate::arithmetic::bounded::wrapping_add(a,crate::arithmetic::bounded::wrapping_add(a,1));
 let bi=crate::arithmetic::bounded::wrapping_add(b,crate::arithmetic::bounded::wrapping_add(b,1));
 let mut ah=ah;let mut bh=bh;
 while ah<bh
  invariant ah<usize::BITS,bh<usize::BITS,parent<=11,
  decreases bh as int-ah as int,
 {
  let _up=crate::arithmetic::bounded::wrapping_add(parent,parent);
  ah=crate::arithmetic::bounded::wrapping_add(ah,1);
 }
 while bh<ah
  invariant ah<usize::BITS,bh<usize::BITS,parent<=11,
  decreases ah as int-bh as int,
 {
  let _up=crate::arithmetic::bounded::wrapping_add(parent,parent);
  bh=crate::arithmetic::bounded::wrapping_add(bh,1);
 }
 let _up=crate::arithmetic::bounded::wrapping_add(parent,parent);
 (ai,bi)
}
// fix_border, one level: the border separator and its two children. The
// product reads the child counts again after every borrow; here they are the
// span results of the same call.
fn btree_fix_border_caller(count:usize,level:usize,a:usize,b:usize,left:bool)->(below:usize)
 requires 0<count<=11,level>0,a<=11,b<=11,
 ensures below<level,
{
 let sep=if left {0} else {crate::arithmetic::bounded::wrapping_sub(count,1)};
 let ch=crate::arithmetic::bounded::wrapping_sub(level,1);
 let edge=crate::arithmetic::bounded::wrapping_add(sep,1);
 btree_edge_index(sep);
 btree_edge_index(edge);
 if crate::arithmetic::bounded::wrapping_add(crate::arithmetic::bounded::wrapping_add(a,1),b)<=11 {
  btree_actual_merge(a,b,count,sep);
 } else {
  let mut a=a;let mut b=b;
  if left {
   while a<6
    invariant a<=11,b<=11,a as int+b as int+1>11,0<count<=11,sep<count,
    decreases 6-a,
   {
    btree_actual_steal_right(a,b,count,sep);
    let next=crate::collections::btree_map::geometry::steal_right_span(a,b);
    a=next.0;b=next.1;
   }
  } else {
   while b<6
    invariant a<=11,b<=11,a as int+b as int+1>11,0<count<=11,sep<count,
    decreases 6-b,
   {
    btree_actual_steal_left(a,b,count,sep);
    let next=crate::collections::btree_map::geometry::steal_left_span(a,b);
    a=next.0;b=next.1;
   }
  }
 }
 ch
}
// count_subtree: child edges 0..=count and the checked running total. The
// bound `limit` is the map length that the subtree is part of.
fn btree_count_caller(count:usize,level:usize,children:&[usize],limit:usize)->(total:usize)
 requires count<=11,count<=limit,children@.len()==count+1,
    forall|j:int| 1<=j<=count+1 ==> count as int+#[trigger] btree_prefix_sum(children@,j)<=limit,
 ensures total as int==count as int+(if level>0 {btree_prefix_sum(children@,count as int+1)} else {0}),
{
 let mut total=count;
 if level>0 {
  let _below=crate::arithmetic::bounded::wrapping_sub(level,1);
  let mut i=0usize;
  while i<=count
   invariant i<=count+1,count<=11,children@.len()==count+1,
    total as int==count as int+btree_prefix_sum(children@,i as int),
    forall|j:int| 1<=j<=count+1 ==> count as int+#[trigger] btree_prefix_sum(children@,j)<=limit,
   decreases count+1-i,
  {
   btree_edge_index(i);
   assert(count as int+btree_prefix_sum(children@,i as int+1)<=limit);
   total=crate::arithmetic::bounded::wrapping_add(total,children[i]);
   i=i+1;
  }
 }
 total
}
pub open spec fn btree_prefix_sum(values:Seq<usize>,end:int)->int
 decreases end,
{
 if end<=0 {0} else {btree_prefix_sum(values,end-1)+values[end-1] as int}
}
// try_split_off: the key scan of one path level and the bookkeeping around it.
fn btree_split_path_caller(count:usize,level:usize,height:usize,replies:&[bool])->(slot:u16)
 requires count<=11,level<=height,height<usize::BITS,replies@.len()>=count,
 ensures slot<=count,slot as usize<=11,
{
 let mut i=0usize;
 while i<count && replies[i]
  invariant i<=count,count<=11,replies@.len()>=count,
  decreases count-i,
 {
  btree_pair_index(i);
  i=crate::arithmetic::bounded::wrapping_add(i,1);
 }
 let slot=i as u16;
 assert(slot as usize==i);
 if level<height {
  btree_edge_index(i);
  let _below=crate::arithmetic::bounded::wrapping_sub(crate::arithmetic::bounded::wrapping_sub(height,level),1);
  let _next=crate::arithmetic::bounded::wrapping_add(level,1);
 }
 slot
}
// try_split_off: the empty right spine, the suffix move of one level and the
// two final lengths. `count` is the node count the stored path slot was
// scanned in; `moved` is the recounted side.
fn btree_split_off_caller(right_height:usize,height:usize,slot:u16,count:usize,level:usize,
 old:usize,moved:usize)
 requires right_height<height,height<usize::BITS,slot as usize<=count,count<=11,level<=height,moved<=old,
{
 let _taller=crate::arithmetic::bounded::wrapping_add(right_height,1);
 btree_fix_caller(0,0);
 let _levels=crate::arithmetic::bounded::wrapping_add(height,1);
 let i=slot as usize;
 btree_actual_move_suffix(count,i);
 if level>0 {
  btree_edge_index(i);
  let _below=crate::arithmetic::bounded::wrapping_sub(level,1);
 }
 let _rest=crate::arithmetic::bounded::wrapping_sub(old,moved);
}
// lower/upper from a vacant leaf edge.
fn btree_bound_caller(index:usize,count:usize,upper:bool,parent_index:usize,parent_count:usize,
 climbed:usize,height:usize)->(out:(bool,usize))
 requires index<=count,count<=11,parent_index<=parent_count,parent_count<=11,climbed<height,
    height<usize::BITS,
 ensures out.0 ==> out.1<11,
{
 if !upper && index<count {
  return (true,index);
 }
 if upper && index>0 {
  let at=crate::arithmetic::bounded::wrapping_sub(index,1);
  let _c=crate::collections::btree_map::geometry::pack(at,0);
  return (true,at);
 }
 let h=crate::arithmetic::bounded::wrapping_add(climbed,1);
 if !upper && parent_index<parent_count {
  let _c=crate::collections::btree_map::geometry::pack(parent_index,h);
  return (true,parent_index);
 }
 if upper && parent_index>0 {
  let at=crate::arithmetic::bounded::wrapping_sub(parent_index,1);
  let _c=crate::collections::btree_map::geometry::pack(at,h);
  return (true,at);
 }
 (false,0)
}
// clone_node: the guard prefix after `pairs_done` pair steps and
// `children_done` child steps is a prefix that NodeGuard::drain accepts.
fn btree_clone_caller(n:usize,height:usize,pairs_stop:usize,children_stop:usize)
 requires n<=11,pairs_stop<=n,children_stop<=n+1,
{
 let mut pairs=0usize;let mut children=0usize;
 while pairs<n && pairs<pairs_stop
  invariant pairs<=n,n<=11,
  decreases n-pairs,
 {
  let i=pairs;
  btree_pair_index(i);
  pairs=crate::arithmetic::bounded::wrapping_add(i,1);
  assert((pairs as u16) as usize==pairs);
 }
 if pairs==n && height>0 {
  while children<=n && children<children_stop
   invariant children<=n+1,n<=11,height>0,
   decreases n+1-children,
  {
   let i=children;
   btree_edge_index(i);
   assert((i as u16) as usize==i);
   let _below=crate::arithmetic::bounded::wrapping_sub(height,1);
   children=crate::arithmetic::bounded::wrapping_add(i,1);
  }
 }
 btree_drain_caller(children,pairs,height);
}
}

verus! {
// Callers for bulk.rs and view.rs.
// pillar_from_pool: an empty spine of the requested height.
fn btree_pillar_caller(height:usize)
 requires height<usize::BITS,
{
 let mut h=0usize;
 while h<height
  invariant h<=height,height<usize::BITS,
  decreases height-h,
 {
  btree_fix_caller(0,0);
  h=crate::arithmetic::bounded::wrapping_add(h,1);
 }
}
// allocated_bytes: child edges 0..=count of one internal node.
fn btree_bytes_caller(count:usize,level:usize)
 requires count<=11,
{
 if level>0 {
  let mut i=0usize;
  while i<=count
   invariant i<=count+1,count<=11,level>0,
   decreases count+1-i,
  {
   btree_edge_index(i);
   let _below=crate::arithmetic::bounded::wrapping_sub(level,1);
   i=i+1;
  }
 }
}
// bulk_push. `path[k]` is the count of the k-th ancestor of the rightmost
// leaf (path[0] is the leaf); the last entry is the root.
fn btree_bulk_push_caller(first:bool,path:&[usize],length:usize)->(next:usize)
 requires path@.len()>0,path@.len()<usize::BITS,length<usize::MAX,
    forall|j:int| 0<=j<path@.len() ==> #[trigger] path@[j]<=11,
 ensures first ==> next==1,!first ==> next as int==length as int+1,
{
 if first {
  btree_actual_insert(0,0);
  return 1;
 }
 let n=path[0];
 if n<11 {
  btree_actual_insert(n,n);
 } else {
  let mut height=0usize;
  while path[height]==11
   invariant height<path@.len(),path@.len()<usize::BITS,path@[0]==11,
   decreases path@.len()-height,
  {
   if height+1<path.len() {
    height=crate::arithmetic::bounded::wrapping_add(height,1);
   } else {
    break;
   }
  }
  let new_root=path[height]==11;
  let target_height=if new_root {crate::arithmetic::bounded::wrapping_add(height,1)} else {height};
  let pillar=crate::arithmetic::bounded::wrapping_sub(target_height,1);
  btree_pillar_caller(pillar);
  if new_root {
   btree_fix_caller(0,0);
   btree_actual_insert(0,0);
  } else {
   let count=path[height];
   btree_actual_insert(count,count);
  }
  let mut h=pillar;
  while h>0
   invariant h<=pillar,
   decreases h,
  {
   btree_edge_index(0);
   h=crate::arithmetic::bounded::wrapping_sub(h,1);
  }
 }
 crate::arithmetic::bounded::wrapping_add(length,1)
}
// fix_packed_border, one level: the last separator, its full left sibling
// and the possibly short rightmost child.
fn btree_packed_border_caller(count:usize,level:usize,left:usize,right:usize)->(below:usize)
 requires 0<count<=11,level>0,left==11,right<=11,
 ensures below<level,
{
 let i=crate::arithmetic::bounded::wrapping_sub(count,1);
 let edge=crate::arithmetic::bounded::wrapping_add(i,1);
 btree_edge_index(edge);
 let mut left=left;let mut right=right;
 while right<5
  invariant left<=11,right<=11,left as int+right as int>=11,0<count<=11,i<count,
  decreases 5-right,
 {
  btree_actual_steal_left(left,right,count,i);
  let next=crate::collections::btree_map::geometry::steal_left_span(left,right);
  left=next.0;right=next.1;
 }
 crate::arithmetic::bounded::wrapping_sub(level,1)
}
// NodeView::child: the public index is checked against the node count.
fn btree_view_child_caller(index:usize,count:usize,level:usize)->(out:bool)
 requires count<=11,
 ensures out ==> index<12 && level>0,
{
 if level==0 {
  return false;
 }
 if index>count {
  return false;
 }
 let _below=crate::arithmetic::bounded::wrapping_sub(level,1);
 true
}
}
