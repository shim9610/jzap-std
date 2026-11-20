"""Reviewed HashMap phase boundaries; hashes invalidate review, never prove code."""
import hashlib,json,re,subprocess
from pathlib import Path
from scanner_tools import scanner
ROOT=Path(__file__).resolve().parents[1]
FILES=['src/collections/hash_map.rs',*('src/collections/hash_map/'+f+'.rs' for f in ['entropy','entry','geometry','group','hash','iter','raw'])]
def is_hash_map(file):return file in FILES
def is_copy_capability_site(row):
 # A zero-sized PhantomData capability has no numeric owner state. Its exact
 # shared source is reviewed below and compiler-enforced bounds are checked by
 # verify_hash_map, including a compiled missing-Copy-bound defect.
 d=row.get('detail',{})
 return (row['file']=='src/clone.rs' and row['function'] in ('new','for_array') and
     row['context'].split('<')[0].strip()=='CopyWitness' and row['kind']=='field' and
     d.get('constructor') in ('Self','CopyWitness') and d.get('field')=='marker' and
     ''.join(d.get('value','').split())=='core::marker::PhantomData')
REVIEWED={'src/collections/hash_map/entropy.rs': '7e9b67ebd9570ce03a519651ca288ed6fef887303451497c946ba330a192f1b5', 'src/collections/hash_map.rs': 'e716aec2b177e9871e5f9a7690e9f6b0c564626f40f411515bae744e53fbc8de', 'src/collections/hash_map/entry.rs': 'fe8ebb8355e09961fdf316689d8727ff7fa88f1129c6437c74cc397f4b5e5bef', 'src/collections/hash_map/geometry.rs': 'a9c60a1831f2c3a35d9c06e292bc58f807f75baf28f8bba4e42f9bfd77304dbe', 'src/collections/hash_map/group.rs': 'c160a1de2f4c5892b704664c07dbe789782a46148223fd45262007b8ee579ab7', 'src/collections/hash_map/hash.rs': '115669968d99257578fea88fcdd1db51430557403b5fcbea5ab6014b5ca3a11d', 'src/collections/hash_map/iter.rs': '1fe28417666310b6324f33ac09be3fedcb8c79a5e431399238aacb32e8bd85c5', 'src/collections/hash_map/raw.rs': '739b450ec8470a6be537088950ea688b69c942df4607ea2bf9ad72a1c2fbee55'}
# Product function -> maintained caller in tools/memory-proof/hash-map.verus.rs.
RAW={'allocate':'hash_allocate_caller','try_clone_copy':'hash_allocation_caller','allocation':'hash_allocation_caller',
 ('drop','metadata_init'):'hash_allocation_caller',('drop','unsafe_block','Owner'):'hash_allocation_caller',('drop','unsafe_block','Storage'):'hash_allocation_caller',
 'slot':'hash_slot_caller','control':'hash_set_control_caller','set_control':'hash_set_control_caller',
 'find':'hash_probe_caller','vacant':'hash_probe_caller','matching':'hash_probe_caller','find_insert':'hash_find_insert_caller',
 'neon_mask':'hash_neon_mask_octet','neon_nibbles':'hash_neon_nibble_pair',
 'any_empty':'hash_probe_caller','first_vacant':'hash_probe_caller',
 ('load','unsafe_block','Vacancy'):'hash_probe_caller',
 'insert_at':'hash_insert_at_caller','erase':'hash_erase_caller','clear':'hash_reset_caller',('run','unsafe_block'):'hash_clear_run_caller'}
