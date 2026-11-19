use jzap::{Error, TryClone, vec::Vec as V};
use proptest::prelude::*;
proptest! {
 #![proptest_config(ProptestConfig::with_cases(2048))]
 #[test]
 fn bounded_edits_match_std(limit in 0usize..80, ops in prop::collection::vec((any::<u8>(),any::<u8>()),0..256)) {
    let mut ours=V::<u8>::with_limit(limit);let mut model=Vec::new();
    for (op,arg) in ops {
      match op%9 {
       0=>{let r=ours.try_push(arg);if model.len()<limit {model.push(arg);prop_assert!(r.is_ok());}else{prop_assert_eq!(r.unwrap_err().error,Error::Capacity);}},
       1=>{prop_assert_eq!(ours.pop(),model.pop());},
       2=>{let index=arg as usize%(model.len()+2);let r=ours.try_insert(index,arg);if index>model.len(){prop_assert_eq!(r.unwrap_err().error,Error::Bounds);}else if model.len()==limit{prop_assert_eq!(r.unwrap_err().error,Error::Capacity);}else{model.insert(index,arg);prop_assert!(r.is_ok());}},
       3=>{let index=arg as usize%(model.len()+1);let r=ours.remove(index);if index<model.len(){prop_assert_eq!(r,Ok(model.remove(index)));}else{prop_assert_eq!(r,Err(Error::Bounds));}},
       4=>{let index=arg as usize%(model.len()+1);let r=ours.swap_remove(index);if index<model.len(){prop_assert_eq!(r,Ok(model.swap_remove(index)));}else{prop_assert_eq!(r,Err(Error::Bounds));}},
       5=>{ours.truncate(arg as usize);model.truncate(arg as usize);},
       6=>{let additional=arg as usize;prop_assert_eq!(ours.try_reserve(additional).is_ok(),additional<=limit-model.len());},
       7=>{let count=arg as usize%8;let r=ours.try_resize(model.len()+count,&arg);if count<=limit-model.len(){model.resize(model.len()+count,arg);prop_assert!(r.is_ok());}else{prop_assert_eq!(r,Err(Error::Capacity));}},
       _=>{let copy=ours.try_clone().unwrap();prop_assert_eq!(copy.as_slice(),model.as_slice());prop_assert_eq!(copy.max_len(),limit);}
      }
      prop_assert_eq!(ours.as_slice(),model.as_slice());prop_assert!(ours.len()<=ours.capacity());prop_assert!(ours.capacity()<=limit);
    }
 }
 #[test]
 fn copy_extension_and_iteration(limit in 0usize..100, input in prop::collection::vec(any::<u64>(),0..120), choices in prop::collection::vec(any::<bool>(),0..140)) {
    let mut ours=V::with_limit(limit);let r=ours.try_extend_copy(&input);
    if input.len()>limit {prop_assert_eq!(r,Err(Error::Capacity));prop_assert!(ours.is_empty());}
    else {prop_assert!(r.is_ok());let mut a=ours.into_iter();let mut b=input.into_iter();for back in choices {prop_assert_eq!(if back{a.next_back()}else{a.next()},if back{b.next_back()}else{b.next()});prop_assert_eq!(a.as_slice(),b.as_slice());}}
 }
}

proptest! {
 #![proptest_config(ProptestConfig::with_cases(2048))]
 #[test]
 fn complete_edits_match_std(input in prop::collection::vec(0u8..12,0..64), ops in prop::collection::vec((any::<u8>(),any::<u8>(),any::<u8>()),0..96)) {
    let mut a=V::try_from_slice(&input).unwrap();let mut b=input;
    for (op,x,y) in ops {
       let start=x as usize%(b.len()+1);let end=start+y as usize%(b.len()-start+1);
       match op%7 {
        0=>{let left:Vec<_>=a.try_drain(start..end).unwrap().collect();let right:Vec<_>=b.drain(start..end).collect();prop_assert_eq!(left,right);},
        1=>{let mut left=a.try_splice(start..end,0..y%8).unwrap();let removed:Vec<_>=left.by_ref().collect();let expected:Vec<_>=b.splice(start..end,0..y%8).collect();prop_assert_eq!(removed,expected);left.finish().unwrap();},
        2=>{let left:Vec<_>=a.try_extract_if(start..end,|v|Ok::<_,()>(*v%3==x%3)).unwrap().take(y as usize).map(Result::unwrap).collect();let right:Vec<_>=b.extract_if(start..end,|v|*v%3==x%3).take(y as usize).collect();prop_assert_eq!(left,right);},
        3=>{a.retain(|v|*v%3!=x%3);b.retain(|v|*v%3!=x%3);},
        4=>{a.dedup_by_key(|v|*v%3);b.dedup_by_key(|v|*v%3);},
        5=>{if b.len()<128{a.try_extend_from_within(start..end).unwrap();b.extend_from_within(start..end);}},
        _=>{prop_assert_eq!(a.pop_if(|v|*v%2==0),b.pop_if(|v|*v%2==0));}
       }
       prop_assert_eq!(a.as_slice(),b.as_slice());
    }
 }
}
