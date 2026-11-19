#![no_main]
use libfuzzer_sys::fuzz_target;
use jzap::{budget::{Budget,MemoryBudget},boxed::Box,TryCloneIn};
fuzz_target!(|data:&[u8]|{
 let target=MemoryBudget::new(data.first().copied().unwrap_or(0) as usize);
 let source=Box::try_new_in([1u64,2,3,4],Budget::Unmetered).ok().unwrap();
 let slice=Box::<[u8],_>::try_copy_from_slice_in(data,Budget::Limited(&target));
 if let Ok(slice)=slice {
  assert_eq!(slice.as_slice(),data);
  match Box::<str,_>::try_from_utf8(slice) { Ok(text)=>assert_eq!(text.as_str().as_bytes(),data), Err(e)=>assert_eq!(e.value.as_slice(),data) }
 }
 assert_eq!(target.occupied(),0);
 let collected=Box::<[u8],_>::try_from_iter_in(data.iter().copied().filter(|b| b%3!=0),Budget::Limited(&target));
 if let Ok(collected)=collected { assert!(collected.iter().all(|b| b%3!=0)); }
 assert_eq!(target.occupied(),0);
 for &n in data.iter().take(64){
  if n%2==0 {let copy=source.try_clone_in(Budget::Limited(&target));if let Ok(copy)=copy{assert_eq!(*copy,*source);drop(copy)}}
  else{let r=target.reserve(n as usize);if let Ok(r)=r{if n%3==0{drop(r.commit())}else{drop(r)}}}
  assert_eq!(target.occupied(),0);assert_eq!(target.reserved(),0);
 }
});
