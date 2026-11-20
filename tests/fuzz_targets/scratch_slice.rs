#![no_main]
use libfuzzer_sys::fuzz_target;
use jzap::{scratch::{Scratch,ScratchBuf},slice::SliceMut};
use std::mem::MaybeUninit;
fuzz_target!(|data: &[u8]| {
    let mut cells = [MaybeUninit::uninit();32];
    let mut a=Scratch::new(&mut cells);
    let mut b=ScratchBuf::try_with_capacity(32).unwrap();
    let mut model=Vec::with_capacity(32);
    for c in data.chunks_exact(4) {
        let n=c[1] as usize%36;
        match c[0]%7 {
            0=>{let fits=model.len()<32;assert_eq!(a.try_push(c[2]).is_ok(),fits);assert_eq!(b.try_push(c[2]).is_ok(),fits);if fits{model.push(c[2]);}},
            1=>{let x=model.pop();assert_eq!(a.pop(),x);assert_eq!(b.pop(),x);},
            2=>{a.truncate(n);b.truncate(n);model.truncate(n);},
            3=>{assert_eq!(a.try_resize(n,&c[2]).is_ok(),n<=32);assert_eq!(b.try_resize(n,&c[2]).is_ok(),n<=32);if n<=32{model.resize(n,c[2]);}},
            4=>{let x=[c[2],c[3]];let fits=model.len()<=30;assert_eq!(a.try_extend_copy(&x).is_ok(),fits);assert_eq!(b.try_extend_cloned(&x).is_ok(),fits);if fits{model.extend_from_slice(&x);}},
            5=>{a.clear();b.clear();model.clear();},
            _=>{
                let end=c[2] as usize%36;let dest=c[3] as usize%36;
                let fits=n<=end&&end<=model.len()&&dest<=model.len()&&end-n<=model.len()-dest;
                assert_eq!(SliceMut::new(a.as_mut_slice()).copy_within(n..end,dest).is_ok(),fits);
                assert_eq!(b.as_view_mut().copy_within(n..end,dest).is_ok(),fits);
                if fits{model.copy_within(n..end,dest);}
            }
        }
        assert_eq!(a.as_slice(),model);assert_eq!(b.as_slice(),model);
    }
});
