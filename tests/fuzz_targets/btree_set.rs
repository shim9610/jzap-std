#![no_main]
use libfuzzer_sys::fuzz_target;
use jzap::{TryClone,TryIterator,collections::BTreeSet};
use std::collections::BTreeSet as Model;
fuzz_target!(|data:&[u8]|{
    let(mut a,mut b)=(BTreeSet::new(),BTreeSet::new());
    let(mut ma,mut mb)=(Model::new(),Model::new());
    for chunk in data.chunks_exact(3){
        let(op,key,other)=(chunk[0]%14,u16::from(chunk[1]),u16::from(chunk[2]));
        match op{
            0=>assert_eq!(a.try_insert(key).unwrap(),ma.insert(key)),
            1=>assert_eq!(a.try_replace(key).unwrap(),ma.replace(key)),
            2=>assert_eq!(b.try_insert(key).unwrap(),mb.insert(key)),
            3=>assert_eq!(a.take(&key),ma.take(&key)),
            4=>assert_eq!(a.pop_first(),ma.pop_first()),
            5=>assert_eq!(a.pop_last(),ma.pop_last()),
            6=>{let mut t=a.try_split_off(&key).unwrap();let mut mt=ma.split_off(&key);assert!(t.iter().eq(mt.iter()));a.try_append(&mut t).unwrap();ma.append(&mut mt);},
            7=>{a.try_append(&mut b).unwrap();ma.append(&mut mb);},
            8=>{a.try_retain(|v|Ok::<_,()>(*v%3!=key%3)).unwrap();ma.retain(|v|*v%3!=key%3);},
            9=>{let(lo,hi)=(key.min(other),key.max(other));assert!(a.try_range(lo..=hi).unwrap().eq(ma.range(lo..=hi)));},
            10=>{let mut i=a.try_extract_if(..,|v|Ok::<_,()>(*v%3==key%3)).unwrap();let mut mi=ma.extract_if(..,|v|*v%3==key%3);for _ in 0..other%8{assert_eq!(i.try_next().unwrap(),mi.next());}},
            11=>assert!(a.try_clone().unwrap().iter().eq(ma.iter())),
            12=>{let(mut i,mut mi)=(a.try_clone().unwrap().into_iter(),ma.clone().into_iter());for _ in 0..other%8{assert_eq!(i.next(),mi.next());assert_eq!(i.next_back(),mi.next_back());}assert_eq!(i.len(),mi.len());},
            _=>{if key==0{a.clear();ma.clear();}},
        }
        assert_eq!(a.len(),ma.len());assert!(a.iter().eq(ma.iter()));assert!(b.iter().eq(mb.iter()));
        assert!(a.union(&b).eq(ma.union(&mb)));assert!(a.intersection(&b).eq(ma.intersection(&mb)));assert!(a.difference(&b).eq(ma.difference(&mb)));assert!(a.symmetric_difference(&b).eq(ma.symmetric_difference(&mb)));
        assert_eq!(a.is_subset(&b),ma.is_subset(&mb));assert_eq!(a.is_disjoint(&b),ma.is_disjoint(&mb));
    }
});