# Position sites outside the table kernel: (file, function, context) -> maintained caller.
POSITION={('hash_map.rs','try_retain','HashMap'):'hash_slot_caller',('hash_map.rs','get','HashMap'):'hash_slot_caller',
 ('hash_map.rs','get_key_value','HashMap'):'hash_slot_caller',('hash_map.rs','get_mut','HashMap'):'hash_slot_caller',
 ('hash_map.rs','try_insert','HashMap'):'hash_slot_caller',('hash_map.rs','remove_entry','HashMap'):'hash_erase_caller',
 ('hash_map.rs','prepare_insert_at','HashMap'):'hash_set_control_caller',
 ('entry.rs','key','OccupiedEntry'):'hash_slot_caller',('entry.rs','get','OccupiedEntry'):'hash_slot_caller',
 ('entry.rs','get_mut','OccupiedEntry'):'hash_slot_caller',('entry.rs','into_mut','OccupiedEntry'):'hash_slot_caller',
 ('entry.rs','remove_entry','OccupiedEntry'):'hash_erase_caller',
 ('iter.rs','fill_group','Scan'):'hash_scan_caller',('iter.rs','next','Iter'):'hash_slot_caller',('iter.rs','fold','Iter'):'hash_slot_caller',
 ('iter.rs','next','IterMut'):'hash_slot_caller',('iter.rs','fold','IterMut'):'hash_slot_caller',('iter.rs','try_next','ExtractIf'):'hash_slot_caller'}
# Ownership/credit sites: the numeric part is a maintained caller; the credit
# invariant and the FULL-tag/initialization agreement are its stated premises.
CREDIT={('iter.rs','next','IntoIter'):'hash_retire_caller',('iter.rs','fold','IntoIter'):'hash_retire_caller',
 ('iter.rs','next','Drain'):'hash_retire_caller',('iter.rs','fold','Drain'):'hash_retire_caller',
 ('raw.rs','run','Clear'):'hash_retire_caller',('raw.rs','drop','Free'):'hash_allocation_caller',
 ('hash_map.rs','resize','HashMap'):'hash_resize_caller',('hash_map.rs','try_clone','HashMap'):'hash_clone_caller',
 ('hash_map.rs','insert_vacant_at','HashMap'):'hash_insert_at_caller',('entry.rs','initialize','VacantEntry'):'hash_insert_at_caller'}
# Sites that keep a lemma or harness for a non-numeric fact; the maintained
# callers listed here cover their positions and arithmetic.
ALSO={('hash_map.rs','try_get_disjoint_mut'):['hash_slot_caller'],('hash_map.rs','get_disjoint_unchecked_mut'):['hash_slot_caller'],
 ('raw.rs','rehash'):['hash_set_control_caller','hash_slot_caller','hash_rehash_distance_caller','hash_growth_caller']}
