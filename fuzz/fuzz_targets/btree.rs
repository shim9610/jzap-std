#![no_main]
use jzap::{collections::BTreeMap,TryClone,TryIterator,budget::{Budget,MemoryBudget}};
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data:&[u8]|{
    let limit=usize::from(data.first().copied().unwrap_or(0)).saturating_mul(512);
    let budget=MemoryBudget::new(limit);
    let mut ours=BTreeMap::new_in(Budget::Limited(&budget));let mut model=std::collections::BTreeMap::new();
    for bytes in data.chunks(4).take(4096){
        let op=bytes.first().copied().unwrap_or(0);let key=u16::from_le_bytes([bytes.get(1).copied().unwrap_or(0),bytes.get(2).copied().unwrap_or(0)]);let value=bytes.get(3).copied().unwrap_or(0);
        match op%9{
            0|1=>{if let Ok(old)=ours.try_insert(key,value){assert_eq!(old,model.insert(key,value));}},
            2=>assert_eq!(ours.remove_entry(&key),model.remove_entry(&key)),
            3=>assert_eq!(ours.pop_first(),model.pop_first()),
            4=>assert_eq!(ours.pop_last(),model.pop_last()),
            5=>{if let Ok(mut tail)=ours.try_split_off(&key){let mut expected=model.split_off(&key);match ours.try_append(&mut tail){Ok(())=>model.append(&mut expected),Err(_)=>{assert!(tail.iter().eq(expected.iter()));}}}},
            6=>{ours.try_retain(|k,v|{*v^=value;Ok::<_,()>(*k%3!=key%3)}).unwrap();model.retain(|k,v|{*v^=value;*k%3!=key%3});},
            7=>{let mut iter=ours.try_extract_if(key..,|k,_|Ok::<_,()>(*k%2==0)).unwrap();let expected=model.extract_if(key..,|k,_|*k%2==0);for p in expected{assert_eq!(iter.try_next().unwrap(),Some(p));}assert!(iter.try_next().unwrap().is_none());},
            _=>{if let Ok(copy)=ours.try_clone(){assert_eq!(copy,ours);}},
        }
        assert_eq!(ours.len(),model.len());assert!(ours.iter().eq(model.iter()));assert_eq!(budget.reserved(),0);assert!(budget.occupied()<=limit);
    }
    drop(ours);assert_eq!(budget.occupied(),0);
});
