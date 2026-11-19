#![no_main]
use libfuzzer_sys::fuzz_target;
use jzap::collections::VecDeque;
use std::collections::VecDeque as Std;
fuzz_target!(|data:&[u8]|{
    let mut a=VecDeque::new();let mut b=Std::new();
    for chunk in data.chunks_exact(3){
        let value=u16::from_le_bytes([chunk[1],chunk[2]]);let at=value as usize%(a.len()+1);
        match chunk[0]%16 {
            0=>{a.try_push_back(value).unwrap();b.push_back(value)},
            1=>{a.try_push_front(value).unwrap();b.push_front(value)},
            2=>assert_eq!(a.pop_front(),b.pop_front()),3=>assert_eq!(a.pop_back(),b.pop_back()),
            4=>{a.try_insert(at,value).unwrap();b.insert(at,value)},5=>assert_eq!(a.remove(at),b.remove(at)),
            6=>{a.rotate_left(at).unwrap();b.rotate_left(at)},7=>{a.rotate_right(at).unwrap();b.rotate_right(at)},
            8=>{assert_eq!(a.make_contiguous(),b.make_contiguous());},9=>{a.try_shrink_to_fit().unwrap();b.shrink_to_fit()},
            10=>{a.try_reserve_exact(value as usize%32).unwrap();b.reserve_exact(value as usize%32)},
            11=>{a.try_retain(|v|Ok::<_,()>(v&3!=value&3)).unwrap();b.retain(|v|v&3!=value&3)},
            12=>{let mut c=a.try_split_off(at).unwrap();let mut d=b.split_off(at);a.try_append(&mut c).unwrap();b.append(&mut d)},
            13=>{a.truncate(at);b.truncate(at)},
            14=>{let end=at+(chunk[1]as usize%(a.len()-at+1));assert_eq!(a.drain(at..end).unwrap().collect::<Vec<_>>(),b.drain(at..end).collect::<Vec<_>>());},
            _=>{for(v,w)in a.iter_mut().zip(b.iter_mut()){*v=v.wrapping_add(value);*w=w.wrapping_add(value)}},
        }
        assert_eq!(a.iter().copied().collect::<Vec<_>>(),b.iter().copied().collect::<Vec<_>>());
    }
});