# Whole bodies of the position callers: (file, anchor, head) -> compact body.
POSITION_BODIES={
 ('hash_map.rs','','fn find<'):'ifself.is_empty(){returnNone;}self.table.find(self.hash(key),|k|k.borrow()==key)',
 ('hash_map.rs','','pub fn get<'):'letindex=self.find(key)?;Some(unsafe{&*ptr::addr_of!((*self.table.slot(index)).1)})',
 ('hash_map.rs','','pub fn get_key_value<'):'letindex=self.find(key)?;Some(unsafe{letp=self.table.slot(index);(&*ptr::addr_of!((*p).0),&*ptr::addr_of!((*p).1))})',
 ('hash_map.rs','','pub fn get_mut<'):'letindex=self.find(key)?;Some(unsafe{&mut*ptr::addr_of_mut!((*self.table.slot(index)).1)})',
 ('hash_map.rs','','pub fn remove_entry<'):'letindex=self.find(key)?;Some(unsafe{self.table.erase(index)})',
 ('hash_map.rs','','pub fn try_retain<'):'letmutscan=iter::Scan::new(&self.table);whileletSome(index)=scan.next(){unsafe{letp=self.table.slot(index);if!predicate(&*ptr::addr_of!((*p).0),&mut*ptr::addr_of_mut!((*p).1))?{drop(self.table.erase(index));}}}Ok(())',
 ('hash_map.rs','','fn prepare_insert_at('):'ifletSome(index)=candidate.or_else(||self.table.vacant(hash)){ifself.table.growth.get()>0||unsafe{self.table.control(index)}==group::DELETED{returnOk(index);}}self.try_reserve(1)?;self.table.vacant(hash).ok_or(Error::InvalidStructure)',
 ('entry.rs','','pub fn key(&self)'):'unsafe{&*ptr::addr_of!((*self.map.table.slot(self.index)).0)}',
 ('entry.rs','','pub fn get(&self)'):'unsafe{&*ptr::addr_of!((*self.map.table.slot(self.index)).1)}',
 ('entry.rs','','pub fn get_mut(&mut self)'):'unsafe{&mut*ptr::addr_of_mut!((*self.map.table.slot(self.index)).1)}',
 ('entry.rs','','pub fn into_mut(self)'):'unsafe{&mut*ptr::addr_of_mut!((*self.map.table.slot(self.index)).1)}',
 ('entry.rs','','pub fn remove_entry(self)'):'unsafe{self.map.table.erase(self.index)}',
 ('iter.rs','','pub(super) fn new<'):'Self{ctrl:table.ctrl,mask:table.mask,group:0,bits:0,remaining:table.len,}',
 ('iter.rs','','fn fill_group('):'whileself.bits==0{ifself.group>self.mask{self.remaining=Checked::new(0);returnfalse;}self.bits=unsafe{!group::vacant(self.ctrl.as_ptr().add(self.group))};self.group=add(self.group,group::WIDTH);}true',
 ('iter.rs','','fn take_cached('):'letindex=add(sub(self.group,group::WIDTH),group::first(self.bits));self.bits=group::clear_first(self.bits);self.remaining=Checked::new(sub(self.remaining.get(),1));index',
 ('iter.rs','','pub(super) fn next('):'ifself.remaining.get()==0||!self.fill_group(){returnNone;}Some(self.take_cached())',
 ('iter.rs','','fn fold_indices<'):'whileself.remaining.get()>0{if!self.fill_group(){break;}whileself.bits!=0{value=f(value,self.take_cached());}}value',
 ('iter.rs','> Iterator for Iter<','fn next('):'letindex=self.scan.next()?;unsafe{letp=self.scan.ctrl.as_ptr().cast::<(K,V)>().sub(add(index,1));Some((&*ptr::addr_of!((*p).0),&*ptr::addr_of!((*p).1)))}',
 ('iter.rs','> Iterator for Iter<','fn fold<'):'letctrl=self.scan.ctrl;self.scan.fold_indices(init,|value,index|{letitem=unsafe{letp=ctrl.as_ptr().cast::<(K,V)>().sub(add(index,1));(&*ptr::addr_of!((*p).0),&*ptr::addr_of!((*p).1))};f(value,item)})',
 ('iter.rs','> Iterator for IntoIter<','fn next('):'letindex=self.scan.next()?;unsafe{ifcore::mem::needs_drop::<(K,V)>(){self.table.set_control(index,group::EMPTY);self.table.len=Checked::new(sub(self.table.len.get(),1));}Some(self.table.slot(index).read())}',
 ('iter.rs','> Iterator for IntoIter<','fn fold<'):'lettable=&mutself.table;self.scan.fold_indices(init,|value,index|{letitem=unsafe{ifcore::mem::needs_drop::<(K,V)>(){table.set_control(index,group::EMPTY);table.len=Checked::new(sub(table.len.get(),1));}table.slot(index).read()};f(value,item)})',
 ('iter.rs','> Iterator for Drain<','fn next('):'letindex=self.scan.next()?;unsafe{ifcore::mem::needs_drop::<(K,V)>(){self.table.set_control(index,group::EMPTY);self.table.len=Checked::new(sub(self.table.len.get(),1));}Some(self.table.slot(index).read())}',
 ('iter.rs','> Iterator for Drain<','fn fold<'):'lettable=&mutself.table;self.scan.fold_indices(init,|value,index|{letitem=unsafe{ifcore::mem::needs_drop::<(K,V)>(){table.set_control(index,group::EMPTY);table.len=Checked::new(sub(table.len.get(),1));}table.slot(index).read()};f(value,item)})',
 ('hash_map.rs','','pub unsafe fn get_disjoint_unchecked_mut<'):'letindices=keys.map(|key|self.find(key));core::array::from_fn(|i|{letindex=indices.get(i).copied().flatten()?;Some(unsafe{&mut*ptr::addr_of_mut!((*self.table.slot(index)).1)})})',
 ('hash_map.rs','','pub fn try_get_disjoint_mut<'):'letindices=keys.map(|key|self.find(key));letmutrest=indices.as_slice();whileletSome((index,tail))=rest.split_first(){ifindex.is_some()&&tail.contains(index){returnErr(Error::Bounds);}rest=tail;}Ok(core::array::from_fn(|i|{letindex=indices.get(i).copied().flatten()?;Some(unsafe{&mut*ptr::addr_of_mut!((*self.table.slot(index)).1)})}))',
 ('hash_map.rs','','fn insert_vacant_at('):'letindex=matchself.prepare_insert_at(hash,candidate){Ok(index)=>index,Err(error)=>{returnErr(Rejected{value:(key,value),error,});}};unsafe{self.table.insert_at(index,hash,(key,value));}Ok(index)',
 ('hash_map.rs','','fn try_clone('):'letmutresult=Self::with_hasher_in(self.hasher.try_clone()?,self.policy.try_clone()?);iflet(Some(keys),Some(values))=(K::copy_witness(),V::copy_witness()){result.table=self.table.try_clone_copy(result.policy.budget(),keys,values)?;returnOk(result);}result.table=RawTable::allocate(self.table.buckets(),result.policy.budget())?;forindexin0..self.table.buckets(){unsafe{ifself.table.control(index)&0x80==0{letpair=(&*self.table.slot(index)).try_clone()?;result.table.slot(index).write(pair);result.table.set_control(index,self.table.control(index));result.table.len=Checked::new(add(result.table.len.get(),1));}}}ifresult.table.mask!=0{result.table.growth=Checked::new(sub(raw::capacity_for(result.table.mask),result.len()));}ifself.table.mask!=0{unsafe{ptr::copy_nonoverlapping(self.table.ctrl.as_ptr(),result.table.ctrl.as_ptr(),add(self.table.buckets(),group::WIDTH),)};result.table.growth=self.table.growth;}Ok(result)',
 ('iter.rs','> Iterator for IterMut<','fn next('):'letindex=self.scan.next()?;unsafe{letp=self.scan.ctrl.as_ptr().cast::<(K,V)>().sub(add(index,1));Some((&*ptr::addr_of!((*p).0),&mut*ptr::addr_of_mut!((*p).1)))}',
 ('iter.rs','> Iterator for IterMut<','fn fold<'):'letctrl=self.scan.ctrl;self.scan.fold_indices(init,|value,index|{letitem=unsafe{letp=ctrl.as_ptr().cast::<(K,V)>().sub(add(index,1));(&*ptr::addr_of!((*p).0),&mut*ptr::addr_of_mut!((*p).1))};f(value,item)})',
 ('iter.rs','for ExtractIf<','fn try_next('):'whileletSome(index)=self.scan.next(){unsafe{letp=self.map.table.slot(index);if(self.predicate)(&*ptr::addr_of!((*p).0),&mut*ptr::addr_of_mut!((*p).1))?{returnOk(Some(self.map.table.erase(index)));}}}Ok(None)'}
