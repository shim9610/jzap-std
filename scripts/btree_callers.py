"""Whole-body text of the B-tree product functions followed by maintained callers.

Each maintained caller in tools/direct-verus/btree-adapters.rs was written
against exactly this text (comments and whitespace removed). A changed body
needs the caller to be read again; the comparison is not a solver proof.
Keys are (file, unique anchor or '', function head after the anchor).
"""
BODIES={
 ('node.rs', '', 'unsafe fn set_len<'):
  'unsafe{(*node.as_ptr()).len=Count(nasu16)};',
 ('node.rs', '', 'unsafe fn key<'):
  'unsafe{addr_of_mut!((*node.as_ptr()).keys).cast::<K>().add(i)}',
 ('node.rs', '', 'unsafe fn val<'):
  'unsafe{addr_of_mut!((*node.as_ptr()).vals).cast::<V>().add(i)}',
 ('node.rs', '', 'unsafe fn edge<'):
  'unsafe{addr_of_mut!((*node.cast::<Branch<K,V>>().as_ptr()).edges).cast::<Link<K,V>>().add(i)}',
 ('node.rs', '', 'unsafe fn child<'):
  'unsafe{edge(node,i).read()}',
 ('node.rs', '', 'unsafe fn set_child<'):
  'unsafe{edge(node,i).write(child);(*child.as_ptr()).parent=Some(node);(*child.as_ptr()).parent_idx=iasu16;}',
 ('node.rs', '', 'unsafe fn pairs<'):
  'unsafe{ptr::copy(key(src,from),key(dst,to),n);ptr::copy(val(src,from),val(dst,to),n);}',
 ('node.rs', '', 'unsafe fn edges<'):
  'unsafe{ptr::copy(edge(src,from),edge(dst,to),n)};',
 ('node.rs', '', 'unsafe fn replace_key<'):
  'unsafe{ptr::replace(key(node,i),replacement)}',
 ('node.rs', '', 'unsafe fn drop_pair<'):
  'structValueDrop<V>(*mutV);impl<V>DropforValueDrop<V>{fndrop(&mutself){unsafe{ptr::drop_in_place(self.0)};}}unsafe{letvalue=ValueDrop(val(node,i));ptr::drop_in_place(key(node,i));drop(value);}',
 ('view.rs', "impl<'a, K, V> NodeView<'a, K, V> {", 'pub fn keys('):
  'unsafe{core::slice::from_raw_parts(node::key(self.node,0),node::len(self.node))}',
 ('view.rs', "impl<'a, K, V> NodeView<'a, K, V> {", 'pub fn values('):
  'unsafe{core::slice::from_raw_parts(node::val(self.node,0),node::len(self.node))}',
 ('btree_map.rs', '', 'fn try_replace_key('):
  'matchself.search(&key){Search::Found(c)=>{letold=unsafe{node::replace_key(c.node,c.index(),key)};drop(value);Ok(Some(old))}Search::Vacant(at)=>{self.try_insert_at(at,key,value)?;Ok(None)}}',
 ('node.rs', '', 'fn fix<'):
  'letmuti=from;whilei<=to{unsafe{set_child(node,i,child(node,i))};i=add(i,1);}',
 ('node.rs', '', 'fn insert<'):
  'unsafe{letn=len(node);let(after,tail,_,_)=super::geometry::insert_span(n,i);pairs(node,i,node,after,tail);key(node,i).write(k);val(node,i).write(v);ifletSome(right)=right{edges(node,after,node,super::geometry::insert_span(n,i).3,tail,);edge(node,after).write(right);fix(node,after,super::geometry::insert_span(n,i).2);}set_len(node,super::geometry::insert_span(n,i).2);}',
 ('node.rs', '', 'fn splitpoint('):
  'ifedge<5{4}elseifedge<=6{5}else{6}',
 ('node.rs', '', 'fn split<'):
  'unsafe{let(start,n,edge_count)=super::geometry::split_span(len(node),mid);pairs(node,start,right,0,n);ifheight>0{edges(node,start,right,0,edge_count);fix(right,0,n);}set_len(right,n);set_len(node,mid);(key(node,mid).read(),val(node,mid).read())}',
 ('node.rs', '', 'fn move_suffix<'):
  'unsafe{let(n,edge_start)=super::geometry::suffix_span(len(left),index);pairs(left,index,right,0,n);ifheight>0{edges(left,edge_start,right,1,n);fix(right,1,n);}set_len(left,index);set_len(right,n);}',
 ('node.rs', '', 'fn remove_leaf<'):
  'unsafe{letn=len(node);letresult=(key(node,i).read(),val(node,i).read());let(after,tail,_)=super::geometry::remove_span(n,i);pairs(node,after,node,i,tail);set_len(node,super::geometry::remove_span(n,i).2);result}',
 ('node.rs', '', 'fn merge<'):
  'unsafe{letleft=child(parent,i);letright=child(parent,add(i,1));letl=len(left);letr=len(right);letstart=super::geometry::merge_span(l,r).0;key(left,l).write(key(parent,i).read());val(left,l).write(val(parent,i).read());pairs(right,0,left,start,r);ifheight>0{edges(right,0,left,start,super::geometry::merge_span(l,r).2);fix(left,start,super::geometry::merge_span(l,r).1);}set_len(left,super::geometry::merge_span(l,r).1);letn=len(parent);let(after,tail,_)=super::geometry::remove_span(n,i);pairs(parent,after,parent,i,tail);edges(parent,add(after,1),parent,after,tail);set_len(parent,super::geometry::remove_span(n,i).2);fix(parent,after,super::geometry::remove_span(n,i).2);free(right,height,policy);left}',
 ('node.rs', '', 'fn steal_left<'):
  'unsafe{letleft=child(parent,i);letright=child(parent,add(i,1));letleft_count=len(left);letr=len(right);letl=super::geometry::steal_left_span(left_count,r).0;pairs(right,0,right,1,r);key(right,0).write(key(parent,i).read());val(right,0).write(val(parent,i).read());key(parent,i).write(key(left,l).read());val(parent,i).write(val(left,l).read());ifheight>0{edges(right,0,right,1,super::geometry::steal_left_span(left_count,r).1,);edge(right,0).write(child(left,add(l,1)));fix(right,0,super::geometry::steal_left_span(left_count,r).1);}set_len(left,l);set_len(right,super::geometry::steal_left_span(left_count,r).1);}',
 ('node.rs', '', 'fn steal_right<'):
  'unsafe{letleft=child(parent,i);letright=child(parent,add(i,1));letl=len(left);letr=len(right);key(left,l).write(key(parent,i).read());val(left,l).write(val(parent,i).read());key(parent,i).write(key(right,0).read());val(parent,i).write(val(right,0).read());pairs(right,1,right,0,super::geometry::steal_right_span(l,r).1,);ifheight>0{set_child(left,super::geometry::steal_right_span(l,r).0,child(right,0),);edges(right,1,right,0,r);fix(right,0,super::geometry::steal_right_span(l,r).1);}set_len(left,super::geometry::steal_right_span(l,r).0);set_len(right,super::geometry::steal_right_span(l,r).1);}',
 ('node.rs', '', 'fn drain('):
  'whileself.children>0{self.children=sub(self.children,1);unsafe{destroy(child(self.node,self.children),sub(self.height,1),self.policy,)};}whileself.pairs>0{self.pairs=sub(self.pairs,1);unsafe{drop_pair(self.node,self.pairs)};}',
 ('node.rs', '', 'fn destroy<'):
  'letn=unsafe{len(node)};drop(NodeGuard{node,height,pairs:n,children:ifheight==0{0}else{add(n,1)},policy,});',
 ('node.rs', '', 'fn drop_pair<'):
  'structValueDrop<V>(*mutV);impl<V>DropforValueDrop<V>{fndrop(&mutself){unsafe{ptr::drop_in_place(self.0)};}}unsafe{letvalue=ValueDrop(val(node,i));ptr::drop_in_place(key(node,i));drop(value);}',
 ('btree_map.rs', '', 'fn first_cursor('):
  'letroot=self.root?;unsafe{Cursor::first(root,self.height.get())}',
 ('btree_map.rs', '', 'fn last_cursor('):
  'letroot=self.root?;unsafe{Cursor::last(root,self.height.get())}',
 ('btree_map.rs', '', 'pub fn first_key_value('):
  'letc=self.first_cursor()?;Some(unsafe{(&*node::key(c.node,c.index()),&*node::val(c.node,c.index()),)})',
 ('btree_map.rs', '', 'pub fn last_key_value('):
  'letc=self.last_cursor()?;Some(unsafe{(&*node::key(c.node,c.index()),&*node::val(c.node,c.index()),)})',
 ('btree_map.rs', '', 'fn search<'):
  'letSome(mutn)=self.rootelse{returnSearch::Vacant(None);};letmuth=self.height.get();loop{unsafe{letmuti=0;letcount=node::len(n);whilei<count{matchkey.cmp((&*node::key(n,i)).borrow()){Ordering::Equal=>returnSearch::Found(Cursor::new(n,i,h)),Ordering::Less=>break,Ordering::Greater=>i=add(i,1),}}ifh==0{returnSearch::Vacant(Some(Cursor::new(n,i,0)));}n=node::child(n,i);h=sub(h,1);}}',
 ('btree_map.rs', '', 'pub fn get<'):
  'matchself.search(key){Search::Found(c)=>{Some(unsafe{&*node::val(c.node,c.index())})}_=>None,}',
 ('btree_map.rs', '', 'pub fn get_key_value<'):
  'matchself.search(key){Search::Found(c)=>{Some(unsafe{(&*node::key(c.node,c.index()),&*node::val(c.node,c.index()),)})}_=>None,}',
 ('btree_map.rs', '', 'pub fn get_mut<'):
  'matchself.search(key){Search::Found(c)=>{Some(unsafe{&mut*node::val(c.node,c.index())})}_=>None,}',
 ('btree_map.rs', '', 'fn try_insert_at('):
  'ifletSome(at)=at{ifunsafe{node::len(at.node)}<CAP{letnew_len=matchself.len+Checked::new(1){Ok(n)=>n,Err(_)=>{returnErr(Rejected{value:(key,value),error:Error::Capacity,});}};returnOk(unsafe{self.commit_fit(at,new_len,key,value)});}}letplan=matchself.plan_insert(at){Ok(plan)=>plan,Err(error)=>{returnErr(Rejected{value:(key,value),error,});}};Ok(self.commit_insert(plan,key,value))',
 ('btree_map.rs', '', 'unsafe fn commit_fit('):
  'unsafe{node::insert(at.node,at.index(),key,value,None)};self.len=new_len;at',
 ('btree_map.rs', '', 'fn plan_insert('):
  'letnew_len=(self.len+Checked::new(1)).map_err(|_|Error::Capacity)?;letspares=Spares::new(self.policy.try_clone()?);letc=matchat{Some(c)=>c,None=>{letroot=node::allocate(0,&self.policy)?;letc=Cursor::new(root,0,0);returnOk(InsertPlan{at:c,stop:c,spares,root:Some(root),new_len,new_height:self.height,});}};letmutplan=InsertPlan{at:c,stop:c,spares,root:None,new_len,new_height:self.height,};unsafe{ifnode::len(c.node)<CAP{returnOk(plan);}letmuttop=c.node;letmuth=0;loop{matchnode::parent(top){Some((p,_))ifnode::len(p)==CAP=>{top=p;h=add(h,1);}_=>break,}}ifnode::parent(top).is_none(){ifself.height.get()>=sub(usize::BITSasusize,1){returnErr(Error::Capacity);}plan.new_height=(self.height+Checked::new(1)).map_err(|_|Error::Capacity)?;plan.root=Some(node::allocate(plan.new_height.get(),&self.policy)?);ifletSome(root)=plan.root{plan.stop=Cursor::new(root,0,plan.new_height.get());}}elseifletSome((parent,index))=node::parent(top){plan.stop=Cursor::new(parent,index,add(h,1));}loop{plan.spares.reserve(h)?;ifh==0{break;}h=sub(h,1);}}Ok(plan)',
 ('btree_map.rs', '', 'fn commit_insert('):
  'letat=plan.at;ifself.root.is_none(){unsafe{node::insert(at.node,0,k,v,None)};self.root=plan.root.take();self.len=plan.new_len;returnat;}letmutcur=at;letmutright_child=None;letmutinserted=at;unsafe{whileletSome(right)=plan.spares.take(){letmid=node::splitpoint(cur.index());let(upk,upv)=node::split(cur.node,mid,right,cur.height());let(target,index)=ifcur.index()<=mid{(cur.node,cur.index())}else{(right,sub(cur.index(),add(mid,1)))};node::insert(target,index,k,v,right_child);ifcur.height()==0{inserted=Cursor::new(target,index,0);}k=upk;v=upv;right_child=Some(right);matchnode::parent(cur.node){Some((parent,index))=>cur=Cursor::new(parent,index,add(cur.height(),1)),None=>{node::set_child(plan.stop.node,0,cur.node);cur=plan.stop;}}}node::insert(cur.node,cur.index(),k,v,right_child);ifletSome(root)=plan.root.take(){self.root=Some(root);self.height=plan.new_height;}}self.len=plan.new_len;inserted',
 ('btree_map.rs', '', 'unsafe fn remove_at('):
  'unsafe{ifc.height()>0{letmutpred=Cursor::last(node::child(c.node,c.index()),sub(c.height(),1));ifletSome(p)=pred.take(){ptr::swap(node::key(c.node,c.index()),node::key(p.node,p.index()));ptr::swap(node::val(c.node,c.index()),node::val(p.node,p.index()));fortintracked.iter_mut().flatten(){if*t==p{*t=c;}}c=p;}}letresult=node::remove_leaf(c.node,c.index());fortintracked.iter_mut().flatten(){ift.node==c.node&&t.index()>c.index(){t.set_index(sub(t.index(),1));}}self.len=Checked::new(sub(self.len(),1));letmutn=c.node;letmuth=0;whilenode::len(n)<MIN{letSome((p,i))=node::parent(n)else{break};letpn=node::len(p);ifgeometry::consider_left(i)&&geometry::borrow_required(node::len(node::child(p,sub(i,1))),node::len(n)){letsep=sub(i,1);letleft=node::child(p,sep);letlast=sub(node::len(left),1);fortintracked.iter_mut().flatten(){ift.node==n{t.set_index(add(t.index(),1));}elseift.node==p&&t.index()==sep{*t=Cursor::new(n,0,h);}elseift.node==left&&t.index()==last{*t=Cursor::new(p,sep,add(h,1));}}node::steal_left(p,sep,h);break;}ifgeometry::consider_right(i,pn)&&geometry::borrow_required(node::len(node::child(p,add(i,1))),node::len(n)){letright=node::child(p,add(i,1));letl=node::len(n);fortintracked.iter_mut().flatten(){ift.node==p&&t.index()==i{*t=Cursor::new(n,l,h);}elseift.node==right{ift.index()==0{*t=Cursor::new(p,i,add(h,1));}else{t.set_index(sub(t.index(),1));}}}node::steal_right(p,i,h);break;}letsep=ifi>0{sub(i,1)}else{0};letleft=node::child(p,sep);letright=node::child(p,add(sep,1));letl=node::len(left);fortintracked.iter_mut().flatten(){ift.node==right{*t=Cursor::new(left,add(add(l,1),t.index()),h);}elseift.node==p{ift.index()==sep{*t=Cursor::new(left,l,h);}elseift.index()>sep{t.set_index(sub(t.index(),1));}}}node::merge(p,sep,h,&self.policy);n=p;h=add(h,1);}self.shrink_root();result}',
 ('btree_map.rs', '', 'unsafe fn shrink_root('):
  'unsafe{whileself.height.get()>0{letSome(root)=self.rootelse{break};ifnode::len(root)>0{break;}letchild=node::child(root,0);(*child.as_ptr()).parent=None;self.root=Some(child);node::free(root,self.height.get(),&self.policy);self.height=Checked::new(sub(self.height.get(),1));}}',
 ('btree_map.rs', '', 'pub fn try_insert('):
  'matchself.search(&key){Search::Found(c)=>{letold=unsafe{letslot=node::val(c.node,c.index());letold=slot.read();slot.write(value);old};drop(key);Ok(Some(old))}Search::Vacant(at)=>{self.try_insert_at(at,key,value)?;Ok(None)}}',
 ('entry.rs', "OccupiedEntry<'a, K, V, P> {", 'pub fn key('):
  'unsafe{&*node::key(self.at.node,self.at.index())}',
 ('entry.rs', "OccupiedEntry<'a, K, V, P> {", 'pub fn get('):
  'unsafe{&*node::val(self.at.node,self.at.index())}',
 ('entry.rs', "OccupiedEntry<'a, K, V, P> {", 'pub fn get_mut('):
  'unsafe{&mut*node::val(self.at.node,self.at.index())}',
 ('entry.rs', "OccupiedEntry<'a, K, V, P> {", 'pub fn into_mut('):
  'unsafe{&mut*node::val(self.at.node,self.at.index())}',
 ('entry.rs', "OccupiedEntry<'a, K, V, P> {", 'pub fn remove_entry('):
  'unsafe{self.map.remove_at(self.at,&mut[])}',
 ('entry.rs', "VacantEntry<'a, K, V, P> {", 'pub fn try_insert_with<'):
  'ifletSome(at)=self.at{ifunsafe{node::len(at.node)}<super::CAP{letnew_len=matchself.map.len+crate::Checked::new(1){Ok(n)=>n,Err(_)=>{returnErr(Rejected{value:self.key,error:InitError::Storage(Error::Capacity),});}};letvalue=matchf(&self.key){Ok(v)=>v,Err(e)=>{returnErr(Rejected{value:self.key,error:InitError::Initialize(e),});}};letat=unsafe{self.map.commit_fit(at,new_len,self.key,value)};returnOk(OccupiedEntry{map:self.map,at}.into_mut());}}letplan=matchself.map.plan_insert(self.at){Ok(p)=>p,Err(e)=>{returnErr(Rejected{value:self.key,error:InitError::Storage(e),});}};letvalue=matchf(&self.key){Ok(v)=>v,Err(e)=>{returnErr(Rejected{value:self.key,error:InitError::Initialize(e),});}};letat=self.map.commit_insert(plan,self.key,value);Ok(unsafe{&mut*node::val(at.node,at.index())})',
 ('iter.rs', 'impl<K, V> Cursor<K, V> {', 'unsafe fn first('):
  'unsafe{whileheight>0{node=node::child(node,0);height=sub(height,1);}ifnode::len(node)==0{None}else{Some(Self::new(node,0,0))}}',
 ('iter.rs', 'impl<K, V> Cursor<K, V> {', 'unsafe fn last('):
  'unsafe{whileheight>0{node=node::child(node,node::len(node));height=sub(height,1);}letn=node::len(node);ifn==0{None}else{Some(Self::new(node,sub(n,1),0))}}',
 ('iter.rs', 'impl<K, V> Cursor<K, V> {', 'unsafe fn next('):
  'unsafe{ifself.height()>0{returnSelf::first(node::child(self.node,add(self.index(),1)),sub(self.height(),1),);}ifadd(self.index(),1)<node::len(self.node){returnSome(Self::new(self.node,add(self.index(),1),0));}letmutn=self.node;letmuth=0;whileletSome((p,i))=node::parent(n){h=add(h,1);ifi<node::len(p){returnSome(Self::new(p,i,h));}n=p;}None}',
 ('iter.rs', 'impl<K, V> Cursor<K, V> {', 'unsafe fn prev('):
  'unsafe{ifself.height()>0{returnSelf::last(node::child(self.node,self.index()),sub(self.height(),1));}ifself.index()>0{returnSome(Self::new(self.node,sub(self.index(),1),0));}letmutn=self.node;letmuth=0;whileletSome((p,i))=node::parent(n){h=add(h,1);ifi>0{returnSome(Self::new(p,sub(i,1),h));}n=p;}None}',
 ('iter.rs', 'pub(super) struct State<K, V> {', 'unsafe fn take('):
  'ifself.remaining.get()==0{returnNone;}letc=ifback{self.back?}else{self.front?};ifself.front==self.back{self.remaining=Checked::new(0);self.front=None;self.back=None;}else{self.remaining=Checked::new(sub(self.remaining.get(),1));unsafe{ifback{self.back=c.prev();}else{self.front=c.next();}}}Some(c)',
 ('iter.rs', 'pub(super) struct State<K, V> {', 'unsafe fn advance_owned('):
  'unsafe{self.remaining=Checked::new(sub(self.remaining.get(),1));ifself.remaining.get()==0{letmutn=c.node;letmuth=c.height();loop{letparent=node::parent(n);retire(n,h);matchparent{Some((p,_))=>{n=p;h=add(h,1);}None=>break,}}self.front=None;self.back=None;return;}letnext=ifc.height()>0{ifback{Cursor::last(node::child(c.node,c.index()),sub(c.height(),1))}else{Cursor::first(node::child(c.node,add(c.index(),1)),sub(c.height(),1))}}elseifback&&c.index()>0{Some(Cursor::new(c.node,sub(c.index(),1),0))}elseif!back&&add(c.index(),1)<node::len(c.node){Some(Cursor::new(c.node,add(c.index(),1),0))}else{letmutn=c.node;letmuth=0;loop{letparent=node::parent(n);retire(n,h);letSome((p,i))=parentelse{breakNone};h=add(h,1);ifback&&i>0{breakSome(Cursor::new(p,sub(i,1),h));}if!back&&i<node::len(p){breakSome(Cursor::new(p,i,h));}n=p;}};ifback{self.back=next;}else{self.front=next;}}',
 ('iter.rs', 'impl<K, V> BorrowState<K, V> {', 'unsafe fn edge('):
  'unsafe{whileheight>0{node=node::child(node,ifback{node::len(node)}else{0});height=sub(height,1);}BorrowEdge::new(node,ifback{node::len(node)}else{0},0)}',
 ('iter.rs', 'impl<K, V> BorrowState<K, V> {', 'fn new<'):
  'letSome(root)=map.rootelse{returnSelf::empty();};unsafe{Self{front:Self::edge(root,map.height.get(),false),back:Self::edge(root,map.height.get(),true),remaining:Checked::new(map.len()),}}',
 ('iter.rs', 'impl<K, V> BorrowState<K, V> {', 'fn remaining('):
  'Self{front:state.front.map(BorrowEdge::from_cursor).unwrap_or_else(||BorrowEdge::new(Link::dangling(),0,0)),back:state.back.map(|mutc|{ifc.height()==0{c.set_index(add(c.index(),1));}BorrowEdge::from_cursor(c)}).unwrap_or_else(||BorrowEdge::new(Link::dangling(),0,0)),remaining:state.remaining,}',
 ('iter.rs', 'impl<K, V> BorrowState<K, V> {', 'unsafe fn interval('):
  'letmutresult=Self::remaining(state);unsafe{ifresult.front.height()>0{letc=result.front;result.front=Self::edge(node::child(c.node,c.index()),sub(c.height(),1),true);}ifresult.back.height()>0{letc=result.back;result.back=Self::edge(node::child(c.node,add(c.index(),1)),sub(c.height(),1),false,);}}result',
 ('iter.rs', 'impl<K, V> BorrowState<K, V> {', 'unsafe fn take<const EXACT: bool>'):
  'letremaining=self.remaining.get();ifremaining==0{returnNone;}if!EXACT&&self.front==self.back{self.remaining=Checked::new(0);returnNone;}letmutc=ifback{self.back}else{self.front};letlast=EXACT&&remaining==1;unsafe{letnext;ifc.height()>0{next=iflast{c}else{Self::edge(node::child(c.node,ifback{c.index()}else{add(c.index(),1)}),sub(c.height(),1),back,)};}elseif(back&&c.index()>0)||(!back&&c.index()<node::len(c.node)){let(i,edge)=super::geometry::leaf_edge(c.index(),node::len(c.node),back);next=BorrowEdge::new(c.node,edge,0);c.set_index(i);}else{letmutn=c.node;letmuth=0;loop{let(parent,i)=node::parent(n)?;h=add(h,1);if(back&&i>0)||(!back&&i<node::len(parent)){letat=ifback{sub(i,1)}else{i};c=BorrowEdge::new(parent,at,h);next=iflast{c}else{Self::edge(node::child(parent,ifback{at}else{add(at,1)}),sub(h,1),back,)};break;}n=parent;}}ifEXACT{self.remaining=Checked::new(sub(remaining,1));}ifback{self.back=next;}else{self.front=next;}}Some(c)',
 ('iter.rs', "impl<'a, K, V> Iterator for Iter<'a, K, V> {", 'fn next('):
  'unsafe{letc=self.state.take::<true>(false)?;Some((&*node::key(c.node,c.index()),&*node::val(c.node,c.index()),))}',
 ('iter.rs', "impl<K, V> DoubleEndedIterator for Iter<'_, K, V> {", 'fn next_back('):
  'unsafe{letc=self.state.take::<true>(true)?;Some((&*node::key(c.node,c.index()),&*node::val(c.node,c.index()),))}',
 ('iter.rs', "impl<'a, K, V> Iterator for IterMut<'a, K, V> {", 'fn next('):
  'unsafe{letc=self.state.take::<true>(false)?;Some((&*node::key(c.node,c.index()),&mut*node::val(c.node,c.index()),))}',
 ('iter.rs', "impl<K, V> DoubleEndedIterator for IterMut<'_, K, V> {", 'fn next_back('):
  'unsafe{letc=self.state.take::<true>(true)?;Some((&*node::key(c.node,c.index()),&mut*node::val(c.node,c.index()),))}',
 ('iter.rs', "impl<'a, K, V> Iterator for Range<'a, K, V> {", 'fn next('):
  'unsafe{letc=self.0.state.take::<false>(false)?;Some((&*node::key(c.node,c.index()),&*node::val(c.node,c.index()),))}',
 ('iter.rs', "impl<K, V> DoubleEndedIterator for Range<'_, K, V> {", 'fn next_back('):
  'unsafe{letc=self.0.state.take::<false>(true)?;Some((&*node::key(c.node,c.index()),&*node::val(c.node,c.index()),))}',
 ('iter.rs', "impl<'a, K, V> Iterator for RangeMut<'a, K, V> {", 'fn next('):
  'unsafe{letc=self.0.state.take::<false>(false)?;Some((&*node::key(c.node,c.index()),&mut*node::val(c.node,c.index()),))}',
 ('iter.rs', "impl<K, V> DoubleEndedIterator for RangeMut<'_, K, V> {", 'fn next_back('):
  'unsafe{letc=self.0.state.take::<false>(true)?;Some((&*node::key(c.node,c.index()),&mut*node::val(c.node,c.index()),))}',
 ('iter.rs', 'impl<K, V, P: AllocationPolicy> IntoIter<K, V, P> {', 'fn take_state('):
  'ifstate.remaining.get()==0{returnNone;}letc=ifback{state.back?}else{state.front?};unsafe{letpair=(node::key(c.node,c.index()).read(),node::val(c.node,c.index()).read(),);state.advance_owned(c,back,retire);Some(pair)}',
 ('operations.rs', '', 'fn ordered<'):
  'let(mutan,mutbn)=(a.node,b.node);let(mutah,mutbh)=(a.height(),b.height());let(mutai,mutbi)=(add(a.index(),add(a.index(),1)),add(b.index(),add(b.index(),1)),);unsafe{whileah<bh{ifletSome((p,i))=node::parent(an){an=p;ai=add(i,i);}ah=add(ah,1);}whilebh<ah{ifletSome((p,i))=node::parent(bn){bn=p;bi=add(i,i);}bh=add(bh,1);}whilean!=bn{match(node::parent(an),node::parent(bn)){(Some((ap,ax)),Some((bp,bx)))=>{an=ap;bn=bp;ai=add(ax,ax);bi=add(bx,bx);}_=>returnfalse,}}}ai<=bi',
 ('operations.rs', '', 'unsafe fn fix_border('):
  'unsafe{self.shrink_root();letSome(mutn)=self.rootelse{return};letmuth=self.height.get();whileh>0{letcount=node::len(n);ifcount==0{break;}letsep=ifleft{0}else{sub(count,1)};letch=sub(h,1);leta=node::child(n,sep);letb=node::child(n,add(sep,1));ifadd(add(node::len(a),1),node::len(b))<=CAP{n=node::merge(n,sep,ch,&self.policy);}else{letparent=n;n=ifleft{a}else{b};whilenode::len(n)<6{ifleft{node::steal_right(parent,sep,ch);}else{node::steal_left(parent,sep,ch);}}}h=ch;}self.shrink_root();}',
 ('operations.rs', '', 'unsafe fn count_subtree('):
  'unsafe{letcount=node::len(n);letmuttotal=count;ifheight>0{foriin0..=count{total=add(total,Self::count_subtree(node::child(n,i),sub(height,1)),);}}total}',
 ('operations.rs', '', 'pub fn try_split_off<'):
  'letSome(root)=self.rootelse{returnOk(Self::new_in(self.policy.try_clone()?));};letmutpath=[0u16;usize::BITSasusize];letmutlevel=0;letmutn=root;letmuth=self.height.get();unsafe{loop{letmuti=0;whilei<node::len(n)&&(&*node::key(n,i)).borrow().cmp(key)==Ordering::Less{i=add(i,1);}*path.get_mut(level).ok_or(Error::Capacity)?=iasu16;ifh==0{break;}n=node::child(n,i);h=sub(h,1);level=add(level,1);}}letmutright=Self::new_in(self.policy.try_clone()?);right.root=Some(node::allocate(0,&self.policy)?);whileright.height<self.height{letparent=node::allocate(add(right.height.get(),1),&self.policy)?;ifletSome(child)=right.root{unsafe{node::set_child(parent,0,child)};}right.root=Some(parent);right.height=Checked::new(add(right.height.get(),1));}letmuta=self.root;letmutb=right.root;letmutheight=self.height.get();unsafe{forindexinpath.iter().take(add(height,1)){iflet(Some(left),Some(dest))=(a,b){leti=usize::from(*index);node::move_suffix(left,i,dest,height);ifheight==0{break;}a=Some(node::child(left,i));b=Some(node::child(dest,0));height=sub(height,1);}}self.fix_border(false);right.fix_border(true);letold=self.len();ifself.height<right.height{letcount=self.root.map_or(0,|r|Self::count_subtree(r,self.height.get()));self.len=Checked::new(count);right.len=Checked::new(sub(old,count));}else{letcount=right.root.map_or(0,|r|Self::count_subtree(r,right.height.get()));right.len=Checked::new(count);self.len=Checked::new(sub(old,count));}}Ok(right)',
 ('operations.rs', '', 'fn lower<'):
  'unsafe{matchself.search(key){Search::Found(c)=>{ifinclusive{Some(c)}else{c.next()}}Search::Vacant(Some(c))=>{ifc.index()<node::len(c.node){returnSome(c);}letmutn=c.node;letmuth=0;whileletSome((p,i))=node::parent(n){h=add(h,1);ifi<node::len(p){returnSome(Cursor::new(p,i,h));}n=p;}None}_=>None,}}',
 ('operations.rs', '', 'fn upper<'):
  'unsafe{matchself.search(key){Search::Found(c)=>{ifinclusive{Some(c)}else{c.prev()}}Search::Vacant(Some(c))=>{ifc.index()>0{returnSome(Cursor::new(c.node,sub(c.index(),1),0));}letmutn=c.node;letmuth=0;whileletSome((p,i))=node::parent(n){h=add(h,1);ifi>0{returnSome(Cursor::new(p,sub(i,1),h));}n=p;}None}_=>None,}}',
 ('operations.rs', '', 'unsafe fn clone_node<'):
  'letnode=node::allocate(height,policy)?;letmutguard=node::NodeGuard{node,height,pairs:0,children:0,policy,};unsafe{letn=node::len(source);whileguard.pairs<n{leti=guard.pairs;letmutk=InitSlot::new(core::ptr::NonNull::new_unchecked(node::key(node,i)));letmutv=InitSlot::new(core::ptr::NonNull::new_unchecked(node::val(node,i)));key(&*node::key(source,i),&mutk)?;value(&*node::val(source,i),&mutv)?;ifk.get().is_none()||v.get().is_none(){returnErr(Error::Uninitialized);}k.finish()?;v.finish()?;guard.pairs=add(i,1);node::set_len(node,guard.pairs);}ifheight>0{whileguard.children<=n{leti=guard.children;letchild=clone_node(node::child(source,i),sub(height,1),policy,key,value)?;node::set_child(node,i,child);guard.children=add(i,1);}}}mem::forget(guard);Ok(node)',
 ('operations.rs', 'TryIterator\n    for ExtractIf<', 'fn try_next('):
  'unsafe{whileletSome(c)=self.state.take(false){if(self.predicate)(&*node::key(c.node,c.index()),&mut*node::val(c.node,c.index()),)?{letmuttracked=[self.state.front,self.state.back];letpair=self.map.remove_at(c,&muttracked);[self.state.front,self.state.back]=tracked;returnOk(Some(pair));}}}Ok(None)',
 ('bulk.rs', "impl<'a, K, V> PlanningStream<'a, K, V> {", 'fn next('):
  'ifself.state.remaining.get()==0{returnNone;}letc=self.state.front?;unsafe{letkey=&*node::key(c.node,c.index());self.state.advance_owned(c,false,|_,h|{retired.record_retired(h);});Some(key)}',
 ('bulk.rs', '', 'fn pillar_from_pool<'):
  'letmutmap=BTreeMap::new_in(policy);map.root=Some(pool.take(0)?);whilemap.height.get()<height{letn=pool.take(1)?;ifletSome(child)=map.root{unsafe{node::set_child(n,0,child)};}map.root=Some(n);map.height=Checked::new(add(map.height.get(),1));}Ok(map)',
 ('bulk.rs', '', 'unsafe fn allocated_bytes('):
  'letmutbytes=node::layout::<K,V>(h).size();unsafe{ifh>0{foriin0..=node::len(n){bytes=bytes.checked_add(Self::allocated_bytes(node::child(n,i),sub(h,1))?).ok_or(Error::Capacity)?;}}}Ok(bytes)',
 ('bulk.rs', '', 'fn bulk_push<'):
  'let(k,v)=pair;letSome(mutn)=*leafelse{letn=matchpool.take(0){Ok(n)=>n,Err(error)=>{returnErr(Rejected{value:(k,v),error,});}};unsafe{node::insert(n,0,k,v,None)};self.root=Some(n);self.len=Checked::new(1);*leaf=Some(n);returnOk(());};unsafe{ifnode::len(n)<CAP{node::insert(n,node::len(n),k,v,None);}else{letmutheight=0;whilenode::len(n)==CAP{matchnode::parent(n){Some((parent,_))=>{n=parent;height=add(height,1);}None=>break,}}letnew_root=node::len(n)==CAP;lettarget_height=ifnew_root{add(height,1)}else{height};letmutright=matchSelf::pillar_from_pool(sub(target_height,1),&self.policy,pool){Ok(r)=>r,Err(error)=>{returnErr(Rejected{value:(k,v),error,});}};letparent=ifnew_root{letparent=matchpool.take(1){Ok(p)=>p,Err(error)=>{returnErr(Rejected{value:(k,v),error,});}};node::set_child(parent,0,n);self.root=Some(parent);self.height=Checked::new(target_height);parent}else{n};letchild=right.root.take();letmutlast=child;letmuth=right.height.get();whileh>0{ifletSome(n)=last{last=Some(node::child(n,0));}h=sub(h,1);}node::insert(parent,node::len(parent),k,v,child);*leaf=last;}self.len=Checked::new(add(self.len(),1));}Ok(())',
 ('bulk.rs', '', 'unsafe fn fix_packed_border('):
  'unsafe{letSome(mutn)=self.rootelse{return};letmuth=self.height.get();whileh>0{leti=sub(node::len(n),1);letright=node::child(n,add(i,1));whilenode::len(right)<MIN{node::steal_left(n,i,sub(h,1));}n=right;h=sub(h,1);}}',
 ('view.rs', '', 'pub fn child('):
  'ifself.height==0{returnifindex==0{Ok(None)}else{Err(Error::Bounds)};}letn=unsafe{node::len(self.node)};ifindex>n{returnErr(Error::Bounds);}Ok(Some(Self{node:unsafe{node::child(self.node,index)},height:super::sub(self.height,1),marker:PhantomData,}))',
}
# Maintained caller -> product calls its text must contain.
CALLERS={
 'btree_count_index':['ensures(nasu16)asusize==n,'],
 'btree_pair_address_index':['storage_range_condition(n.keys,i,0)', 'storage_range_condition(n.vals,i,0)'],
 'btree_pair_index':['storage_range_condition(n.keys,i,1)', 'storage_range_condition(n.vals,i,1)'],
 'btree_pairs_index':['storage_copy_condition(a.keys,b.keys,from,to,count)', 'storage_copy_condition(a.vals,b.vals,from,to,count)'],
 'btree_edge_address_index':['storage_range_condition(n.edges,i,0)'],
 'btree_edge_index':['storage_range_condition(n.edges,i,1)'],
 'btree_edges_index':['storage_copy_condition(a.edges,b.edges,from,to,count)'],
 'btree_replace_index':['storage_replace_condition(n.keys,i)'],
 'btree_view_index':['storage_range_condition(n.keys,0,count)', 'storage_range_condition(n.vals,0,count)'],
 'btree_replace_key_caller':['btree_search_node_caller(count,h,replies)', 'crate::collections::btree_map::geometry::index(out.2)', 'btree_replace_index(index)'],
 'btree_actual_insert':['btree_pairs_index(i,after,tail)', 'btree_pair_index(i)', 'btree_edges_index(after,edge_to,tail)', 'btree_edge_index(after)'],
 'btree_actual_split':['btree_pairs_index(start,0,count)', 'btree_edges_index(start,0,edges)', 'btree_pair_index(mid)'],
 'btree_actual_move_suffix':['btree_pairs_index(index,0,count)', 'btree_edges_index(edge_start,1,count)'],
 'btree_actual_remove_leaf':['btree_pair_index(i)', 'btree_pairs_index(after,i,tail)'],
 'btree_actual_merge':['btree_edge_index(i)', 'btree_edge_index(right_child)', 'btree_pair_index(left_old)', 'btree_pair_index(i)', 'btree_pairs_index(0,start,right_old)', 'btree_edges_index(0,start,edges)', 'btree_pairs_index(after,i,tail)', 'btree_edges_index(edge_start,after,tail)'],
 'btree_actual_steal_left':['btree_edge_index(i)', 'btree_edge_index(right_child)', 'btree_pairs_index(0,1,right_old)', 'btree_pair_index(0)', 'btree_pair_index(left_length)', 'btree_edges_index(0,1,right_length)', 'btree_edge_index(left_edge)'],
 'btree_actual_steal_right':['btree_edge_index(i)', 'btree_edge_index(right_child)', 'btree_pair_index(left_old)', 'btree_pair_index(0)', 'btree_pairs_index(1,0,right_length)', 'btree_edge_index(left_length)', 'btree_edges_index(1,0,right_old)'],
 'btree_fix_caller':['crate::arithmetic::bounded::wrapping_add(i,1)'],
 'btree_drain_caller':['crate::arithmetic::bounded::wrapping_sub(children,1)', 'crate::arithmetic::bounded::wrapping_sub(height,1)', 'crate::arithmetic::bounded::wrapping_sub(pairs,1)'],
 'btree_destroy_caller':['crate::arithmetic::bounded::wrapping_add(n,1)', 'btree_drain_caller(children,n,height)'],
 'btree_actual_insert':['crate::collections::btree_map::geometry::insert_span(old,i)', 'btree_fix_caller(after,length)'],
 'btree_actual_split':['crate::collections::btree_map::geometry::split_span(old,mid)', 'btree_fix_caller(0,count)'],
 'btree_actual_move_suffix':['crate::collections::btree_map::geometry::suffix_span(left_old,index)', 'btree_fix_caller(1,count)'],
 'btree_actual_remove_leaf':['crate::collections::btree_map::geometry::remove_span(old,i)'],
 'btree_actual_merge':['crate::collections::btree_map::geometry::merge_span(left_old,right_old)', 'crate::collections::btree_map::geometry::remove_span(parent_old,i)', 'btree_fix_caller(start,length)', 'btree_fix_caller(after,parent_length)'],
 'btree_actual_steal_left':['crate::collections::btree_map::geometry::steal_left_span(left_old,right_old)', 'btree_fix_caller(0,right_length)'],
 'btree_actual_steal_right':['crate::collections::btree_map::geometry::steal_right_span(left_old,right_old)', 'btree_fix_caller(0,right_length)'],
 'btree_search_node_caller':['crate::collections::btree_map::geometry::pack(i,h)', 'crate::collections::btree_map::geometry::pack(i,0)', 'crate::arithmetic::bounded::wrapping_add(i,1)', 'crate::arithmetic::bounded::wrapping_sub(h,1)'],
 'btree_found_slot_caller':['btree_search_node_caller(count,h,replies)', 'crate::collections::btree_map::geometry::index(out.2)'],
 'btree_commit_fit_caller':['btree_search_node_caller(count,0,replies)', 'crate::collections::btree_map::geometry::index(out.2)', 'btree_actual_insert(count,index)'],
 'btree_plan_caller':['crate::arithmetic::bounded::wrapping_sub(usize::BITSasusize,1)', 'crate::collections::btree_map::geometry::pack(0,new_height)', 'crate::collections::btree_map::geometry::pack(parent_index,', 'crate::arithmetic::bounded::wrapping_add(height,1)'],
 'btree_commit_split_caller':['btree_splitpoint(edge)', 'btree_actual_split(11,mid)', 'crate::collections::btree_map::geometry::split_span(11,mid)', 'btree_actual_insert(mid,edge)', 'btree_actual_insert(right,index)', 'crate::collections::btree_map::geometry::pack(parent_index,'],
 'btree_commit_final_caller':['btree_actual_insert(count,index)'],
 'btree_remove_start_caller':['btree_actual_remove_leaf(leaf_count,leaf_index)', 'crate::arithmetic::bounded::wrapping_sub(length,1)'],
 'btree_rebalance_caller':['crate::collections::btree_map::geometry::consider_left(i)', 'crate::collections::btree_map::geometry::borrow_required(left,count)', 'crate::collections::btree_map::geometry::consider_right(i,parent)', 'crate::collections::btree_map::geometry::borrow_required(right,count)', 'btree_actual_steal_left(left,count,parent,sep)', 'btree_actual_steal_right(count,right,parent,i)', 'btree_actual_merge(l,r,parent,sep)'],
 'btree_shrink_caller':['crate::arithmetic::bounded::wrapping_sub(height,1)'],
 'btree_descend_caller':['crate::arithmetic::bounded::wrapping_sub(h,1)', 'crate::collections::btree_map::geometry::pack(0,0)', 'crate::collections::btree_map::geometry::pack(last,0)'],
 'btree_cursor_step_caller':['crate::arithmetic::bounded::wrapping_add(index,1)', 'crate::arithmetic::bounded::wrapping_sub(index,1)', 'crate::arithmetic::bounded::wrapping_add(climbed,1)', 'crate::collections::btree_map::geometry::pack(at,0)', 'crate::collections::btree_map::geometry::pack(at,h)', 'crate::collections::btree_map::geometry::pack(parent_index,h)'],
 'btree_state_take_caller':['crate::arithmetic::bounded::wrapping_sub(remaining,1)'],
 'btree_retire_chain_caller':['crate::arithmetic::bounded::wrapping_add(level,1)'],
 'btree_borrow_remaining_caller':['crate::collections::btree_map::geometry::pack(index,level)', 'crate::collections::btree_map::geometry::height(position)', 'crate::collections::btree_map::geometry::replace_index(position,', 'crate::collections::btree_map::geometry::index(position)'],
 'btree_borrow_interval_caller':['crate::arithmetic::bounded::wrapping_add(index,1)', 'crate::arithmetic::bounded::wrapping_sub(level,1)'],
 'btree_borrow_take_caller':['crate::collections::btree_map::geometry::leaf_edge(index,count,back)', 'crate::arithmetic::bounded::wrapping_add(climbed,1)', 'crate::arithmetic::bounded::wrapping_sub(remaining,1)', 'crate::arithmetic::bounded::wrapping_sub(h,1)'],
 'btree_ordered_caller':['crate::arithmetic::bounded::wrapping_add(a,crate::arithmetic::bounded::wrapping_add(a,1))', 'crate::arithmetic::bounded::wrapping_add(parent,parent)'],
 'btree_fix_border_caller':['btree_actual_merge(a,b,count,sep)', 'btree_actual_steal_right(a,b,count,sep)', 'btree_actual_steal_left(a,b,count,sep)', 'crate::collections::btree_map::geometry::steal_right_span(a,b)', 'crate::collections::btree_map::geometry::steal_left_span(a,b)'],
 'btree_count_caller':['crate::arithmetic::bounded::wrapping_add(total,children[i])', 'crate::arithmetic::bounded::wrapping_sub(level,1)'],
 'btree_split_path_caller':['crate::arithmetic::bounded::wrapping_add(i,1)', 'crate::arithmetic::bounded::wrapping_add(level,1)'],
 'btree_split_off_caller':['btree_actual_move_suffix(count,i)', 'crate::arithmetic::bounded::wrapping_sub(old,moved)', 'crate::arithmetic::bounded::wrapping_add(height,1)', 'btree_fix_caller(0,0)'],
 'btree_bound_caller':['crate::collections::btree_map::geometry::pack(at,0)', 'crate::collections::btree_map::geometry::pack(parent_index,h)', 'crate::collections::btree_map::geometry::pack(at,h)'],
 'btree_clone_caller':['btree_drain_caller(children,pairs,height)', 'crate::arithmetic::bounded::wrapping_add(i,1)', 'crate::arithmetic::bounded::wrapping_sub(height,1)'],
 'btree_pillar_caller':['btree_fix_caller(0,0)', 'crate::arithmetic::bounded::wrapping_add(h,1)'],
 'btree_bytes_caller':['crate::arithmetic::bounded::wrapping_sub(level,1)'],
 'btree_bulk_push_caller':['btree_actual_insert(0,0)', 'btree_actual_insert(n,n)', 'btree_actual_insert(count,count)', 'btree_pillar_caller(pillar)', 'crate::arithmetic::bounded::wrapping_sub(target_height,1)', 'crate::arithmetic::bounded::wrapping_add(length,1)'],
 'btree_packed_border_caller':['btree_actual_steal_left(left,right,count,i)', 'crate::collections::btree_map::geometry::steal_left_span(left,right)', 'crate::arithmetic::bounded::wrapping_sub(count,1)'],
 'btree_view_child_caller':['crate::arithmetic::bounded::wrapping_sub(level,1)'],
}
# Restated product bodies: (product key, maintained name, maintained body suffix).
RESTATED=[(('node.rs','','fn splitpoint('),'btree_splitpoint','ifedge<5{4}elseifedge<=6{5}else{6}}')]
# Unsafe site (file, function, context type or None for any) -> maintained caller.
SITES={
 ('node.rs', 'set_len', None):'btree_count_index',
 ('node.rs', 'key', None):'btree_pair_address_index',
 ('node.rs', 'val', None):'btree_pair_address_index',
 ('node.rs', 'edge', None):'btree_edge_address_index',
 ('node.rs', 'child', None):'btree_edge_index',
 ('node.rs', 'set_child', None):'btree_edge_index',
 ('node.rs', 'pairs', None):'btree_pairs_index',
 ('node.rs', 'edges', None):'btree_edges_index',
 ('node.rs', 'replace_key', None):'btree_replace_index',
 ('node.rs', 'drop_pair', None):'btree_pair_index',
 ('view.rs', 'keys', None):'btree_view_index',
 ('view.rs', 'values', None):'btree_view_index',
 ('btree_map.rs', 'try_replace_key', None):'btree_replace_key_caller',
 ('btree_map.rs', 'search', None):'btree_search_node_caller',
 ('btree_map.rs', 'get', None):'btree_found_slot_caller',
 ('btree_map.rs', 'get_key_value', None):'btree_found_slot_caller',
 ('btree_map.rs', 'get_mut', None):'btree_found_slot_caller',
 ('btree_map.rs', 'try_insert', None):'btree_found_slot_caller',
 ('btree_map.rs', 'commit_fit', None):'btree_commit_fit_caller',
 ('btree_map.rs', 'try_insert_at', None):'btree_commit_fit_caller',
 ('btree_map.rs', 'plan_insert', None):'btree_plan_caller',
 ('btree_map.rs', 'commit_insert', None):'btree_commit_split_caller',
 ('btree_map.rs', 'remove_at', None):'btree_rebalance_caller',
 ('btree_map.rs', 'pop_first', None):'btree_rebalance_caller',
 ('btree_map.rs', 'pop_last', None):'btree_rebalance_caller',
 ('btree_map.rs', 'remove_entry', None):'btree_rebalance_caller',
 ('btree_map.rs', 'shrink_root', None):'btree_shrink_caller',
 ('btree_map.rs', 'first_cursor', None):'btree_descend_caller',
 ('btree_map.rs', 'last_cursor', None):'btree_descend_caller',
 ('btree_map.rs', 'first_key_value', None):'btree_descend_caller',
 ('btree_map.rs', 'last_key_value', None):'btree_descend_caller',
 ('btree_map.rs', 'clear', None):'btree_destroy_caller',
 ('entry.rs', 'key', 'OccupiedEntry'):'btree_found_slot_caller',
 ('entry.rs', 'get', 'OccupiedEntry'):'btree_found_slot_caller',
 ('entry.rs', 'get_mut', 'OccupiedEntry'):'btree_found_slot_caller',
 ('entry.rs', 'into_mut', 'OccupiedEntry'):'btree_found_slot_caller',
 ('entry.rs', 'remove_entry', 'OccupiedEntry'):'btree_rebalance_caller',
 ('entry.rs', 'try_insert_with', 'VacantEntry'):'btree_commit_fit_caller',
 ('iter.rs', 'first', 'Cursor'):'btree_descend_caller',
 ('iter.rs', 'last', 'Cursor'):'btree_descend_caller',
 ('iter.rs', 'next', 'Cursor'):'btree_cursor_step_caller',
 ('iter.rs', 'prev', 'Cursor'):'btree_cursor_step_caller',
 ('iter.rs', 'take', 'State'):'btree_state_take_caller',
 ('iter.rs', 'advance_owned', 'State'):'btree_retire_chain_caller',
 ('iter.rs', 'edge', 'BorrowState'):'btree_descend_caller',
 ('iter.rs', 'new', 'BorrowState'):'btree_descend_caller',
 ('iter.rs', 'remaining', 'BorrowState'):'btree_borrow_remaining_caller',
 ('iter.rs', 'interval', 'BorrowState'):'btree_borrow_interval_caller',
 ('iter.rs', 'take', 'BorrowState'):'btree_borrow_take_caller',
 ('iter.rs', 'next', 'Iter'):'btree_borrow_take_caller',
 ('iter.rs', 'next_back', 'Iter'):'btree_borrow_take_caller',
 ('iter.rs', 'next', 'IterMut'):'btree_borrow_take_caller',
 ('iter.rs', 'next_back', 'IterMut'):'btree_borrow_take_caller',
 ('iter.rs', 'next', 'Range'):'btree_borrow_take_caller',
 ('iter.rs', 'next_back', 'Range'):'btree_borrow_take_caller',
 ('iter.rs', 'next', 'RangeMut'):'btree_borrow_take_caller',
 ('iter.rs', 'next_back', 'RangeMut'):'btree_borrow_take_caller',
 ('iter.rs', 'take_state', 'IntoIter'):'btree_cursor_step_caller',
 ('operations.rs', 'ordered', None):'btree_ordered_caller',
 ('operations.rs', 'fix_border', None):'btree_fix_border_caller',
 ('operations.rs', 'count_subtree', None):'btree_count_caller',
 ('operations.rs', 'try_split_off', None):'btree_split_off_caller',
 ('operations.rs', 'lower', None):'btree_bound_caller',
 ('operations.rs', 'upper', None):'btree_bound_caller',
 ('operations.rs', 'try_range', None):'btree_borrow_interval_caller',
 ('operations.rs', 'try_range_mut', None):'btree_borrow_interval_caller',
 ('operations.rs', 'try_next', None):'btree_cursor_step_caller',
 ('operations.rs', 'clone_node', None):'btree_clone_caller',
 ('operations.rs', 'try_clone', None):'btree_clone_caller',
 ('operations.rs', 'try_clone_from', None):'btree_clone_caller',
 ('operations.rs', 'try_clone_in', None):'btree_clone_caller',
 ('bulk.rs', 'next', 'PlanningStream'):'btree_state_take_caller',
 ('bulk.rs', 'pillar_from_pool', None):'btree_pillar_caller',
 ('bulk.rs', 'allocated_bytes', None):'btree_bytes_caller',
 ('bulk.rs', 'bulk_push', None):'btree_bulk_push_caller',
 ('bulk.rs', 'fix_packed_border', None):'btree_packed_border_caller',
 ('bulk.rs', 'try_from_sorted_source', None):'btree_packed_border_caller',
 ('bulk.rs', 'drop', 'Repair'):'btree_packed_border_caller',
 ('view.rs', 'child', None):'btree_view_child_caller',
 ('node.rs', 'fix', None):'btree_fix_caller',
 ('node.rs', 'drain', None):'btree_drain_caller',
 ('node.rs', 'destroy', None):'btree_destroy_caller',
}
