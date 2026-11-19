use vstd::prelude::*;
verus! {
// Numerical ownership transitions. Typed allocation/provenance and actual
// element drops use Rust/Miri/ASan, not integer axioms in this model.
proof fn btree_insert(n:usize,i:usize)
    requires n<11,i<=n,
    ensures i+1<=11,n-i<=11,i+1+(n-i)<=11,n+1<=11,
{}
proof fn btree_split(mid:usize,edge:usize)
    requires edge<=11,mid==if edge<5 {4usize} else if edge<=6 {5usize} else {6usize},
    ensures mid<11,mid+1<=11,11-(mid+1)<=11,
        mid>=4,11-(mid+1)>=4,
        if edge<=mid {5<=mid+1<=7 && edge<=mid}
        else {5<=11-(mid+1)+1<=7 && edge-(mid+1)<=11-(mid+1)},
{}
proof fn btree_remove(n:usize,i:usize)
    requires 0<n<=11,i<n,
    ensures i+1<=n,n-(i+1)<=11,n-1<=11,i+(n-(i+1))<=11,
{}
proof fn btree_merge(l:usize,r:usize,p:usize,i:usize)
    requires l<=11,r<=11,l+r+1<=11,0<p<=11,i<p,
    ensures l+1<=11,l+1+r<=11,l+1+r+1<=12,
        i+1<=p,i+2<=p+1,(i+2)+(p-(i+1))<=12,
        (i+1)+(p-(i+1))<=12,p-1<=11,
{}
proof fn btree_steal_left(l:usize,r:usize)
    requires 0<l<=11,r<11,
    ensures l-1<11,l-1+1<=11,r+1<=11,r+2<=12,
{}
proof fn btree_steal_right(l:usize,r:usize)
    requires l<11,0<r<=11,
    ensures l+1<=11,r-1<11,1+(r-1)<=11,1+r<=12,
{}
proof fn btree_suffix(n:usize,i:usize)
    requires n<=11,i<=n,
    ensures n-i<=11,i+1+(n-i)<=12,1+(n-i)<=12,
{}
proof fn btree_repair(n:usize,l:usize,r:usize,left:bool)
    requires 0<n<=11,l<=11,r<=11,if left{r>=5}else{l>=5},
    ensures if l+r+1<=11 {6<=l+r+1<=11 && n-1<=11}
        else {if left {l<6 ==> r>5} else {r<6 ==> l>5}},
{}
proof fn btree_delete_underflow(l:usize,r:usize,p:usize)
    requires l<=5,r<=5,l+r+1<=11,0<p<=11,
    ensures l+r+1<=11,p-1<=11,
{}
proof fn btree_cursor(i:usize,n:usize,height:usize)
    requires i<n,n<=11,height<usize::BITS,
    ensures i+1<=n,i+i+1<=23,height+1<=usize::BITS,
{}
proof fn btree_checked_length(old:usize,new:usize)
    requires old<usize::MAX,new==old+1,
    ensures new>old,new<=usize::MAX,new-1==old,
{}
proof fn btree_remove_length(old:usize)
    requires old>0,
    ensures old-1<old,
{}
proof fn btree_set_replace_slot(count:usize,index:usize,total:usize)
    requires 0<count<=11,index<count,count<=total,
    ensures index<11,index+1<=count,total==total,
{}
proof fn btree_set_merge_progress(a:nat,b:nat,consume_a:bool,consume_b:bool)
    requires consume_a ==> a>0,consume_b ==> b>0,consume_a||consume_b,
    ensures a-(if consume_a{1nat}else{0nat})+b-(if consume_b{1nat}else{0nat})<a+b,
{}
proof fn btree_tracked_merge(l:usize,r:usize,i:usize)
    requires l+r+1<=11,i<r,
    ensures l+1+i<l+1+r,l+1+i<11,
{}
proof fn btree_tracked_steal(l:usize,r:usize,i:usize)
    requires l<=11,r<11,i<r,
    ensures i+1<r+1,i+1<11,
{}
proof fn btree_clone_prefix(done:usize,count:usize)
    requires done<count,count<=11,
    ensures done<11,done+1<=count,done+1<=11,
{}
proof fn btree_children_prefix(done:usize,count:usize)
    requires done<=count,count<=11,
    ensures done<12,done+1<=12,
{}
proof fn btree_cleanup_prefix(pairs:usize,children:usize,height:usize)
    requires pairs<=11,children<=12,children>0 ==> height>0,
    ensures pairs>0 ==> pairs-1<11,children>0 ==> children-1<12,
        height>0 ==> height-1<height,
{}
proof fn btree_layout_slot(index:usize,size:usize,array_bytes:usize)
    requires index<=11,array_bytes as int==11*(size as int),array_bytes<=isize::MAX,
    ensures (index as int)*(size as int)<=array_bytes,(index as int)*(size as int)<=isize::MAX,
{ assert((index as int)*(size as int)<=array_bytes) by(nonlinear_arith)
    requires 0<=index as int<=11,0<=size as int,array_bytes as int==11*(size as int); }
proof fn btree_layout_edge(index:usize,size:usize,array_bytes:usize)
    requires index<12,array_bytes as int==12*(size as int),array_bytes<=isize::MAX,
    ensures (index as int)*(size as int)+(size as int)<=array_bytes,
{ assert((index as int)*(size as int)+(size as int)<=array_bytes) by(nonlinear_arith)
    requires 0<=(index as int)<12,0<=size as int,array_bytes as int==12*(size as int); }
// Credit conservation for the recycling append. These prefix inequalities
// describe complete retired nodes and at most one partial frontier per input.
proof fn btree_recycle_leaf_credit(consumed:int,retired_leaves:int,output:int,allocated:int)
    requires 0<=retired_leaves,0<=output<=consumed,
        consumed<=12*retired_leaves+20,allocated<=output/12+1,
    ensures allocated<=retired_leaves+2,
{}
proof fn btree_recycle_branch_credit(retired_leaves:int,retired_branches:int,
    input_heights:int,output_leaves:int,output_branches:int,output_height:int)
    requires 0<=retired_leaves,0<=retired_branches,0<=input_heights,0<=output_height,
        retired_leaves<=11*retired_branches+11*input_heights+2,
        output_leaves<=retired_leaves+2,
        11*output_branches<=output_leaves+11*output_height,
    ensures output_branches<=retired_branches+input_heights+output_height+2,
{}
proof fn btree_external_depth(depth:usize,height:usize)
    requires depth<height,height<usize::BITS,
    ensures depth+1<=height,depth+1<usize::BITS,height-depth>0,
{}
proof fn btree_external_index(index:usize,length:usize)
    requires index<length,
    ensures index+1<=length,index+1<=usize::MAX,
{}
// Prefix invariants are established by traversal events, rather than assumed
// to follow merely from a key count. All counters here are ghost integers.
spec fn prefix_valid(leaf_items:int,internal_items:int,leaves:int,partial:int)->bool {
    0<=leaf_items && 0<=internal_items<=leaves && 0<=leaves &&
    0<=partial<=10 && leaf_items<=11*leaves+partial
}
proof fn btree_prefix_initial()
    ensures prefix_valid(0,0,0,0),
{}
proof fn btree_prefix_leaf_step(e:int,a:int,l:int,p:int,n:int)
    requires prefix_valid(e,a,l,p),p+1<n,0<n<=11,
    ensures prefix_valid(e+1,a,l,p+1),
{}
proof fn btree_prefix_retire_leaf(e:int,a:int,l:int,p:int,n:int)
    requires prefix_valid(e,a,l,p),p+1==n,0<n<=11,
    ensures prefix_valid(e+1,a,l+1,0),
{}
proof fn btree_prefix_internal_step(e:int,a:int,l:int,p:int)
    requires prefix_valid(e,a,l,p),a<l,p==0,
    ensures prefix_valid(e,a+1,l,0),
{}
proof fn btree_prefix_total(e:int,a:int,l:int,p:int)
    requires prefix_valid(e,a,l,p),
    ensures e+a<=12*l+10,
{}
proof fn btree_two_prefixes(e1:int,a1:int,l1:int,p1:int,e2:int,a2:int,l2:int,p2:int)
    requires prefix_valid(e1,a1,l1,p1),prefix_valid(e2,a2,l2,p2),
    ensures e1+a1+e2+a2<=12*(l1+l2)+20,
{}
// A retired-node forest attaches to the live ancestor frontier. Retiring a
// branch replaces its <=12 completed-child credits with one parent credit.
spec fn forest_valid(leaves:int,branches:int,frontier:int)->bool {
    0<=leaves && 0<=branches && 0<=frontier && leaves+branches<=12*branches+frontier
}
proof fn btree_forest_initial()
    ensures forest_valid(0,0,0),
{}
proof fn btree_forest_leaf(leaves:int,branches:int,frontier:int)
    requires forest_valid(leaves,branches,frontier),
    ensures forest_valid(leaves+1,branches,frontier+1),
{}
proof fn btree_forest_branch(leaves:int,branches:int,frontier:int,children:int)
    requires forest_valid(leaves,branches,frontier),1<=children<=12,children<=frontier,
    ensures forest_valid(leaves,branches+1,frontier-children+1),
{}
proof fn btree_frontier_frame(total:int,frames:int,completed:int)
    requires 0<=total<=11*frames,0<=frames,0<=completed<=11,
    ensures 0<=total+completed<=11*(frames+1),
{}
proof fn btree_retired_node_balance(leaves:int,branches:int,frontier:int,height:int)
    requires forest_valid(leaves,branches,frontier),0<=height,frontier<=11*height+1,
    ensures leaves<=11*branches+11*height+1,
{}
// Packed construction leaves only one possibly nonfull branch per level.
proof fn btree_packed_branch_balance(leaves:int,branches:int,height:int,keys:int)
    requires 0<=leaves,0<=branches,0<=height,keys==leaves-1,
        11*(branches-height)<=keys,
    ensures 11*branches<=leaves-1+11*height,
{}
proof fn btree_reservation_cap(allocated:int,retired:int,credit:int,total:int)
    requires 0<=allocated<=total,0<=retired,0<=credit,
        allocated<=retired+credit,
    ensures allocated<=retired+if credit<total{credit}else{total},
{}
proof fn btree_split_reservation(levels:usize,height:usize,new_root:bool)
    requires 0<levels<=height+1,height<usize::BITS,
        new_root ==> levels==height+1,
    ensures levels<=usize::BITS,new_root ==> levels-1==height,
{}
proof fn btree_plan_consume(remaining:usize,level:usize,stop:usize)
    requires remaining>0,level<stop,remaining==stop-level,stop<=usize::BITS,
    ensures remaining-1==stop-(level+1),level+1<=stop,
{}
proof fn btree_height_admission(old:usize)
    requires old<usize::BITS-1,
    ensures old+1<usize::BITS,old+1<65536,
{}
proof fn btree_packed_leaf_count(items:int,leaves:int)
    requires 0<items,leaves==items/12+1,
    ensures 12*(leaves-1)<=items<12*leaves,
{}

proof fn btree_initial_state()
    ensures 0usize<=11,0usize<usize::BITS,
{}
proof fn btree_closed_node_count(count:usize)
    requires count<=11,
    ensures count<65536,(count as u16) as usize==count,
{}
proof fn btree_cursor_state(index:usize,height:usize)
    requires index<=11,height<usize::BITS,
    ensures index<65536,height<65536,
        (index as u16) as usize==index,(height as u16) as usize==height,
{}
proof fn btree_fix_cursor(index:usize,last:usize)
    requires index<=last,last<=11,
    ensures index<12,index+1<=12,index+1<=usize::MAX,
{}
proof fn btree_delete_decision(child:usize,sibling:usize,parent:usize,index:usize,left:bool)
    requires child<5,sibling<=11,0<parent<=11,index<=parent,
        if left{index>0}else{index<parent},
    ensures child<11,
        sibling>5 ==> sibling>0,
        sibling<=5 ==> child+sibling+1<=11,
        if left {index-1<parent} else {index<parent},
{}
proof fn btree_count_join(total:usize,child:usize,limit:usize)
    requires total<=limit,child<=limit-total,
    ensures total+child<=limit,total+child<=usize::MAX,
{}
proof fn btree_insert_pair_conservation(left:int,right:int,median:int,pending:int)
    requires 0<=left,0<=right,median==1,pending==1,left+right+median==11,
    ensures left+right+median+pending==12,
{}
proof fn btree_merge_pair_conservation(left:int,right:int,parent:int)
    requires 0<=left,0<=right,parent>0,
    ensures left+right+parent==(left+right+1)+(parent-1),
{}
proof fn btree_borrow_pair_conservation(left:int,right:int,parent:int)
    requires left>0,right>=0,parent>0,
    ensures left+right+parent==(left-1)+(right+1)+parent,
{}
proof fn btree_split_pair_conservation(old:usize,at:usize)
    requires at<=old,
    ensures at+(old-at)==old,
{}
proof fn btree_merge_stream_conservation(output:int,a:int,b:int,pending:int,discarded:int,total:int)
    requires 0<=output,0<=a,0<=b,1<=pending<=2,0<=discarded,
        output+a+b+pending+discarded==total,total<=usize::MAX,
    ensures output+1<=total,
        (output+1)+a+b+(pending-1)+discarded==total,
{}

// Opaque ownership tokens are identified by their original initialized slot.
// A bijection of slot indices proves preservation for arbitrary token values;
// it does not depend on concrete heap addresses or on u8 payloads.
spec fn inserted_token(slots:int,position:int,index:int)->int {
    if index<position{index}else if index==position{slots}else{index-1}
}
proof fn btree_insert_token_bijection(slots:int,position:int,a:int,b:int,old:int)
    requires 0<slots<=12,0<=position<=slots,0<=a<=slots,0<=b<=slots,0<=old<slots,
    ensures 0<=inserted_token(slots,position,a)<=slots,
        a!=b ==> inserted_token(slots,position,a)!=inserted_token(slots,position,b),
        inserted_token(slots,position,position)==slots,
        inserted_token(slots,position,if old<position{old}else{old+1})==old,
{}
spec fn removed_token(position:int,index:int)->int {
    if index<position{index}else{index+1}
}
proof fn btree_remove_token_bijection(slots:int,position:int,a:int,b:int,old:int)
    requires 1<slots<=12,0<=position<slots,0<=a<slots-1,0<=b<slots-1,
        0<=old<slots,old!=position,
    ensures 0<=removed_token(position,a)<slots,removed_token(position,a)!=position,
        a!=b ==> removed_token(position,a)!=removed_token(position,b),
        removed_token(position,if old<position{old}else{old-1})==old,
{}
proof fn btree_partition_tokens(total:int,boundary:int,index:int)
    requires 0<=boundary<=total,0<=index<total,
    ensures if index<boundary {0<=index<boundary}
        else {0<=index-boundary<total-boundary && boundary+(index-boundary)==index},
{}
proof fn btree_split_child_partition(mid:usize,index:usize)
    requires mid<11,index<12,
    ensures mid+1<=11,12-(mid+1)<=12,
        if index<mid+1{index<=mid}
        else{index-(mid+1)<12-(mid+1) && mid+1+(index-(mid+1))==index},
{}
proof fn btree_merge_child_partition(left:usize,right:usize,index:usize)
    requires left+right+1<=11,index<left+right+2,
    ensures if index<left+1{index<=left}
        else{index-(left+1)<=right && left+1+(index-(left+1))==index},
{}
proof fn btree_borrow_child_partition(left:usize,right:usize,index:usize,from_left:bool)
    requires left<=11,right<=11,index<left+right+2,
        if from_left {left>0&&right<11}else{left<11&&right>0},
    ensures if from_left {
        if index<left {index<left}
        else {index-left<=right+1 && left+(index-left)==index}
    } else {
        if index<left+2 {index<=left+1}
        else {index-(left+2)<right && left+2+(index-(left+2))==index}
    },
{}

proof fn btree_delete_nonroot_count(count:usize)
    requires 5<=count<=11,
    ensures 4<=count-1<=10,count-1<5 ==> count-1==4,
{}
proof fn btree_delete_closed(child:usize,sibling:usize,parent:usize,borrow:bool)
    requires child==4,5<=sibling<=11,0<parent<=11,
        borrow <==> child+sibling+1>11,
    ensures if borrow {child+1==5 && 5<=sibling-1<=11}
        else{5<=child+sibling+1<=11 && parent-1<=11 &&
            (parent>=6 ==> parent-1>=5) && (parent==5 ==> parent-1==4)},
{}

// Borrowed iterators store leaf edges, including the one-past position. Whole
// iterators use an exact count; checked ranges meet at the same physical edge.
proof fn btree_leaf_edge(index:usize,count:usize,back:bool)
    requires index<=count,count<=11,if back{index>0}else{index<count},
    ensures if back{index-1<count && index-1<65536}
        else{index<count && index+1<=count && index+1<65536},
{}
proof fn btree_edge_parent(index:usize,count:usize,height:usize,back:bool)
    requires index<=count,count<=11,height<usize::BITS-1,
        if back{index>0}else{index<count},
    ensures height+1<usize::BITS,
        if back{index-1<count && index-1<12}
        else{index<count && index+1<=count && index+1<12},
{}
proof fn btree_exact_frontier(remaining:usize)
    requires remaining>0,
    ensures remaining-1<remaining,remaining==1 ==> remaining-1==0,
{}

proof fn btree_planned_allocation(reserved:int,retired:int,allocated:int)
    requires 0<=reserved,0<=retired,0<=allocated,allocated+1<=reserved+retired,
    ensures reserved+retired-allocated>=1,
{}
proof fn btree_plan_history(previous:int,next:int,old_alloc:int,old_retired:int)
    requires previous>=0,next>=previous,0<=old_alloc,0<=old_retired,
        old_alloc<=previous+old_retired,
    ensures old_alloc<=next+old_retired,
{}
proof fn btree_spare_amortization(extra:int,lower:int,input:int,std_peak:int)
    requires 0<=extra,10*extra<=lower,lower<=input,input<=std_peak,
    ensures 10*(input+extra)<=11*std_peak,
{}
proof fn btree_nonempty_node_bound(child_nodes:int,child_pairs:int,root_pairs:int)
    requires 0<=child_nodes<=child_pairs,root_pairs>=1,
    ensures child_nodes+1<=child_pairs+root_pairs,
{}


}

fn main() {}