# Statements that fix where an occupied index comes from.
POSITION_PARTS={('hash_map.rs','','pub fn try_insert('):['letslot=self.table.find_insert(hash,|stored|stored==&key);ifletSome(raw::InsertSlot::Occupied(index))=slot{letold=unsafe{mem::replace(&mut*ptr::addr_of_mut!((*self.table.slot(index)).1),value)};'],
 ('hash_map.rs','','pub fn entry('):['ifletSome(index)=self.table.find(hash,|stored|stored==&key){drop(key);Entry::Occupied(OccupiedEntry{map:self,index})}'],
 ('hash_map.rs','','fn resize('):['letmutdestination=raw::Storage{table:RawTable::allocate(buckets,budget)?,budget,};','letmutscan=iter::Scan::new(source);whileletSome(index)=scan.next(){unsafe{lethash=hasher.hash_one(&*ptr::addr_of!((*source.slot(index)).0));lettarget=destination.table.vacant(hash).ok_or(Error::InvalidStructure)?;ptr::copy_nonoverlapping(source.slot(index),destination.table.slot(target),1);destination.table.set_control(target,raw::tag(hash));}}','letlen=self.table.len;letgrowth=raw::capacity_for(destination.table.mask).checked_sub(len.get()).ok_or(Error::InvalidStructure)?;destination.table.len=len;destination.table.growth=Checked::new(growth);mem::swap(&mutself.table,&mutdestination.table);'],
 ('entry.rs','','fn initialize<'):['letindex=self.map.prepare_insert(self.hash).map_err(InitError::Storage)?;','unsafe{self.map.table.insert_at(index,self.hash,(self.key,value));Ok(&mut*ptr::addr_of_mut!((*self.map.table.slot(index)).1))}'],
 ('raw.rs','','pub(crate) fn rehash('):['foriin0..self.buckets(){unsafe{letvalue=ifself.control(i)&0x80==0{DELETED}else{EMPTY};self.set_control(i,value)}}','guard.table.growth=Checked::new(sub(capacity_for(guard.table.mask),guard.table.len.get()));'],
 ('raw.rs','','fn run('):['self.table.set_control(index,EMPTY);self.table.len=Checked::new(sub(self.table.len.get(),1));ptr::drop_in_place(self.table.slot(index));','self.table.growth=Checked::new(sub(capacity_for(self.table.mask),self.table.len.get()));'],
 ('entry.rs','','pub fn try_insert_entry('):['letindex=self.map.insert_vacant(self.hash,self.key,value)?;Ok(OccupiedEntry{map:self.map,index,})']}
