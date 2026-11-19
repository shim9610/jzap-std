#!/usr/bin/env python3
"""Compile real collection ownership, policy and borrowed-view boundaries."""
import argparse,hashlib,json,subprocess,shutil,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
CASES=[
 ('scratch-owned-concurrent-static',True,"fn f(){fn both<T:Send+Sync+'static>(){}both::<jzap::scratch::ScratchBuf<u64,jzap::budget::OwnedConcurrentBudget>>();}"),
 ('scratch-local-not-send',False,"fn f(){fn send<T:Send>(){}send::<jzap::scratch::ScratchBuf<u64,jzap::budget::Budget<'static>>>();}"),
 ('scratch-no-mut-vec',False,'fn f(mut s:jzap::scratch::ScratchBuf<u8>){s.values.try_reserve(99);}'),
 ('scratch-no-reserve',False,'fn f(mut s:jzap::scratch::ScratchBuf<u8>){s.try_reserve(99);}'),
 ('scratch-no-implicit-clone',False,'fn f(s:jzap::scratch::ScratchBuf<u8>){let _=s.clone();}'),
 ('scratch-no-byte-spare-escape',False,'fn f(mut s:jzap::scratch::Scratch<u8>){let _=s.spare_capacity_mut();}'),
 ('scratch-backing-borrow',False,'fn f(){let mut a=[0u8;4];let s=jzap::scratch::Scratch::from_bytes(&mut a);a[0]=1;std::hint::black_box(s);}'),
 ('scratch-no-borrowed-clone',False,'fn f(s:jzap::scratch::Scratch<u8>){let _=s.clone();}'),
 ('slice-no-index',False,'fn f(s:jzap::slice::Slice<u8>){let _=s[0];}'),
 ('slice-no-deref',False,'fn f(s:jzap::slice::Slice<u8>){let _: &[u8]= &s;}'),
 ('slice-no-standard-iterator',False,'fn f(s:jzap::slice::Slice<u8>){let _=s.iter().next();}'),
 ('slice-mut-no-clone',False,'fn f(s:jzap::slice::SliceMut<u8>){let _=s.clone();}'),
 ('slice-no-borrow-escape',False,"fn f()->jzap::slice::Slice<'static,u8>{let a=[1];jzap::slice::Slice::new(&a)}"),
 ('slice-mut-exclusive',False,'fn f(mut s:jzap::slice::SliceMut<u8>){let a=s.get_mut(0).unwrap();let b=s.get_mut(1).unwrap();std::hint::black_box((a,b));}'),
 ('slice-split-disjoint',True,'fn f(mut s:jzap::slice::SliceMut<u8>){let(mut a,mut b)=s.split_at_mut(1).unwrap();std::hint::black_box((a.get_mut(0),b.get_mut(0)));}'),

 ('hash-set-owned-concurrent-static', True, "fn f(){fn both<T:Send+Sync+'static>(){}both::<jzap::collections::HashSet<u64,jzap::collections::hash_map::RandomState,jzap::budget::OwnedConcurrentBudget>>();}"),
 ('hash-set-borrowed-domain-escape', False, "fn f()->jzap::collections::HashSet<u64,jzap::collections::hash_map::RandomState,jzap::budget::Budget<'static>>{let b=jzap::budget::MemoryBudget::new(99);jzap::collections::HashSet::with_hasher_in(jzap::collections::hash_map::RandomState::from_seed(0,0),jzap::budget::Budget::Limited(&b))}"),
 ('hash-set-no-default', False, 'fn f(){let _=jzap::collections::HashSet::<u64>::default();}'),
 ('hash-set-no-clone', False, 'fn f(s:jzap::collections::HashSet<u64>){let _=s.clone();}'),
 ('hash-set-no-implicit-collect', False, 'fn f(){let _:jzap::collections::HashSet<u64>=[1u64].into_iter().collect();}'),
 ('hash-set-no-mutable-key', False, 'fn f(mut s:jzap::collections::HashSet<u64>){for k in &mut s{*k=99;}}'),
 ('hash-set-key-loan-blocks-replace', False, 'fn f(mut s:jzap::collections::HashSet<u64>){let k=s.get(&1);let _=s.try_replace(1);std::hint::black_box(k);}'),
 ('hash-set-iter-key-sync', False, "fn f(){fn send<T:Send>(){}send::<jzap::collections::hash_set::Iter<'static,std::cell::Cell<u64>>>();}"),
 ('hash-set-drain-builder-not-send', True, "fn f(){fn send<T:Send>(){}send::<jzap::collections::hash_set::Drain<'static,u64,std::rc::Rc<()>>>();}"),
 ('hash-set-algebra-builder-sync', False, "fn f(){fn send<T:Send>(){}send::<jzap::collections::hash_set::Difference<'static,u64,std::rc::Rc<()>>>();}"),

 ('hash-mut-iterator-key-send',True,"fn f(){fn send<T:Send>(){}send::<jzap::collections::hash_map::IterMut<'static,std::cell::Cell<u64>,u64>>();}"),
 ('hash-extract-send-sync',True,"fn f(){fn both<T:Send+Sync>(){}both::<jzap::collections::hash_map::ExtractIf<'static,u64,u64,std::rc::Rc<()>,jzap::budget::Unmetered,fn(&u64,&mut u64)->Result<bool,()>>>();}"),
 ('hash-extract-key-send',False,"fn f(){fn send<T:Send>(){}send::<jzap::collections::hash_map::ExtractIf<'static,std::rc::Rc<u64>,u64,(),jzap::budget::Unmetered,fn(&std::rc::Rc<u64>,&mut u64)->Result<bool,()>>>();}"),
 ('hash-extract-predicate-send',False,"fn f(){fn send<T:Send>(){}send::<jzap::collections::hash_map::ExtractIf<'static,u64,u64,(),jzap::budget::Unmetered,std::rc::Rc<()>>>();}"),
    ('hash-copy-witness-default-noncopy',False,'fn f(){let _:jzap::CopyWitness<String>=Default::default();}'),
    ('hash-drain-hasher-not-send',True,"fn f(){fn send<T:Send>(){}send::<jzap::collections::hash_map::Drain<'static,u64,u64,std::rc::Rc<()>>>();}"),
    ('array-copy-witness-positive',True,'fn f(){let _:jzap::CopyWitness<[u64;32]>=jzap::CopyWitness::<u64>::new().for_array();}'),
    ('array-copy-witness-noncopy',False,'fn f(){let _=jzap::CopyWitness::<String>::new().for_array::<4>();}'),
    ('hash-copy-witness-positive',True,'fn f(){let _=jzap::CopyWitness::<u64>::new();let _:jzap::CopyWitness<u64>=Default::default();}'),
    ('hash-copy-witness-noncopy',False,'fn f(){let _=jzap::CopyWitness::<String>::new();}'),
    ('hash-copy-witness-private',False,'fn f(){let _:jzap::CopyWitness<String>=jzap::CopyWitness{marker:std::marker::PhantomData};}'),
    ('hash-send-sync', True, 'fn f(){fn both<T:Send+Sync>(){}both::<jzap::collections::HashMap<u64,u64>>();}'),
    ('hash-owned-concurrent-static', True, "fn f(){fn both<T:Send+Sync+'static>(){}both::<jzap::collections::HashMap<u64,u64,jzap::collections::hash_map::RandomState,jzap::budget::OwnedConcurrentBudget>>();}"),
    ('hash-local-send', False, "fn f(){fn send<T:Send>(){}send::<jzap::collections::HashMap<u64,u64,jzap::collections::hash_map::RandomState,jzap::budget::Budget<'static>>>();}"),
    ('hash-key-sync', False, "fn f(){fn send<T:Send>(){}send::<jzap::collections::hash_map::Iter<'static,std::cell::Cell<u64>,u64>>();}"),
    ('hash-owned-key-send', False, 'fn f(){fn send<T:Send>(){}send::<jzap::collections::hash_map::IntoIter<std::rc::Rc<u64>,u64>>();}'),
    ('hash-borrowed-policy', False, "fn f()->jzap::collections::HashMap<u64,u64,jzap::collections::hash_map::RandomState,jzap::budget::Budget<'static>>{let b=jzap::budget::MemoryBudget::new(100);jzap::collections::HashMap::with_hasher_in(jzap::collections::hash_map::RandomState::from_seed(0,0),jzap::budget::Budget::Limited(&b))}"),
    ('hash-no-clone', False, 'fn f(m:jzap::collections::HashMap<u64,u64>){let _=m.clone();}'),
    ('hash-no-default', False, 'fn f(){let _:jzap::collections::HashMap<u64,u64>=Default::default();}'),
    ('hash-no-index', False, 'fn f(m:jzap::collections::HashMap<u64,u64>){let _=m[&0];}'),
    ('hash-iterator-exclusive', False, 'fn f(mut m:jzap::collections::HashMap<u64,u64>){let mut i=m.iter_mut();m.clear();std::hint::black_box(i.next());}'),
    ('hash-borrowed-key', True, 'fn f(m:jzap::collections::HashMap<String,u64>){let _=m.get("key");}'),
    ('hash-entry-key-immutable', False, 'fn f(mut m:jzap::collections::HashMap<u64,u64>){if let Some((k,_))=m.iter_mut().next(){*k=2;}}'),
 ('set-send-sync',True,'fn f(){fn both<T:Send+Sync>(){} both::<jzap::collections::BTreeSet<u64>>();}'),
 ('set-owned-concurrent-static',True,'fn f(){fn both<T:Send+Sync+\'static>(){} both::<jzap::collections::BTreeSet<u64,jzap::budget::OwnedConcurrentBudget>>();}'),
 ('set-owned-local-send',False,'fn f(){fn send<T:Send>(){} send::<jzap::collections::BTreeSet<u64,jzap::budget::OwnedBudget>>();}'),
 ('set-local-policy-send',False,"fn f(){fn send<T:Send>(){} send::<jzap::collections::BTreeSet<u64,jzap::budget::Budget<'static>>>();}"),
 ('set-key-send',False,'fn f(){fn send<T:Send>(){} send::<jzap::collections::BTreeSet<std::rc::Rc<u64>>>();}'),
 ('set-intoiter-key-send',False,'fn f(){fn send<T:Send>(){} send::<jzap::collections::btree_set::IntoIter<std::rc::Rc<u64>>>();}'),
 ('set-iter-key-sync',False,"fn f(){fn send<T:Send>(){} send::<jzap::collections::btree_set::Iter<'static,std::cell::Cell<u64>>>();}"),
 ('set-no-clone',False,'fn f(s:jzap::collections::BTreeSet<u64>){let _=s.clone();}'),
 ('set-no-implicit-collect',False,'fn f(){let _:jzap::collections::BTreeSet<u64>=[1,2].into_iter().collect();}'),
 ('set-no-implicit-extend',False,'fn f(mut s:jzap::collections::BTreeSet<u64>){s.extend([1,2]);}'),
 ('set-no-allocating-operator',False,'fn f(a:jzap::collections::BTreeSet<u64>,b:jzap::collections::BTreeSet<u64>){let _=&a|&b;}'),
 ('set-no-mutable-key',False,'fn f(mut s:jzap::collections::BTreeSet<u64>){if let Some(k)=(&mut s).into_iter().next(){*k=7;}}'),
 ('set-iterator-freezes-owner',False,'fn f(mut s:jzap::collections::BTreeSet<u64>){let i=s.iter();s.clear();std::hint::black_box(i);}'),
 ('set-algebra-freezes-owner',False,'fn f(mut a:jzap::collections::BTreeSet<u64>,b:jzap::collections::BTreeSet<u64>){let i=a.union(&b);a.clear();std::hint::black_box(i);}'),
 ('set-borrowed-policy-lifetime',False,"fn f()->jzap::collections::BTreeSet<u64,jzap::budget::Budget<'static>>{let b=jzap::budget::MemoryBudget::new(4096);jzap::collections::BTreeSet::new_in(jzap::budget::Budget::Limited(&b))}"),
 ('set-heterogeneous-range',True,'fn f(s:jzap::collections::BTreeSet<String>){let _=s.get("query");let _=s.try_range::<str,_>((core::ops::Bound::Included("a"),core::ops::Bound::Excluded("z")));}'),
 ('set-no-payload-clone-bridge',False,'use jzap::TryClone;fn f(s:jzap::collections::BTreeSet<String>){let _=s.try_clone();}'),
 ('map-send-sync',True,'fn f(){fn both<T:Send+Sync>(){} both::<jzap::collections::BTreeMap<u64,u64>>();}'),
 ('concurrent-policy',True,"fn f(){fn both<T:Send+Sync>(){} both::<jzap::collections::BTreeMap<u64,u64,jzap::budget::ConcurrentBudget<'static>>>();}"),
 ('local-policy-send',False,"fn f(){fn send<T:Send>(){} send::<jzap::collections::BTreeMap<u64,u64,jzap::budget::Budget<'static>>>();}"),
 ('local-policy-sync',False,"fn f(){fn sync<T:Sync>(){} sync::<jzap::collections::BTreeMap<u64,u64,jzap::budget::Budget<'static>>>();}"),
 ('rc-map-send',False,'fn f(){fn send<T:Send>(){} send::<jzap::collections::BTreeMap<u64,std::rc::Rc<()>>>();}'),
 ('rc-intoiter-send',False,'fn f(){fn send<T:Send>(){} send::<jzap::collections::btree_map::IntoIter<u64,std::rc::Rc<()>>>();}'),
 ('mutable-iterator-send',True,"fn f(){fn send<T:Send>(){} send::<jzap::collections::btree_map::IterMut<'static,std::cell::Cell<u64>,u64>>();}"),
 ('immutable-iterator-sync-bound',False,"fn f(){fn send<T:Send>(){} send::<jzap::collections::btree_map::Iter<'static,std::cell::Cell<u64>,u64>>();}"),
 ('entry-send',True,"fn f(){fn send<T:Send>(){} send::<jzap::collections::btree_map::OccupiedEntry<'static,u64,u64>>();send::<jzap::collections::btree_map::VacantEntry<'static,u64,u64>>();}"),
 ('borrowed-string-key-extract-inference',True,'fn f(mut m:jzap::collections::BTreeMap<String,u64>){let _=m.get("query");let _=m.try_extract_if(..,|_,_|Ok::<_,()>(false));}'),
 ('query-lifetime-independent-of-node',True,"fn f<'a>(node:jzap::collections::btree_map::view::NodeSlice<'a,String,u64>)->Option<&'a u64>{let query=String::from(\"key\");let mut lookup=jzap::collections::btree_map::view::Lookup::new(0,0,query.as_str()).unwrap();match lookup.provide(0,node).unwrap(){jzap::collections::btree_map::view::LookupStep::Found(_,v)=>Some(v),_=>None}}"),
 ('no-standard-clone',False,'fn f(m:jzap::collections::BTreeMap<u64,u64>){let _=m.clone();}'),
 ('no-index',False,'fn f(m:jzap::collections::BTreeMap<u64,u64>){let _=m[&0];}'),
 ('no-implicit-collect',False,'fn f(){let _:jzap::collections::BTreeMap<u64,u64>=[(1,2)].into_iter().collect();}'),
 ('no-implicit-extend',False,'fn f(mut m:jzap::collections::BTreeMap<u64,u64>){m.extend([(1,2)]);}'),
 ('no-blanket-payload-clone',False,'use jzap::TryClone;fn f(m:jzap::collections::BTreeMap<String,String>){let _=m.try_clone();}'),
 ('external-slice-lifetime',False,"fn f()->jzap::collections::btree_map::view::NodeSlice<'static,u64,u64>{let a=[1];jzap::collections::btree_map::view::NodeSlice::new(&a,&a,None).unwrap()}"),
 ('owned-view-lifetime',False,"fn f()->jzap::collections::btree_map::view::NodeView<'static,u64,u64>{let mut m=jzap::collections::BTreeMap::new();m.try_insert(1,2).unwrap();m.root_view().unwrap()}"),
 ('view-freezes-map',False,'fn f(mut m:jzap::collections::BTreeMap<u64,u64>){let v=m.root_view();m.clear();std::hint::black_box(v);}'),
 ('iterator-freezes-map',False,'fn f(mut m:jzap::collections::BTreeMap<u64,u64>){let mut it=m.iter_mut();m.clear();std::hint::black_box(it.next());}'),
 ('private-metadata',False,'fn f(mut m:jzap::collections::BTreeMap<u64,u64>){m.len=jzap::Checked::new(usize::MAX);}'),
 ('fallible-source',True,'fn f()->Result<jzap::collections::BTreeMap<u64,u64>,jzap::collections::btree_map::ExtendError<u64,u64,core::convert::Infallible>>{jzap::collections::BTreeMap::try_from_array([(1,2)])}'),
]
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--mutations',action='store_true');args=parser.parse_args()
    out=ROOT/'target/verification/btree/compiler';out.mkdir(parents=True,exist_ok=True)
    subprocess.run(['cargo','build','--locked','--offline','--all-features'],cwd=ROOT,check=True)
    library=ROOT/'target/debug/libjzap.rlib';rows=[]
    for name,positive,source in CASES:
        file=out/(name+'.rs');file.write_text(source)
        command=['rustc','--edition=2024','--crate-type=lib','--crate-name=btree_boundary','--extern','jzap='+str(library),'-L','dependency='+str(ROOT/'target/debug/deps'),str(file),'-o',str(out/(name+'.rlib'))]
        result=subprocess.run(command,cwd=ROOT,capture_output=True,text=True);(out/(name+'.log')).write_text(result.stdout+result.stderr)
        assert (result.returncode==0)==positive,(name,result.stderr)
        if not positive:assert 'error[E' in result.stderr and not any(x in result.stderr for x in ['E0603','E0433','E0463']),(name,result.stderr)
        rows.append(dict(name=name,positive=positive,exit_code=result.returncode))
    mutations=[]
    if args.mutations:
        from arithmetic_projection import replace_once
        for name,file,old,new,case in [
            ('map-value-send','src/collections/btree_map.rs','unsafe impl<K: Send, V: Send, P: AllocationPolicy + Send> Send','unsafe impl<K: Send, V, P: AllocationPolicy + Send> Send','rc-map-send'),
            ('map-policy-send','src/collections/btree_map.rs','unsafe impl<K: Send, V: Send, P: AllocationPolicy + Send> Send','unsafe impl<K: Send, V: Send, P: AllocationPolicy> Send','local-policy-send'),
            ('intoiter-value-send','src/collections/btree_map/iter.rs','unsafe impl<K: Send, V: Send, P: AllocationPolicy + Send> Send','unsafe impl<K: Send, V, P: AllocationPolicy + Send> Send','rc-intoiter-send'),
            ('iter-key-sync','src/collections/btree_map/iter.rs',"unsafe impl<K: Sync, V: Sync> Send for Iter<'_, K, V>","unsafe impl<K, V: Sync> Send for Iter<'_, K, V>",'immutable-iterator-sync-bound'),
        ]:
            with tempfile.TemporaryDirectory(prefix='jzap-btree-bounds-') as tmp:
                root=Path(tmp);shutil.copytree(ROOT/'src',root/'src');path=root/file;path.write_text(replace_once(path.read_text(),old,new));lib=root/'libjzap.rlib'
                built=subprocess.run(['rustc','--edition=2024','--crate-type=rlib','--crate-name=jzap','--cfg=feature="alloc"',str(root/'src/lib.rs'),'-o',str(lib)],capture_output=True,text=True);assert built.returncode==0,built.stderr
                text=next(s for n,_,s in CASES if n==case);probe=root/'probe.rs';probe.write_text(text)
                checked=subprocess.run(['rustc','--edition=2024','--crate-type=lib','--extern','jzap='+str(lib),str(probe),'-o',str(root/'probe.rlib')],capture_output=True,text=True)
                assert checked.returncode==0,(name,checked.stderr)
                mutations.append(dict(name=name,compiled=True,missing_negative_boundary_detected=True))
    (out/'summary.json').write_text(json.dumps(dict(status='pass',cases=rows,mutations=mutations,library_sha256=hashlib.sha256(library.read_bytes()).hexdigest()),indent=2)+'\n')
    print(len(rows),'ordered-map/compiler boundaries checked;',len(mutations),'weakened bounds detected')
if __name__=='__main__':main()
