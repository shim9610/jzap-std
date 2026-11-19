#![no_main]
use libfuzzer_sys::fuzz_target;
use jzap::{vec::Vec as V,budget::{Budget,MemoryBudget},TryCloneIn};
fuzz_target!(|data:&[u8]|{
 let limit=data.first().copied().unwrap_or(0) as usize;
 let budget=MemoryBudget::new(1024);
 let mut v=V::<u8,_>::with_limit_in(limit,Budget::Limited(&budget));let mut model=Vec::new();
 for (i,&x) in data.iter().take(128).enumerate(){
  match x%11{
   0=>{let r=v.try_push(x);if model.len()<limit {model.push(x);assert!(r.is_ok());}else{assert!(r.is_err());}},
   1=>assert_eq!(v.pop(),model.pop()),
   2=>{let at=i%(model.len()+1);let r=v.try_insert(at,x);if model.len()<limit{model.insert(at,x);assert!(r.is_ok());}else{assert!(r.is_err());}},
   3=>{if !model.is_empty(){let at=i%model.len();assert_eq!(v.remove(at).unwrap(),model.remove(at));}},
   4=>{v.truncate(x as usize);model.truncate(x as usize);},
   5=>{let start=i%(model.len()+1);let end=start+(x as usize)%(model.len()-start+1);drop(v.try_drain(start..end).unwrap());drop(model.drain(start..end));},
   6=>{v.retain(|n|*n%3!=x%3);model.retain(|n|*n%3!=x%3);},
   7=>{v.dedup();model.dedup();},
   8=>{let start=i%(model.len()+1);let end=start+(x as usize)%(model.len()-start+1);let take=x as usize%4;let a:Vec<_>=v.try_extract_if(start..end,|n|Ok::<_,()>(*n%2==0)).unwrap().take(take).map(Result::unwrap).collect();let b:Vec<_>=model.extract_if(start..end,|n|*n%2==0).take(take).collect();assert_eq!(a,b);},
   9=>{let start=i%(model.len()+1);let end=start+(x as usize)%(model.len()-start+1);let n=(x as usize%4).min(limit-(model.len()-(end-start)));v.try_splice(start..end,(0..n).filter(|n|n%2==x as usize%2).map(|n|n as u8)).unwrap().finish().unwrap();drop(model.splice(start..end,(0..n).filter(|n|n%2==x as usize%2).map(|n|n as u8)));},
   _=>{let target=MemoryBudget::new(256);let copy=v.try_clone_in(Budget::Limited(&target)).unwrap();assert_eq!(copy.as_slice(),model.as_slice());drop(copy);assert_eq!(target.occupied(),0);}
  }
  assert_eq!(v.as_slice(),model.as_slice());assert!(v.capacity()<=limit);assert_eq!(budget.reserved(),0);
 }
 drop(v);assert_eq!(budget.occupied(),0);
});