# Maintained callers and the product kernels each must call.
CALLERS={'hash_allocate_caller':['geometry::checked_sizes(size,alignment,buckets)','add(buckets,group::WIDTH)','sub(buckets,1)','raw::capacity_for(mask)','hash_controls_index(mask)'],
 'hash_allocation_caller':['geometry::checked_sizes(size,alignment,buckets)','add(mul(buckets,size),round)&!round','add(add(rebuilt,buckets),group::WIDTH)'],
 'hash_slot_caller':['geometry::checked_sizes(size,alignment,buckets)','add(index,1)','hash_slot_index(mask,index)'],
 'hash_slot_index':['storage_sub_condition(t.slots,(mask+1)asusize,(index+1)asusize)','storage_range_condition(t.slots,(mask-index)asusize,1)'],
 'hash_control_index':['storage_range_condition(t.controls,index,1)'],
 'hash_group_index':['storage_range_condition(t.controls,pos,16)'],
 'hash_controls_index':['storage_range_condition(t.controls,0,controls)'],
 'hash_set_control_caller':['geometry::mirror(index,add(mask,1))','hash_control_index(mask,index)','hash_control_index(mask,mirror)'],
 'hash_probe_caller':['hash_groups(mask)','group::first(first_group)','hash_buckets(mask)','group::first(bits)','group::clear_first(bits)','geometry::probe_next(pos,stride,mask)','sub(groups,1)','hash_group_index(mask,pos)','hash_slot_index(mask,index)'],
 'hash_find_insert_caller':['group::first(bits)','group::clear_first(bits)','group::first(vacant)','hash_buckets(mask)','geometry::probe_next(pos,stride,mask)','hash_group_index(mask,pos)','hash_slot_index(mask,index)'],
 'hash_insert_at_caller':['raw::capacity_for(mask)','sub(growth,1)','raw::tag(hash)','add(len,1)'],
 'hash_erase_caller':['geometry::mirror(index,add(mask,1))','group::WIDTH','add(growth,1)','sub(len,1)','hash_group_index(mask,before)','hash_group_index(mask,index)','hash_slot_index(mask,index)'],
 'hash_reset_caller':['geometry::checked_sizes(size,alignment,buckets)','add(buckets,group::WIDTH)','raw::capacity_for(mask)','hash_controls_index(mask)'],
 'hash_clear_run_caller':['hash_buckets(mask)','group::first(bits)','group::clear_first(bits)','add(base,group::WIDTH)','add(slot,1)','hash_group_index(mask,base)','hash_slot_index(mask,slot)','hash_control_index(mask,slot)'],
 'hash_scan_caller':['group::first(bits)','group::clear_first(bits)','add(group,group::WIDTH)','add(sub(group,group::WIDTH),group::first(bits))','hash_group_index(mask,group)','hash_slot_index(mask,index)'],
 'hash_retire_caller':['geometry::mirror(index,add(mask,1))','sub(len,1)','hash_slot_index(mask,index)','hash_control_index(mask,index)','hash_control_index(mask,mirror)'],
 'hash_resize_caller':['geometry::mirror(target,add(destination,1))','raw::tag(hash)','raw::capacity_for(destination).checked_sub(len)','hash_slot_index(source,index)','hash_slot_index(destination,target)','hash_control_index(destination,target)','hash_control_index(destination,mirror)'],
 'hash_clone_caller':['geometry::checked_sizes(size,alignment,buckets)','geometry::mirror(index,add(mask,1))','add(len,1)','raw::capacity_for(mask)','sub(capacity,len)','add(buckets,group::WIDTH)','hash_slot_index(mask,index)','hash_control_index(mask,index)','hash_control_index(mask,mirror)','hash_controls_index(mask)'],
 'hash_growth_caller':['sub(raw::capacity_for(mask),len)'],
 'hash_rehash_distance_caller':['hash_buckets(mask)','sub(add(index,buckets),start)&mask','sub(add(target,buckets),start)&mask']}
