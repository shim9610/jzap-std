#![no_main]
use libfuzzer_sys::fuzz_target;
use jzap::{TryClone,TryIterator,collections::{HashMap,hash_map::RandomState}};
use std::collections::HashMap as Model;
fuzz_target!(|data:&[u8]|{
 let seed=RandomState::from_seed(0,0);let mut a=HashMap::with_hasher(seed);let mut b=Model::with_hasher(seed);
 for chunk in data.chunks_exact(4) {
 let (op,k,v)=(chunk[0]%12,u16::from_le_bytes([chunk[1],chunk[2]]),chunk[3]);
 match op {
 0=>assert_eq!(a.try_insert(k,v).unwrap(),b.insert(k,v)),
 1=>assert_eq!(a.remove_entry(&k),b.remove_entry(&k)),
 2=>assert_eq!(a.get(&k),b.get(&k)),
 3=>assert_eq!(a.entry(k).try_or_insert(v).unwrap(),b.entry(k).or_insert(v)),
 4=>{a.try_reserve(v as usize).unwrap();b.reserve(v as usize);},
 5=>{a.try_shrink_to(v as usize).unwrap();b.shrink_to(v as usize);},
 6=>{a.try_retain(|k,_|Ok::<_,()>(*k%3!=u16::from(v)%3)).unwrap();b.retain(|k,_|*k%3!=u16::from(v)%3);},
 7=>{let c=a.try_clone().unwrap();assert_eq!(c.len(),b.len());for (k,v) in &b{assert_eq!(c.get(k),Some(v));}},
 8=>{for value in a.values_mut(){*value=value.wrapping_add(v)}for value in b.values_mut(){*value=value.wrapping_add(v)}},
 9=>{let mut i=a.try_extract_if(|k:&u16,_:&mut u8|Ok::<_,()>(*k%5==u16::from(v)%5));let mut removed=Vec::new();while let Some(pair)=i.try_next().unwrap(){removed.push(pair)}for (k,v) in removed{assert_eq!(b.remove(&k),Some(v));}},
 10=>{if v==0{a.clear();b.clear();}},
 _=>{let mut c=a.try_clone().unwrap();let count=c.drain().count();assert_eq!(count,b.len());assert!(c.is_empty());},
 }
 assert_eq!(a.len(),b.len());for (k,v) in &b{assert_eq!(a.get(k),Some(v))}assert_eq!(a.iter().count(),b.len());
 }
});
