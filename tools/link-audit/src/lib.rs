//! Concrete Vec/Box link roots, including initialization and ownership adapters.
//! Generic payload callbacks remain separate verification obligations.
#![no_std]
extern crate alloc;
use core::{
    alloc::{GlobalAlloc, Layout},
    hint::black_box,
};
use jzap::{
    Checked, Error, TryClone,
    boxed::Box,
    budget::{AllocationPolicy, Budget, MemoryBudget},
    vec::{LengthPolicy, Vec},
};

unsafe extern "C" {
    fn audit_alloc(size: usize, align: usize) -> *mut u8;
    fn audit_free(ptr: *mut u8);
    fn audit_realloc(ptr: *mut u8, old_size: usize, align: usize, size: usize) -> *mut u8;
    fn jzap_link_audit_panic_reachable() -> !;
}
struct Allocator;
// SAFETY: the C fixture honors allocation alignment, ownership and null refusal.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        unsafe { audit_alloc(l.size(), l.align()) }
    }
    unsafe fn dealloc(&self, p: *mut u8, _: Layout) {
        unsafe { audit_free(p) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        unsafe { audit_realloc(p, l.size(), l.align(), n) }
    }
}
#[global_allocator]
static A: Allocator = Allocator;

#[unsafe(no_mangle)]
pub extern "C" fn audit_owned(seed:usize,limit:usize)->u32 {
    use jzap::budget::{OwnedBudget,OwnedConcurrentBudget};
    let result=(|| {
        let local=OwnedBudget::try_new(limit)?;
        black_box((local.limit(),local.available(),local.ledger()));
        drop(local.reserve(seed%17)?);
        box_operations(seed,local.try_clone()?)?;
        vec_operations(seed,Vec::new_in(local.try_clone()?))?;
        string_operations(seed,jzap::string::String::new_in(local.try_clone()?))?;
        btree_operations(seed,local.try_clone()?)?;
        shared_operations::<jzap::shared::count::LocalCount,_>(seed,local.try_clone()?)?;
        if local.occupied()!=0||local.reserved()!=0{return Err(Error::InvalidStructure)}
        let concurrent=OwnedConcurrentBudget::try_new(limit)?;
        black_box((concurrent.limit(),concurrent.available(),concurrent.ledger()));
        drop(concurrent.reserve(seed%17)?);
        box_operations(seed,concurrent.try_clone()?)?;
        vec_operations(seed,Vec::new_in(concurrent.try_clone()?))?;
        string_operations(seed,jzap::string::String::new_in(concurrent.try_clone()?))?;
        btree_operations(seed,concurrent.try_clone()?)?;
        shared_operations::<jzap::shared::count::AtomicCount,_>(seed,concurrent.try_clone()?)?;
        if concurrent.used()!=0{return Err(Error::InvalidStructure)}
        Ok::<_,Error>(())
    })();
    u32::from(result.is_err())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_btree_set(seed:usize,budget_limit:usize)->u32 {
    let local=MemoryBudget::new(budget_limit);
    let concurrent=jzap::budget::ConcurrentMemoryBudget::new(budget_limit);
    let result=(||{
        btree_set_operations(seed,jzap::budget::Unmetered)?;
        btree_set_operations(seed,Budget::Limited(&local))?;
        btree_set_operations(seed,jzap::budget::ConcurrentBudget(&concurrent))?;
        let owned=jzap::budget::OwnedBudget::try_new(budget_limit)?;
        btree_set_operations(seed,owned.try_clone()?)?;
        if owned.occupied()!=0||owned.reserved()!=0{return Err(Error::InvalidStructure)}
        let owned=jzap::budget::OwnedConcurrentBudget::try_new(budget_limit)?;
        btree_set_operations(seed,owned.try_clone()?)?;
        if owned.used()!=0{return Err(Error::InvalidStructure)}
        Ok::<_,Error>(())
    })();
    if local.occupied()!=0||local.reserved()!=0||concurrent.used()!=0{return 2}
    u32::from(result.is_err())
}
#[inline(never)]
fn btree_set_operations<P:AllocationPolicy>(seed:usize,policy:P)->Result<(),Error>{
    use jzap::{collections::BTreeSet,TryIterator};
    let mut a=BTreeSet::new_in(policy.try_clone()?);
    let mut b=BTreeSet::new_in(policy.try_clone()?);
    for i in 0..32u64{
        a.try_insert(black_box(i.wrapping_mul(7)%32)).map_err(|e|e.error)?;
        b.try_insert(black_box(i.wrapping_add(16))).map_err(|e|e.error)?;
    }
    black_box(a.try_insert(black_box(7)).map_err(|e|e.error)?);
    black_box(a.try_replace(black_box(7)).map_err(|e|e.error)?);
    black_box(a.try_replace(black_box(99)).map_err(|e|e.error)?);
    black_box(a.contains(black_box(&(seed as u64))));black_box(a.get(black_box(&7)));
    black_box((a.first(),a.last(),a.len(),a.is_empty()));
    let mut range=a.try_range(black_box(2)..black_box(20))?;
    black_box(range.next());black_box(range.next_back());drop(range);
    let mut iter=a.iter();black_box(iter.next());black_box(iter.next_back());black_box(iter.clone().size_hint());drop(iter);
    for key in a.union(&b){black_box(key);}for key in a.intersection(&b){black_box(key);}
    for key in a.difference(&b){black_box(key);}for key in a.symmetric_difference(&b){black_box(key);}
    black_box(a.union(&b).size_hint());black_box(a.symmetric_difference(&b).size_hint());
    black_box((a.is_subset(&b),a.is_superset(&b),a.is_disjoint(&b)));
    drop(a.try_union(&b)?);drop(a.try_intersection(&b)?);drop(a.try_difference(&b)?);drop(a.try_symmetric_difference(&b)?);
    let mut copy=a.try_clone()?;copy.try_clone_from(&b)?;
    let mut tail=copy.try_split_off(black_box(&24))?;copy.try_append(&mut tail)?;
    copy.try_retain(|k|Ok::<_,Error>(*k%3!=0))?;
    let mut extraction=copy.try_extract_if(..,|k|Ok::<_,Error>(*k%5==0))?;
    while let Some(k)=extraction.try_next()?{black_box(k);}drop(extraction);
    black_box(copy.pop_first());black_box(copy.pop_last());black_box(copy.take(black_box(&7)));black_box(copy.remove(black_box(&8)));
    let mut owned=copy.into_iter();black_box(owned.next());black_box(owned.next_back());black_box(owned.as_iter().next());drop(owned);
    let mut source=BTreeSet::try_from_iter_in([1u64,2,3],policy).map_err(|e|match e{jzap::collections::btree_set::ExtendError::Source(e)=>match e{},jzap::collections::btree_set::ExtendError::Insert(e)=>e.error})?;
    source.try_extend([4,5]).map_err(|e|match e{jzap::collections::btree_set::ExtendError::Source(e)=>match e{},jzap::collections::btree_set::ExtendError::Insert(e)=>e.error})?;
    struct Sink;
    impl core::fmt::Write for Sink{fn write_str(&mut self,_:&str)->core::fmt::Result{Ok(())}}
    impl core::hash::Hasher for Sink{fn finish(&self)->u64{0}fn write(&mut self,s:&[u8]){black_box(s);}}
    core::fmt::write(&mut Sink,format_args!("{:?}",a)).map_err(|_|Error::InvalidStructure)?;
    core::hash::Hash::hash(&a,&mut Sink);black_box(a.cmp(&b));black_box(a==b);
    a.clear();b.clear();source.clear();
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_btree(seed:usize,budget_limit:usize)->u32 {
    let local=MemoryBudget::new(budget_limit);
    let concurrent=jzap::budget::ConcurrentMemoryBudget::new(budget_limit);
    let result=(||{
        bplus_lookup(seed)?;
        btree_operations(seed,jzap::budget::Unmetered)?;
        btree_operations(seed,Budget::Limited(&local))?;
        btree_operations(seed,jzap::budget::ConcurrentBudget(&concurrent))
    })();
    if local.occupied()!=0||local.reserved()!=0||concurrent.used()!=0{return 2}
    u32::from(result.is_err())
}
#[inline(never)]
fn bplus_lookup(seed:usize)->Result<(),Error>{
    use jzap::collections::btree_map::view::{LookupStep,bplus::{Lookup,Node}};
    let query=black_box(seed as u64);
    let minima=black_box([3u64,9]);
    let edges=black_box([1usize,2]);
    let mut lookup=Lookup::new(0,1,&query)?;
    let branch=Node::<_,u64>::branch(&minima,&edges)?;
    if let LookupStep::NeedNode(id)=lookup.provide(0,&branch)?{
        let keys=if id==1{black_box([3u64,7])}else{black_box([9u64,17])};
        let values=black_box([19u64,35]);
        let leaf=Node::leaf(&keys,&values)?;
        black_box(lookup.provide(id,&leaf)?);
    }
    Ok(())
}
#[inline(never)]
fn btree_operations<P:AllocationPolicy>(seed:usize,policy:P)->Result<(),Error>{
    use jzap::{collections::{BTreeMap,btree_map::view::{NodeSlice,Lookup,LookupStep,Traversal,Visit}},TryIterator};
    let mut map=BTreeMap::new_in(policy.try_clone()?);
    for i in 0..128usize {let k=black_box(i.wrapping_mul(37)%128);map.try_insert(k as u64,DropValue(i as u64)).map_err(|e|e.error)?;}
    black_box(map.get(black_box(&(seed as u64))));black_box(map.get_mut(black_box(&17)));
    black_box(map.first_key_value());black_box(map.last_key_value());
    if let Some(view)=map.root_view(){black_box(view.keys());black_box(view.values());let _=black_box(view.child(black_box(seed%16)));}
    for (_,v) in map.try_range_mut(black_box(10)..black_box(90))? {v.0=v.0.wrapping_add(1);}
    black_box(map.try_range(black_box(9)..black_box(120))?.next_back());
    let mut range=map.iter_mut();black_box(range.next());black_box(range.next_back());drop(range);
    black_box(map.entry(black_box(181)).try_or_insert(DropValue(99)).map_err(|e|e.error)?);
    let mut copy=map.try_clone()?;let mut tail=copy.try_split_off(black_box(&51))?;
    copy.try_append(&mut tail)?;
    copy.try_retain(|k,v|{v.0=v.0.wrapping_add(1);Ok::<_,Error>(*k%7!=0)})?;
    let mut extract=copy.try_extract_if(..,|k,v|{v.0=v.0.wrapping_add(1);Ok::<_,Error>(*k%3==0)})?;
    while let Some(pair)=extract.try_next()?{black_box(pair);}drop(extract);
    black_box(copy.pop_first());black_box(copy.pop_last());black_box(copy.remove_entry(black_box(&13)));
    if let Some(entry)=copy.first_entry(){black_box(entry.remove_entry());}
    let mut iter=copy.into_iter();black_box(iter.next());black_box(iter.next_back());black_box(iter.as_iter().next());drop(iter);
    map.clear();
    // Traits and adapters are consumer roots too: exercise them with returning,
    // allocation-free scalar callbacks and a fixed formatter/hash sink.
    struct Sink(u64);
    impl core::fmt::Write for Sink {
        fn write_str(&mut self,s:&str)->core::fmt::Result {self.0=self.0.wrapping_add(s.len() as u64);Ok(())}
    }
    impl core::hash::Hasher for Sink {
        fn finish(&self)->u64{self.0}
        fn write(&mut self,bytes:&[u8]){for b in bytes {self.0=self.0.wrapping_mul(31).wrapping_add(u64::from(*b));}}
    }
    let mut scalar=BTreeMap::new_in(policy.try_clone()?);
    scalar.try_insert(black_box(seed as u64),black_box(7u64)).map_err(|e|e.error)?;
    let copy=scalar.try_clone()?;
    black_box(scalar==copy);black_box(scalar.cmp(&copy));
    let mut sink=Sink(0);core::hash::Hash::hash(&scalar,&mut sink);
    use core::fmt::Write;
    write!(&mut sink,"{:?}{:?}{:?}{:?}{:?}{:?}",scalar,scalar.keys(),scalar.values(),scalar.iter(),scalar.try_range(..)?,scalar.root_view().map(|v|v.height())).map_err(|_|Error::InvalidStructure)?;
    write!(&mut sink,"{:?}",scalar.values_mut()).map_err(|_|Error::InvalidStructure)?;
    write!(&mut sink,"{:?}",scalar.try_range_mut(..)?).map_err(|_|Error::InvalidStructure)?;
    write!(&mut sink,"{:?}",scalar.entry(black_box(seed as u64))).map_err(|_|Error::InvalidStructure)?;
    let mut owned=copy.into_iter();write!(&mut sink,"{:?}",owned).map_err(|_|Error::InvalidStructure)?;
    black_box(owned.try_next().map_err(|x|match x{})?);
    black_box(sink.0);
    let keys=black_box([3u64,9,17]);let vals=black_box([7u64,19,35]);
    let node=NodeSlice::<_,_,usize>::new(&keys,&vals,None)?;let query=black_box(seed as u64);
    let mut lookup=Lookup::new(0,0,&query)?;
    match lookup.provide(0,node)?{LookupStep::Found(k,v)=>{black_box((k,v));},LookupStep::Absent=>{},LookupStep::NeedNode(_)=>return Err(Error::InvalidStructure)}
    let mut traversal=Traversal::new(0,0)?;traversal.provide(0,node)?;
    loop{match traversal.advance()?{Visit::Item(k,v)=>{black_box((k,v));},Visit::Done=>break,Visit::NeedNode(_)=>return Err(Error::InvalidStructure)}}
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_string(seed: usize, budget_limit: usize) -> u32 {
    use jzap::string::String;
    let budget = MemoryBudget::new(budget_limit);
    let outcome = (|| {
        string_operations(seed, String::new())?;
        string_operations(seed, String::with_limit(1024))?;
        string_operations(seed, String::new_in(Budget::Limited(&budget)))?;
        string_operations(seed, String::with_limit_in(1024, Budget::Limited(&budget)))
    })();
    if budget.occupied() != 0 || budget.reserved() != 0 {
        return 3;
    }
    if outcome.is_ok() { 0 } else { 1 }
}
#[inline(never)]
fn string_operations<P: AllocationPolicy, L: LengthPolicy>(
    seed: usize,
    mut text: jzap::string::String<P, L>,
) -> Result<(), Error> {
    let policy = text.allocation_policy().try_clone()?;
    let limit = text.length_policy();
    let invalid = black_box([0xffu8, 0xe2, 0x82, b'a']);
    black_box(jzap::string::String::try_from_utf8_lossy_in(
        &invalid, policy.try_clone()?, limit,
    )?);
    let units = black_box([0x61u16, 0xd83d, 0xde00]);
    black_box(
        jzap::string::String::try_from_utf16_in(&units, policy.try_clone()?, limit).map_err(|e| match e {
            jzap::string::Utf16Error::Storage(e) => e,
            _ => Error::InvalidStructure,
        })?,
    );
    black_box(jzap::string::String::try_from_utf16_lossy_in(
        &units, policy.try_clone()?, limit,
    )?);
    text.try_push_str(black_box("aé한😀z"))?;
    text.try_push(black_box('😀'))?;
    text.try_insert_str(black_box(1), black_box("hello"))?;
    text.try_replace_range(black_box(1)..black_box(6), "한")?;
    black_box(text.remove(black_box(1))?);
    text.try_extend_from_within(black_box(0)..black_box(1))?;
    text.retain(|ch| black_box(ch) != 'z');
    let _: Result<(), ()> = text.try_retain(|ch| Ok(black_box(ch) != 'é'));
    let mut tail = text.try_split_off(black_box(1))?;
    text.try_append(&mut tail)?;
    black_box(text.try_drain(black_box(0)..black_box(1))?.next_back());
    text.try_write_fmt(format_args!("{}:{}", black_box(seed), black_box("한")))
        .map_err(|e| match e {
            jzap::string::FormatError::Storage(e) => e,
            _ => Error::InvalidStructure,
        })?;
    text.try_extend_chars(black_box(['a', 'é', '한']))?;
    text.try_extend_texts(black_box(["a", "한"]))?;
    black_box(text.pop());
    text.truncate(black_box(0))?;
    text.try_reserve(black_box(48))?;
    text.try_reserve_exact(black_box(96))?;
    text.try_push_str("aé")?;
    text.try_shrink_to(black_box(8))?;
    text.try_shrink_to_fit()?;
    let copy = text.try_clone()?;
    text.try_clone_from(&copy)?;
    black_box(text.get(black_box(0)..black_box(1))?);
    black_box(text.get_mut(black_box(0)..black_box(1))?);
    let _=black_box(text.chars());
    let _=black_box(text.char_indices());
    let boxed = text.try_into_boxed_str().map_err(|e| e.error)?;
    let text = jzap::string::String::from_boxed_str(boxed);
    let bytes = text.into_bytes();
    let text = jzap::string::String::try_from_utf8(bytes).map_err(|_| Error::InvalidStructure)?;
    let std = text.into_std();
    drop(jzap::string::String::try_from_std_in(
        std,
        jzap::budget::Unmetered,
        jzap::vec::Unlimited,
    ));
    Ok(())
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    // Intentionally undefined. A retained panic edge must fail the final link.
    unsafe { jzap_link_audit_panic_reachable() }
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_vec(seed: usize, budget_limit: usize) -> u32 {
    let budget = MemoryBudget::new(budget_limit);
    let outcome = (|| {
        vec_operations(seed, Vec::new())?;
        vec_operations(seed, Vec::with_limit(256))?;
        vec_operations(seed, Vec::new_in(Budget::Limited(&budget)))?;
        vec_operations(seed, Vec::with_limit_in(256, Budget::Limited(&budget)))
    })();
    if budget.occupied() != 0 || budget.reserved() != 0 {
        return 3;
    }
    match outcome {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
#[inline(never)]
fn vec_operations<P: AllocationPolicy, L: LengthPolicy>(
    seed: usize,
    mut v: Vec<u64, P, L>,
) -> Result<(), Error> {
    let n = seed % 65;
    v.try_reserve_exact(n)?;
    for i in 0..n {
        v.try_push(i as u64).map_err(|e| e.error)?;
    }
    v.try_insert(n / 2, 99).map_err(|e| e.error)?;
    black_box(v.remove(n / 2)?);
    let resized = (Checked::new(n) + Checked::new(7))
        .map_err(|_| Error::Capacity)?
        .get();
    v.try_resize(resized, &17)?;
    v.try_extend_copy(&[3, 5, 8])?;
    v.try_extend_from_within(..3)?;
    let mut tail = v.try_split_off(n / 2)?;
    v.try_append(&mut tail)?;
    v.try_retain(|x| Ok::<_, Error>(*x % 3 != 0))?;
    v.try_dedup_by(|a, b| Ok::<_, Error>(*a == *b))?;
    for x in v.try_extract_if(.., |x| Ok::<_, Error>(*x % 2 == 0))? {
        black_box(x?);
    }
    v.try_splice(..0, [7, 11])?.finish()?;
    black_box(v.try_drain(..1)?);
    v.try_shrink_to_fit()?;
    let copy = v.try_clone()?;
    black_box(copy.try_into_boxed_slice().map_err(|e| e.error)?);
    // Dynamic rejection inputs are kept opaque to the optimizer.
    if black_box(v.get(black_box(usize::MAX))).is_ok()
        || black_box(v.try_reserve(black_box(usize::MAX))).is_ok()
        || black_box(v.try_retain(|_| Err::<bool, _>(Error::Bounds))).is_ok()
    {
        return Err(Error::Bounds);
    }
    for value in v {
        black_box(value);
    }
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_box(seed: usize, budget_limit: usize) -> u32 {
    let budget = MemoryBudget::new(budget_limit);
    let outcome = (|| {
        box_operations(seed, jzap::budget::Unmetered)?;
        box_operations(seed, Budget::Limited(&budget))
    })();
    if budget.occupied() != 0 || budget.reserved() != 0 {
        return 3;
    }
    match outcome {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
#[inline(never)]
fn box_operations<P: AllocationPolicy>(seed: usize, policy: P) -> Result<(), Error> {
    let b = Box::try_new_in(seed, policy.try_clone()?).map_err(|e| e.error)?;
    black_box(b.try_clone()?);
    let text = Box::<str, _>::try_from_str_in("fallible allocation", policy.try_clone()?)?;
    black_box(text.try_clone()?);
    let n = black_box(seed % 65);
    let slice = Box::<[u64], _>::try_init_slice_in(n, policy.try_clone()?, |i, slot| {
        slot.write(i as u64).map_err(|e| e.error)
    })
    .map_err(|e| match e {
        jzap::error::InitError::Storage(e) | jzap::error::InitError::Initialize(e) => e,
    })?;
    let mut it = slice.into_iter();
    black_box(it.next());
    black_box(it.next_back());
    black_box(it.len());
    drop(it);
    black_box(Box::<[u64], _>::try_from_iter_in(
        (0..n).map(|i| i as u64),
        policy.try_clone()?,
    )?);
    // A false zero lower bound forces the Box collection overflow path.
    black_box(Box::<[u64], _>::try_from_iter_in(
        (0..n).filter(|_| true).map(|i| i as u64),
        policy.try_clone()?,
    )?);
    black_box(Box::<str, _>::try_from_chars_in(['a', '한', '🦀'], policy.try_clone()?)?);
    black_box(Box::<str, _>::try_from_texts_in(
        ["fallible", " ", "text"],
        policy.try_clone()?,
    )?);
    let uninit = Box::<u64, _>::try_new_uninit_in(policy.try_clone()?)?;
    black_box(Box::write(uninit, seed as u64));
    let zeroed = Box::<[u64], _>::try_new_zeroed_slice_in(n, policy.try_clone()?)?;
    // SAFETY: all-zero bytes are valid u64 elements.
    black_box(unsafe { Box::<[core::mem::MaybeUninit<u64>], _>::assume_init(zeroed) });
    let value = Box::try_new_in(seed, policy.try_clone()?).map_err(|e| e.error)?;
    let (ptr, retained_policy) = Box::into_raw_parts(value);
    // SAFETY: exact pointer/policy pair transferred immediately back, once.
    let value = unsafe { Box::from_raw_parts(ptr, retained_policy) };
    black_box(Box::into_inner(value));
    let callable = Box::try_new_in(move |x: usize| x ^ seed, policy.try_clone()?).map_err(|e| e.error)?;
    black_box(jzap::box_call_once!(callable; black_box(seed)));
    drop_operations(n, policy.try_clone()?)?;
    Ok(())
}

// Retain the actual destructor and cleanup loops. Copy payloads alone would
// erase these paths and hide obligations behind needs_drop::<T>() == false.
struct DropValue(u64);
impl Drop for DropValue {
    fn drop(&mut self) {
        black_box(&mut self.0);
    }
}
impl TryClone for DropValue {
    fn try_clone(&self) -> Result<Self, Error> {
        Ok(Self(self.0))
    }
}
fn init_error(error: jzap::error::InitError<Error>) -> Error {
    match error {
        jzap::error::InitError::Storage(e) | jzap::error::InitError::Initialize(e) => e,
    }
}
#[inline(never)]
fn drop_operations<P: AllocationPolicy>(n: usize, policy: P) -> Result<(), Error> {
    let b = Box::<[DropValue], _>::try_init_slice_in(n, policy.try_clone()?, |i, slot| {
        slot.write(DropValue(i as u64)).map_err(|e| e.error)
    })
    .map_err(init_error)?;
    let mut it = b.into_iter();
    black_box(it.next());
    black_box(it.next_back());
    drop(it);
    drop_vec_operations(n, Vec::new_in(policy.try_clone()?))?;
    drop_vec_operations(n, Vec::with_limit_in(256, policy.try_clone()?))?;
    Ok(())
}
#[inline(never)]
fn drop_vec_operations<P: AllocationPolicy, L: LengthPolicy>(
    n: usize,
    mut v: Vec<DropValue, P, L>,
) -> Result<(), Error> {
    v.try_resize_owned(n, DropValue(17)).map_err(|e| e.error)?;
    v.try_extend_from_within(..n)?;
    v.try_retain(|x| Ok::<_, Error>(x.0 % 2 != 0))?;
    v.try_dedup_by(|a, b| Ok::<_, Error>(a.0 == b.0))?;
    let len = v.len();
    for item in v.try_extract_if(..len, |x| Ok::<_, Error>(x.0 % 3 == 0))? {
        black_box(item?);
    }
    v.try_splice(..0, [DropValue(3), DropValue(5)])?.finish()?;
    let len = v.len();
    let mut drain = v.try_drain(..len)?;
    black_box(drain.next());
    drop(drain);
    v.try_push(DropValue(19)).map_err(|e| e.error)?;
    black_box(v.try_clone()?);
    drop(v.into_iter());
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_negative(seed: usize) -> usize {
    #[cfg(feature = "panic-control")]
    if black_box(seed) != 0 {
        panic!("deliberate link control");
    }
    #[cfg(feature = "bounds-control")]
    return black_box([1usize, 2])[black_box(seed)];
    #[cfg(feature = "oom-control")]
    return black_box(alloc::vec::Vec::<u64>::with_capacity(black_box(seed))).capacity();
    #[cfg(feature = "abort-control")]
    {
        unsafe extern "C" {
            fn abort() -> !;
        }
        unsafe { abort() }
    }
    #[allow(unreachable_code)]
    seed
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_shared(seed: usize, limit: usize) -> u32 {
    let local = jzap::budget::MemoryBudget::new(limit);
    let concurrent = jzap::budget::ConcurrentMemoryBudget::new(limit);
    let result = (|| {
        shared_operations::<jzap::shared::count::LocalCount, _>(
            seed,
            jzap::budget::Budget::Limited(&local),
        )?;
        shared_operations::<jzap::shared::count::AtomicCount, _>(
            seed,
            jzap::budget::Budget::Limited(&local),
        )?;
        shared_operations::<jzap::shared::count::AtomicCount, _>(
            seed,
            jzap::budget::ConcurrentBudget(&concurrent),
        )?;
        let a = jzap::sync::Arc::try_new(black_box(seed as u64)).map_err(|e| e.error)?;
        let a = jzap::arc_unsize!(a=>dyn core::any::Any+Send+Sync);
        black_box(
            jzap::sync::Arc::<dyn core::any::Any + Send + Sync>::downcast::<u64>(a)
                .map_err(|_| Error::InvalidStructure)?,
        );
        Ok::<_, Error>(())
    })();
    if local.occupied() != 0 || local.reserved() != 0 || concurrent.used() != 0 {
        return 2;
    }
    u32::from(result.is_err())
}
#[inline(never)]
fn shared_operations<C: jzap::shared::count::Counter, P: jzap::shared::SharedPolicy>(
    seed: usize,
    policy: P,
) -> Result<(), Error> {
    use jzap::shared::Strong;
    let n = seed % 17;
    let input = Box::try_new_in(DropValue(seed as u64), policy.try_clone()?).map_err(|e| e.error)?;
    black_box(Strong::<DropValue, C, P>::try_from_box_in(input).map_err(|e| e.error)?);
    let input = Box::<[u64], P>::try_copy_from_slice_in(&[3, 5, 7], policy.try_clone()?)?;
    let slice = Strong::<[u64], C, P>::try_from_box_in(input).map_err(|e| e.error)?;
    let array = Strong::<[u64], C, P>::try_into_array::<3>(slice).map_err(|_| Error::Bounds)?;
    black_box(Strong::into_slice(array));
    black_box(Strong::<[u64], C, P>::try_from_array_in([11, 13, 17], policy.try_clone()?).map_err(|e| e.error)?);
    black_box(Strong::<core::ffi::CStr, C, P>::try_from_ref_in(
        c"native", policy.try_clone()?,
    )?);
    let text = Strong::<str, C, P>::try_from_str_in("native", policy.try_clone()?)?;
    black_box(Strong::<str, C, P>::into_bytes(text));
    let mut a = Strong::<DropValue, C, P>::try_new_in(DropValue(seed as u64), policy.try_clone()?)
        .map_err(|e| e.error)?;
    black_box(Strong::get_mut(&mut a));
    let w = Strong::try_downgrade(&a)?;
    let q = w.try_clone()?;
    let b = a.try_clone()?;
    black_box(w.try_upgrade()?);
    black_box(Strong::<DropValue, C, P>::try_make_mut(&mut a)?);
    drop((b, w, q));
    let w = Strong::try_downgrade(&a)?;
    black_box(Strong::<DropValue, C, P>::try_make_mut(&mut a)?);
    black_box(w.try_upgrade()?);
    drop(w);
    let raw = Strong::into_raw(a);
    unsafe {
        Strong::<DropValue, C, P>::try_increment_strong_count(raw)?;
        Strong::<DropValue, C, P>::decrement_strong_count(raw);
    }
    let a = unsafe { Strong::<DropValue, C, P>::from_raw(raw) };
    black_box(Strong::strong_count(&a));
    black_box(Strong::weak_count(&a));
    black_box(Strong::ptr_eq(&a, &a));
    black_box(Strong::try_unwrap(a).map_err(|_| Error::InvalidStructure)?);
    let b = Strong::<u64, C, P>::try_new_cyclic_in(policy.try_clone()?, |w| {
        black_box(w.try_upgrade()?);
        Ok::<_, Error>(seed as u64)
    })
    .map_err(init_error)?;
    black_box(Strong::into_inner(b));
    let mut slice = Strong::<[DropValue], C, P>::try_init_slice_in(n, policy.try_clone()?, |i, s| {
        s.write(DropValue(i as u64)).map_err(|e| e.error)
    })
    .map_err(init_error)?;
    let clone = slice.try_clone()?;
    black_box(Strong::<[DropValue], C, P>::try_make_mut(&mut slice)?);
    drop(clone);
    let weak = Strong::try_downgrade(&slice)?;
    drop(slice);
    black_box(weak.try_upgrade()?);
    drop(weak);
    let mut text = Strong::<str, C, P>::try_from_str_in("aé한🦀", policy.try_clone()?)?;
    let clone = text.try_clone()?;
    black_box(Strong::<str, C, P>::try_make_mut(&mut text)?);
    drop(clone);
    let uninit = Strong::<u64, C, P>::try_new_zeroed_in(policy.try_clone()?)?;
    black_box(unsafe { Strong::<core::mem::MaybeUninit<u64>, C, P>::assume_init(uninit) });
    let slice = Strong::<[u64], C, P>::try_new_zeroed_slice_in(n, policy.try_clone()?)?;
    black_box(unsafe { Strong::<[core::mem::MaybeUninit<u64>], C, P>::assume_init(slice) });
    let pinned = Strong::<u64, C, P>::try_pin_in(seed as u64, policy.try_clone()?).map_err(|e| e.error)?;
    black_box(pinned.as_ref());
    black_box(pinned.try_clone()?);
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_hash_map(seed:usize,budget_limit:usize)->u32 {
    let local=MemoryBudget::new(budget_limit);
    let concurrent=jzap::budget::ConcurrentMemoryBudget::new(budget_limit);
    let result=(||{
        hash_map_operations(seed,jzap::budget::Unmetered)?;
        hash_map_operations(seed,Budget::Limited(&local))?;
        hash_map_operations(seed,jzap::budget::ConcurrentBudget(&concurrent))?;
        let owned=jzap::budget::OwnedBudget::try_new(budget_limit)?;
        hash_map_operations(seed,owned.try_clone()?)?;
        if owned.occupied()!=0||owned.reserved()!=0{return Err(Error::InvalidStructure)}
        let owned=jzap::budget::OwnedConcurrentBudget::try_new(budget_limit)?;
        hash_map_operations(seed,owned.try_clone()?)?;
        if owned.used()!=0{return Err(Error::InvalidStructure)}
        Ok::<_,Error>(())
    })();
    u32::from(result.is_err()||local.occupied()!=0||local.reserved()!=0||concurrent.used()!=0)
}
fn hash_map_operations<P:AllocationPolicy>(seed:usize,policy:P)->Result<(),Error>{
    use jzap::{collections::{HashMap,hash_map::{RandomState,Entry}},TryIterator};
    let mut m=HashMap::with_hasher_in(RandomState::from_seed(seed as u64,7),policy);
    for k in 0..48u64 {m.try_insert(black_box(k),black_box(k.wrapping_mul(3))).map_err(|e|e.error)?;}
    for k in 0..16u64{black_box(m.remove_entry(&k));}
    m.try_reserve(seed%33)?;
    m.try_shrink_to_fit()?;
    let k=black_box(seed as u64%48);
    black_box((m.get(&k),m.get_key_value(&k),m.contains_key(&k),m.hasher(),m.capacity()));
    if let Some(v)=m.get_mut(&k){*v=v.wrapping_add(1);}
    match m.entry(k){Entry::Occupied(mut e)=>{black_box(e.key());black_box(e.get());*e.get_mut()=k;black_box(e.insert(k));black_box(e.into_mut());},Entry::Vacant(e)=>{black_box(e.key());black_box(e.try_insert(k).map_err(|e|e.error)?);}}
    black_box(m.entry(89).try_or_insert_with_key(|k|Ok::<_,Error>(*k)).map_err(init_error)?);
    black_box(m.entry(90).try_insert_entry(90).map_err(|e|e.error)?);
    black_box(m.entry(91).try_or_default()?);
    black_box(m.entry(92).try_and_modify(|v|{*v=92;Ok::<_,Error>(())})?.try_or_insert(92).map_err(|e|e.error)?);
    let refs=m.try_get_disjoint_mut([&89,&90])?;black_box(refs);
    for (k,v) in m.iter_mut(){*v=v.wrapping_add(*k);}
    for v in m.values_mut(){*v=v.wrapping_add(1);}
    black_box((m.keys().count(),m.values().count(),m.iter().count()));
    struct Sink(usize);
    impl core::fmt::Write for Sink {fn write_str(&mut self,s:&str)->core::fmt::Result{self.0=self.0.wrapping_add(s.len());Ok(())}}
    use core::fmt::Write;
    let mut sink=Sink(0);
    write!(&mut sink,"{:?}{:?}{:?}{:?}",m,m.keys(),m.values(),m.iter()).map_err(|_|Error::InvalidStructure)?;
    write!(&mut sink,"{:?}",m.values_mut()).map_err(|_|Error::InvalidStructure)?;
    write!(&mut sink,"{:?}",m.entry(93)).map_err(|_|Error::InvalidStructure)?;
    black_box(sink.0);
    let clone=m.try_clone()?;black_box(clone==m);
    m.try_retain(|k,_|Ok::<_,Error>(*k%3!=0))?;
    let mut extract=m.try_extract_if(|k:&u64,_:&mut u64|Ok::<_,Error>(*k%5==0));
    while let Some(pair)=extract.try_next()?{black_box(pair);}
    drop(extract);
    black_box(m.try_clone()?.into_keys().count());black_box(m.try_clone()?.into_values().count());
    let mut c=m.try_clone()?;black_box(c.drain().next());black_box(c.is_empty());
    let mut owned=m.into_iter();black_box(owned.next());black_box(owned.len());drop(owned);
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_hash_set(seed:usize,budget_limit:usize)->u32 {
    let local=MemoryBudget::new(budget_limit);
    let concurrent=jzap::budget::ConcurrentMemoryBudget::new(budget_limit);
    let result=(||{
        hash_set_operations(seed,jzap::budget::Unmetered)?;
        hash_set_operations(seed,Budget::Limited(&local))?;
        hash_set_operations(seed,jzap::budget::ConcurrentBudget(&concurrent))?;
        let owned=jzap::budget::OwnedBudget::try_new(budget_limit)?;
        hash_set_operations(seed,owned.try_clone()?)?;
        if owned.occupied()!=0||owned.reserved()!=0{return Err(Error::InvalidStructure)}
        let owned=jzap::budget::OwnedConcurrentBudget::try_new(budget_limit)?;
        hash_set_operations(seed,owned.try_clone()?)?;
        if owned.used()!=0{return Err(Error::InvalidStructure)}
        Ok::<_,Error>(())
    })();
    u32::from(result.is_err()||local.occupied()!=0||local.reserved()!=0||concurrent.used()!=0)
}
fn hash_set_operations<P:AllocationPolicy>(seed:usize,policy:P)->Result<(),Error>{
    use jzap::{collections::{HashSet,hash_map::RandomState},TryIterator};
    let mut a=HashSet::with_hasher_in(RandomState::from_seed(seed as u64,7),policy.try_clone()?);
    let mut b=HashSet::with_hasher_in(RandomState::from_seed(seed as u64,9),policy);
    for k in 0..32u64 {a.try_insert(black_box(k)).map_err(|e|e.error)?;b.try_insert(black_box(k.wrapping_add(17))).map_err(|e|e.error)?;}
    let key=black_box(seed as u64%32);
    black_box(a.try_replace(key).map_err(|e|e.error)?);
    black_box(a.try_replace(99).map_err(|e|e.error)?);
    black_box((a.get(&key),a.contains(&key),a.len(),a.capacity(),a.hasher()));
    black_box(a.take(&key));black_box(a.remove(&100));
    a.try_reserve(seed%33)?;a.try_shrink_to_fit()?;
    black_box((a.is_subset(&b),a.is_superset(&b),a.is_disjoint(&b)));
    black_box((a.union(&b).count(),a.intersection(&b).count(),a.difference(&b).count(),a.symmetric_difference(&b).count()));
    black_box(a.try_union(&b)?);black_box(a.try_intersection(&b)?);black_box(a.try_difference(&b)?);black_box(a.try_symmetric_difference(&b)?);
    struct Sink(usize);
    impl core::fmt::Write for Sink {fn write_str(&mut self,s:&str)->core::fmt::Result{self.0=self.0.wrapping_add(s.len());Ok(())}}
    use core::fmt::Write;
    let mut sink=Sink(0);
    write!(&mut sink,"{:?}{:?}{:?}{:?}{:?}{:?}",a,a.iter(),a.union(&b),a.intersection(&b),a.difference(&b),a.symmetric_difference(&b)).map_err(|_|Error::InvalidStructure)?;
    let mut clone=a.try_clone()?;clone.try_clone_from(&b)?;black_box(clone==b);
    clone.try_retain(|k|Ok::<_,Error>(*k%3!=0))?;
    let mut extract=clone.try_extract_if(|k|Ok::<_,Error>(*k%5==0));write!(&mut sink,"{extract:?}").map_err(|_|Error::InvalidStructure)?;while let Some(k)=extract.try_next()?{black_box(k);}drop(extract);
    let mut drain=clone.drain();black_box(drain.next());write!(&mut sink,"{drain:?}").map_err(|_|Error::InvalidStructure)?;black_box(drain.len());drop(drain);
    let mut consumed=a.try_clone()?.into_iter();black_box(consumed.next());write!(&mut sink,"{consumed:?}").map_err(|_|Error::InvalidStructure)?;black_box(consumed.len());drop(consumed);
    black_box(sink.0);black_box(a.iter().count());black_box(a.try_clone()?.into_iter().count());
    let target=MemoryBudget::new(usize::MAX);black_box(jzap::TryCloneIn::try_clone_in(&a,Budget::Limited(&target))?);
    if target.occupied()!=0||target.reserved()!=0{return Err(Error::InvalidStructure)}
    a.clear();b.clear();Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_binary_heap(seed:usize,budget_limit:usize)->u32 {
    let local=MemoryBudget::new(budget_limit);
    let concurrent=jzap::budget::ConcurrentMemoryBudget::new(budget_limit);
    let result=(||{
        binary_heap_operations(seed,jzap::budget::Unmetered)?;
        binary_heap_operations(seed,Budget::Limited(&local))?;
        binary_heap_operations(seed,jzap::budget::ConcurrentBudget(&concurrent))?;
        let owned=jzap::budget::OwnedBudget::try_new(budget_limit)?;
        binary_heap_operations(seed,owned.try_clone()?)?;
        if owned.occupied()!=0 || owned.reserved()!=0 {return Err(Error::InvalidStructure)}
        let owned=jzap::budget::OwnedConcurrentBudget::try_new(budget_limit)?;
        binary_heap_operations(seed,owned.try_clone()?)?;
        if owned.used()!=0 {return Err(Error::InvalidStructure)}
        Ok::<_,Error>(())
    })();u32::from(result.is_err())
}
fn binary_heap_operations<P:AllocationPolicy>(seed:usize,policy:P)->Result<(),Error> {
    use jzap::{collections::{BinaryHeap,binary_heap::PeekMut},TryCloneIn,TryIterator,TryDoubleEndedIterator,TryExactSizeIterator};
    let mut a=BinaryHeap::try_with_capacity_in(seed%7,policy)?;
    a.try_extend([seed,3,7,2,9]).map_err(|e|match e {jzap::collections::binary_heap::ExtendError::Push(e)=>e.error,jzap::collections::binary_heap::ExtendError::Source(e)=>match e{}})?;
    a.try_reserve(4)?;a.try_reserve_exact(1)?;
    if let Some(mut p)=a.peek_mut(){*p=seed%11;black_box(*p);}
    if let Some(p)=a.peek_mut(){black_box(PeekMut::pop(p));}
    a.try_push(17).map_err(|e|e.error)?;
    a.try_retain(|v|Ok::<_,Error>(*v!=2))?;
    black_box((a.len(),a.is_empty(),a.capacity(),a.max_len(),a.peek(),a.as_slice(),a.allocation_policy()));
    struct Sink(usize);
    impl core::fmt::Write for Sink {fn write_str(&mut self,s:&str)->core::fmt::Result{self.0=self.0.wrapping_add(s.len());Ok(())}}
    let mut sink=Sink(0);
    core::fmt::write(&mut sink,format_args!("{:?}",a)).map_err(|_|Error::InvalidStructure)?;
    if let Some(p)=a.peek_mut(){core::fmt::write(&mut sink,format_args!("{:?}",p)).map_err(|_|Error::InvalidStructure)?;}
    let mut b=a.try_clone()?;b.try_clone_from(&a)?;a.try_append(&mut b)?;
    let target=MemoryBudget::new(usize::MAX);drop(a.try_clone_in(Budget::Limited(&target))?);
    {let mut i=a.iter();let _=black_box(i.try_next());let _=black_box(i.try_next_back());let _=black_box(i.try_len());}
    let mut i=a.try_clone()?.into_iter();let _=black_box(i.try_next());let _=black_box(i.try_next_back());core::fmt::write(&mut sink,format_args!("{:?}",i)).map_err(|_|Error::InvalidStructure)?;drop(i.try_clone()?);drop(i);
    {let mut d=a.drain();let _=black_box(d.try_next());let _=black_box(d.try_next_back());let _=black_box(d.try_len());core::fmt::write(&mut sink,format_args!("{:?}",d)).map_err(|_|Error::InvalidStructure)?;}
    a.try_shrink_to(0)?;a.try_shrink_to_fit()?;a.try_push(seed).map_err(|e|e.error)?;
    a.clear();
    for value in 0..(seed%17){a.try_push(value).map_err(|e|e.error)?;}
    let policy=a.allocation_policy().try_clone()?;
    let bounded=jzap::vec::Vec::with_limit_in(8,policy);
    let mut bounded=BinaryHeap::from_vec(bounded);
    bounded.try_extend_copy(&[seed,1,2][..]).map_err(|e|match e {jzap::collections::binary_heap::ExtendError::Push(e)=>e.error,jzap::collections::binary_heap::ExtendError::Source(e)=>match e{}})?;
    black_box(bounded.into_vec());
    let a=BinaryHeap::from_vec(a.into_vec());
    black_box(a.into_sorted_vec());black_box(sink.0);
    drop(BinaryHeap::try_from_array([seed,1,2]).map_err(|e|e.error)?);
    drop(BinaryHeap::try_from_iter([seed,1,2]).map_err(|e|match e {jzap::collections::binary_heap::ExtendError::Push(e)=>e.error,jzap::collections::binary_heap::ExtendError::Source(e)=>match e{}})?);
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_deque(seed:usize,budget_limit:usize)->u32 {
    let local=MemoryBudget::new(budget_limit);
    let concurrent=jzap::budget::ConcurrentMemoryBudget::new(budget_limit);
    let result=(||{
        deque_operations(seed,jzap::budget::Unmetered)?;
        deque_operations(seed,Budget::Limited(&local))?;
        deque_operations(seed,jzap::budget::ConcurrentBudget(&concurrent))?;
        let owned=jzap::budget::OwnedBudget::try_new(budget_limit)?;
        deque_operations(seed,owned.try_clone()?)?;
        if owned.occupied()!=0 || owned.reserved()!=0 {return Err(Error::InvalidStructure)}
        let owned=jzap::budget::OwnedConcurrentBudget::try_new(budget_limit)?;
        deque_operations(seed,owned.try_clone()?)?;
        if owned.used()!=0 {return Err(Error::InvalidStructure)}
        Ok::<_,Error>(())
    })();u32::from(result.is_err())
}
fn deque_operations<P:AllocationPolicy>(seed:usize,policy:P)->Result<(),Error> {
    use jzap::{collections::VecDeque,TryCloneIn,TryIterator,TryDoubleEndedIterator,TryExactSizeIterator};
    let mut a=VecDeque::try_with_capacity_in(seed%7,policy)?;
    a.try_extend([seed,3,7,2,9]).map_err(|e|match e {jzap::deque::ExtendError::Push(e)=>e.error,jzap::deque::ExtendError::Source(e)=>match e{}})?;
    a.try_push_front(1).map_err(|r|r.error)?;
    *a.try_push_back_mut(5).map_err(|r|r.error)?=6;
    *a.try_push_front_mut(8).map_err(|r|r.error)?=4;
    a.try_insert(3,11).map_err(|r|r.error)?;*a.try_insert_mut(1,13).map_err(|r|r.error)?=15;
    black_box((a.remove(2),a.swap_remove_front(3),a.swap_remove_back(4)));
    a.swap(0,1)?;a.rotate_left(2)?;a.rotate_right(1)?;
    black_box((a.get(0)?,a.front(),a.back(),a.as_slices()));black_box(a.get_mut(0)?);
    if let Some(v)=a.front_mut(){*v=3}if let Some(v)=a.back_mut(){*v=7}
    black_box(a.as_mut_slices());a.try_reserve(6)?;a.try_reserve_exact(1)?;
    a.try_retain(|v|Ok::<_,Error>(*v!=2))?;a.try_retain_mut(|v|{*v=v.wrapping_add(1);Ok::<_,Error>(true)})?;
    for v in a.range_mut(..)?{*v=v.wrapping_add(1)}
    let mut range=a.range(..)?;let _=black_box(range.try_next());let _=black_box(range.try_next_back());let _=black_box(range.try_len());
    let mut iter=a.iter_mut();let _=black_box(iter.try_next());let _=black_box(iter.try_next_back());
    black_box(a.make_contiguous());
    let _=black_box((a.binary_search(&seed),a.binary_search_by(|v|v.cmp(&seed)),a.binary_search_by_key(&seed,|v|*v),a.partition_point(|v|*v<seed),a.contains(&seed)));
    let _=a.try_pop_front_if(|v|Ok::<_,Error>(*v==1))?;let _=a.try_pop_back_if(|v|Ok::<_,Error>(*v==1))?;
    a.try_resize(12,&seed)?;a.try_resize_with(14,||Ok::<_,Error>(seed)).map_err(|e|match e{jzap::error::InitError::Storage(e)|jzap::error::InitError::Initialize(e)=>e})?;
    let mut b=a.try_clone()?;b.try_clone_from(&a)?;a.try_append(&mut b)?;
    let target=MemoryBudget::new(usize::MAX);drop(a.try_clone_in(Budget::Limited(&target))?);
    let mut tail=a.try_split_off(a.len()/2)?;a.try_append(&mut tail)?;
    {let mut d=a.drain(1..3)?;let _=black_box(d.try_next());let _=black_box(d.try_next_back());let _=black_box(d.try_len());}
    a.truncate(3);a.try_shrink_to(1)?;a.try_shrink_to_fit()?;
    struct Sink;impl core::fmt::Write for Sink{fn write_str(&mut self,_:&str)->core::fmt::Result{Ok(())}}
    core::fmt::write(&mut Sink,format_args!("{:?} {:?} {:?}",a,a.iter(),jzap::deque::Iter::<usize>::default())).map_err(|_|Error::InvalidStructure)?;
    core::fmt::write(&mut Sink,format_args!("{:?} {:?}",a.iter_mut(),jzap::deque::IterMut::<usize>::default())).map_err(|_|Error::InvalidStructure)?;
    core::fmt::write(&mut Sink,format_args!("{:?}",a.drain(1..2)?)).map_err(|_|Error::InvalidStructure)?;
    let mut i=a.try_clone()?.into_iter();let _=black_box(i.try_next());let _=black_box(i.try_next_back());drop(i.try_clone()?);
    black_box((a.len(),a.capacity(),a.max_len(),a.remaining_limit(),a.is_empty()));
    let std=a.into_std();let mut a=VecDeque::try_from_std(std).map_err(|r|r.error)?;a.clear();
    black_box(VecDeque::from_vec(a.into_vec()));
    let mut bounded=VecDeque::try_with_capacity_and_limit(2,4)?;bounded.try_extend_copy(&[1usize,2])?;
    drop(VecDeque::try_from_array([seed,2]).map_err(|r|r.error)?);
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn audit_scratch_slice(seed:usize,budget_limit:usize)->u32 {
    let local=MemoryBudget::new(budget_limit);
    let concurrent=jzap::budget::ConcurrentMemoryBudget::new(budget_limit);
    let result=(|| {
        scratch_operations(seed,jzap::budget::Unmetered)?;
        scratch_operations(seed,Budget::Limited(&local))?;
        scratch_operations(seed,jzap::budget::ConcurrentBudget(&concurrent))?;
        let local=jzap::budget::OwnedBudget::try_new(budget_limit)?;
        scratch_operations(seed,local.try_clone()?)?;
        let concurrent=jzap::budget::OwnedConcurrentBudget::try_new(budget_limit)?;
        scratch_operations(seed,concurrent.try_clone()?)?;
        Ok::<_,Error>(())
    })();u32::from(result.is_err())
}
fn scratch_operations<P:AllocationPolicy>(seed:usize,policy:P)->Result<(),Error> {
    use jzap::{scratch::{Scratch,ScratchBuf},slice::{Slice,SliceMut},TryCloneIn,TryIterator};
    use jzap::error::InitError;
    let flatten=|e|match e {InitError::Storage(e)|InitError::Initialize(e)=>e};
    let mut a=ScratchBuf::try_with_capacity_in(16,policy)?;
    a.try_push(seed).map_err(|r|r.error)?;
    a.try_extend_copy(&[1,2])?;a.try_extend_cloned(&[3,4])?;
    a.try_resize_with_init(8,|i,slot|slot.write(i).map_err(|r|r.error)).map_err(flatten)?;
    a.try_resize(10,&seed)?;
    a.try_extend(jzap::iter::from_std([2,3])).map_err(|e|match e{InitError::Storage(e)=>e,InitError::Initialize(e)=>match e{}})?;
    let _=black_box(a.get(seed));let _=black_box(a.get_mut(seed));let _=black_box(a.pop());
    let _=black_box(a.as_view().range(..));let _=black_box(a.as_view_mut().range_mut(..));
    let _=black_box(a.iter().try_next());let _=black_box(a.iter_mut().try_next());
    let mut b=a.try_clone()?;b.try_clone_from(&a)?;
    drop(a.try_clone_in(Budget::Unmetered)?);
    let _=black_box((a.len(),a.is_empty(),a.capacity(),a.max_len(),a.remaining_capacity(),a.allocation_policy()));
    a.truncate(2);a.clear();drop(ScratchBuf::from_vec(a.into_vec()));
    let mut limited=ScratchBuf::try_with_capacity_and_limit(2,3)?;limited.try_push(()).map_err(|r|r.error)?;
    let mut cells=[core::mem::MaybeUninit::uninit();16];let mut s=Scratch::new(&mut cells);
    s.try_push(seed).map_err(|r|r.error)?;s.try_extend_copy(&[1,2])?;s.try_extend_cloned(&[3,4])?;
    s.try_resize_with_init(7,|i,slot|slot.write(i).map_err(|r|r.error)).map_err(flatten)?;
    s.try_resize(8,&seed)?;s.try_extend(jzap::iter::from_std([2,3])).map_err(|e|match e{InitError::Storage(e)=>e,InitError::Initialize(e)=>match e{}})?;
    let _=black_box((s.len(),s.is_empty(),s.capacity(),s.remaining_capacity(),s.get(seed)));
    let _=black_box(s.get_mut(seed));let _=black_box(s.as_view());let _=black_box(s.as_view_mut());
    let _=black_box(s.iter().try_next());let _=black_box(s.iter_mut().try_next());let _=black_box(s.pop());s.truncate(1);s.clear();
    let mut bytes=[0u8;8];let mut s=Scratch::from_bytes(&mut bytes);s.try_push(seed as u8).map_err(|r|r.error)?;s.clear();drop(s);let _=black_box(bytes);
    let values=[seed,2,3,4];let view=Slice::new(&values);let clone=view.try_clone()?;
    let _=black_box((clone.as_slice(),clone.first(),clone.last(),clone.get(seed),clone.range(..=seed),clone.split_at(seed),clone==view));
    let _=black_box(view.iter().try_next());drop(view.try_to_vec()?);
    let mut values=values;let mut view=SliceMut::new(&mut values);
    let _=black_box(view.get(seed));let _=black_box(view.get_mut(seed));let _=black_box(view.range(seed..));let _=black_box(view.range_mut(..seed));
    let _=black_box(view.split_at_mut(seed));let _=black_box(view.swap(seed,0));let _=black_box(view.copy_within(..seed,1));
    view.copy_from_slice(&[1,2,3,4])?;let mut other=[4,3,2,1];view.swap_with_slice(&mut other)?;
    let _=black_box(view.iter().try_next());let _=black_box(view.iter_mut().try_next());let _=black_box(view.as_slice());let _=black_box(view.as_mut_slice());drop(view.try_to_vec()?);
    let _=black_box(view.as_view());let _=black_box(view.into_shared());let _=black_box(SliceMut::new(&mut values).into_slice());
    struct C(usize);impl Drop for C {fn drop(&mut self){black_box(self.0);}}impl TryClone for C {fn try_clone(&self)->Result<Self,Error>{Ok(C(self.0))}}
    let mut a=ScratchBuf::try_with_capacity(4)?;a.try_push(C(seed)).map_err(|r|r.error)?;let b=a.try_clone()?;a.try_clone_from(&b)?;
    let mut cells=[const {core::mem::MaybeUninit::uninit()};4];let mut s=Scratch::new(&mut cells);s.try_extend_cloned(b.as_slice())?;
    let before=s.len();let result=s.try_resize_with_init(3,|i,slot|{slot.write(C(i)).map_err(|r|r.error)?;if seed&1==0 {Err(Error::Capacity)}else{Ok(())}});if result.is_err()&&s.len()!=before{return Err(Error::InvalidStructure)}
    let before=a.len();let result=a.try_resize_with_init(3,|i,slot|{slot.write(C(i)).map_err(|r|r.error)?;if seed&1==0 {Err(Error::Capacity)}else{Ok(())}});if result.is_err()&&a.len()!=before{return Err(Error::InvalidStructure)}
    struct Sink;impl core::fmt::Write for Sink{fn write_str(&mut self,_:&str)->core::fmt::Result{Ok(())}}
    core::fmt::write(&mut Sink,format_args!("{:?} {:?}",Slice::new(&values),SliceMut::new(&mut other))).map_err(|_|Error::InvalidStructure)?;
    Ok(())
}