# RawTable methods cannot be called from Verus (NonNull field); their bodies
# are restated in the maintained functions below and compared whole.
RESTATED={'fn groups(':('hash_groups','ifself.mask<WIDTH{1}else{add(self.mask/WIDTH,1)}','ifmask<group::WIDTH{1}else{add(mask/group::WIDTH,1)}'),
 'pub(crate) fn buckets(':('hash_buckets','ifself.mask==0{0}else{add(self.mask,1)}','ifmask==0{0}else{add(mask,1)}')}
# Product statements the maintained callers follow.
PRODUCT={'allocate':['checked_layout(mem::size_of::<(K,V)>(),mem::align_of::<(K,V)>(),buckets)?','ptr::write_bytes(p,EMPTY,add(buckets,WIDTH))','letmask=sub(buckets,1);','growth:Checked::new(capacity_for(mask))'],
 'checked_layout':['super::geometry::checked_sizes(size,alignment,buckets)?'],
 'allocation':['letalign=mem::align_of::<(K,V)>().max(WIDTH);letround=sub(align,1);letoffset=add(mul(buckets,mem::size_of::<(K,V)>()),round)&!round;letbytes=add(add(offset,buckets),WIDTH);'],
 'slot':['self.ctrl.as_ptr().cast::<(K,V)>().sub(add(index,1))'],
 'set_control':['letmirror=mirror(index,add(self.mask,1));','self.ctrl.as_ptr().add(index).write(value);self.ctrl.as_ptr().add(mirror).write(value)'],
 'find':['letmutpos=(hashasusize)&self.mask;letmutstride=0;letmutgroups=self.groups();whilegroups>0{','letindex=add(pos,group::first(bits))&self.mask;','bits=group::clear_first(bits);','groups=sub(groups,1);ifgroups!=0{(pos,stride)=probe_next(pos,stride,self.mask);}'],
 'vacant':['letslots=group::Vacancy::load(self.ctrl.as_ptr().add(pos));ifslots.found(){letindex=add(pos,slots.lane())&self.mask;', 'letmutpos=(hashasusize)&self.mask;letmutstride=0;letmutgroups=self.groups();whilegroups>0{','groups=sub(groups,1);ifgroups!=0{(pos,stride)=probe_next(pos,stride,self.mask);}'],
 'find_insert':['letmutpos=(hashasusize)&self.mask;letmutstride=0;',
  'letslots=group::Vacancy::load(address);ifslots.found(){letindex=add(pos,slots.lane())&self.mask;',
  'ifgroup::any_empty(address){returnvacant.map(InsertSlot::Vacant);}','(pos,stride)=probe_next(pos,stride,self.mask);ifstride>self.mask{returnvacant.map(InsertSlot::Vacant);}'],
 'insert_at':['ifself.control(index)==EMPTY{self.growth=Checked::new(sub(self.growth.get(),1));}self.slot(index).write(pair);self.set_control(index,tag(hash));','self.len=Checked::new(add(self.len.get(),1));'],
 'erase':['letbefore=sub(mirror(index,add(self.mask,1)),WIDTH);','ifadd(aasusize,basusize)>=WIDTH{DELETED}else{self.growth=Checked::new(add(self.growth.get(),1));EMPTY}','self.len=Checked::new(sub(self.len.get(),1));'],
 'clear':['ptr::write_bytes(self.ctrl.as_ptr(),EMPTY,add(self.buckets(),WIDTH));','self.growth=Checked::new(capacity_for(self.mask));'],
 'run':['while!self.pending&&self.index<self.table.buckets(){letbase=self.index;','letindex=add(base,group::first(bits));bits=group::clear_first(bits);','self.index=add(base,WIDTH);','letindex=self.index;self.index=add(index,1);'],
 'rehash':['letstart=(valueasusize)&guard.table.mask;leta=sub(add(index,guard.table.buckets()),start)&guard.table.mask;letb=sub(add(target,guard.table.buckets()),start)&guard.table.mask;ifa/WIDTH==b/WIDTH{']}
def body(text,head):
 # Brace-matched function body, comments removed, whitespace removed.
 start=text.index(head);i=text.index('{',text.index(')',start));depth=0;j=i
 while True:
  depth+=text[j]=='{';depth-=text[j]=='}'
  if depth==0:break
  j+=1
 return re.sub(r'\s+','',re.sub(r'//[^\n]*','',text[i+1:j]))
def maintained(text,name):
 # Specification and body of one maintained function, up to its closing brace.
 start=text.index('\nfn '+name+'(');return re.sub(r'\s+','',re.sub(r'//[^\n]*','',text[start:text.index('\n}\n',start)]))
def correspondence(root=ROOT):
 root=Path(root);raw=(root/'src/collections/hash_map/raw.rs').read_text();model=(ROOT/'tools/memory-proof/hash-map.verus.rs').read_text()
 for name,calls in CALLERS.items():
  text=maintained(model,name)
  for call in calls:assert call in text,('maintained HashMap caller lost a product call',name,call)
 for head,(name,product,restated) in RESTATED.items():
  assert body(raw,head)==product,('HashMap restated product body changed',head)
  assert maintained(model,name).endswith('{'+restated),('HashMap restated maintained body changed',name)
 for name,parts in PRODUCT.items():
  text=body(raw,'fn '+name+'(')
  for part in parts:assert part in text,('HashMap product caller changed',name,part)
  # RawTable::vacant's fallback, followed by hash_probe_caller.
 assert 'letindex=group::first_vacant(self.ctrl.as_ptr());return(index<self.buckets()).then_some(index);' in body(raw,'pub(crate) fn vacant(')
 for (file,anchor,head),expected in POSITION_BODIES.items():
  text=(root/'src/collections'/('hash_map/'+file if file!='hash_map.rs' else file)).read_text();text=text[text.index(anchor):]
  assert body(text,head)==expected,('HashMap position caller changed',file,anchor,head)
 for (file,anchor,head),parts in POSITION_PARTS.items():
  text=(root/'src/collections'/('hash_map/'+file if file!='hash_map.rs' else file)).read_text();text=body(text[text.index(anchor):],head)
  for part in parts:assert part in text,('HashMap position caller changed',file,head,part)
 return dict(callers=sorted(CALLERS),restated=sorted(v[0] for v in RESTATED.values()),product=sorted(PRODUCT),position=sorted(' '.join(k) for k in [*POSITION_BODIES,*POSITION_PARTS]))
# Re-reviewed against 2a63fcb: only geometry's verus_keep_ghost module
# visibility changed. CopyWitness fields, bounds and constructors are identical;
# normal builds still keep geometry private. Retain the whole-file review gate.
CAPABILITY_REVIEWED='5283db7576da648252b14f1fac76274449d21728ced9a2fed0dd92f442fe98f9'
def check(root=ROOT):
 root=Path(root)
 actual={'src/collections/hash_map.rs',*(str(p.relative_to(root)) for p in (root/'src/collections/hash_map').glob('*.rs') if not p.name.endswith('proofs.rs'))}
 assert actual==set(FILES),'HashMap domain changed'
 phases={f:hashlib.sha256((root/f).read_bytes()).hexdigest() for f in FILES}
 assert phases==REVIEWED,'HashMap source/contract correspondence requires review'
 capability=hashlib.sha256((root/'src/clone.rs').read_bytes()).hexdigest()
 assert capability==CAPABILITY_REVIEWED,'Shared Copy capability constructors and privacy require review'
 phases['src/clone.rs']=capability
 rows=json.loads(subprocess.check_output([str(scanner('unsafe_sites')),*FILES],cwd=root,text=True))
 bindings=[]
 for row in rows:
  fn=row['function'];contract='hash_layout_ranges'
  if fn in ('try_replace_key','replace_key'):contract='kani:collections::hash_map::proofs::hash_set_replace_preserves_slot_class_length_and_credit'
  elif fn in ('insert_at','insert_vacant','insert_vacant_at','prepare_insert','prepare_insert_at','try_insert'):contract='hash_insert_credit'
  elif fn in ('erase','clear','run','drop','next','try_next','fold','take_cached'):contract='hash_retire_progress'
  elif fn in ('find','find_insert','vacant','probe_next','fill_group','fold_indices'):contract='hash_probe_progress'
  elif fn=='rehash':contract='hash_rehash_pending'
  elif fn in ('try_get_disjoint_mut','get_disjoint_unchecked_mut'):contract='hash_physical_duplicates'
  elif fn in ('try_clone','try_clone_copy','try_clone_in','resize'):contract='hash_clone_publication'
  elif fn=='set_control':contract='hash_mirror_padding'
  # Table-kernel sites are bound to maintained functions that call the
  # product kernels; ownership/credit sites keep the mathematical lemmas.
  contract=RAW.get((fn,row['kind'],row['context'].split('<')[0].strip()),RAW.get((fn,row['kind']),RAW.get(fn,contract))) if row['file'].endswith(('/raw.rs','/group.rs')) else contract
  if row['kind']=='unsafe_block':contract=POSITION.get((row['file'].split('/')[-1],fn,row['context'].split('<')[0].strip()),contract)
  key=(row['file'].split('/')[-1],fn,row['context'].split('<')[0].strip())
  if row['kind']=='metadata_write' or (row['kind']=='unsafe_block' and key[:2]!=('raw.rs','run')):contract=CREDIT.get(key,contract)
  also=ALSO.get(key[:2],[]) if not contract.endswith('_caller') else []
  bindings.append(dict(site=row,contract=contract,also=also,role='reviewed-initialized-slot/ownership-credit/probe-progress boundary',boundary='Numeric phase model plus actual Rust kernels/direct offsets; provenance, aliasing and callback cleanup use separate Miri/ASan/compiler checks.'))
 return phases,bindings
