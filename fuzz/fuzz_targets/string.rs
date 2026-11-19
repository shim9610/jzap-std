#![no_main]
use jzap::{
    budget::{Budget, MemoryBudget},
    string::String as S,
};
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    let repaired = S::try_from_utf8_lossy(data).unwrap();
    assert_eq!(repaired.as_str(), String::from_utf8_lossy(data));
    let limit = data.first().copied().unwrap_or(0) as usize;
    let budget = MemoryBudget::new(limit.saturating_mul(4));
    let mut ours = S::with_limit_in(limit, Budget::Limited(&budget));
    let mut model = String::new();
    for bytes in data.chunks(3).take(128) {
        let a = bytes.first().copied().unwrap_or(0);
        let b = bytes.get(1).copied().unwrap_or(0);
        let ch = ['a', 'é', '한', '😀'][b as usize % 4];
        let boundaries: Vec<_> = model
            .char_indices()
            .map(|(i, _)| i)
            .chain([model.len()])
            .collect();
        let at = boundaries[a as usize % boundaries.len()];
        match a % 7 {
            0 => {
                if ours.try_push(ch).is_ok() {
                    model.push(ch);
                }
            }
            1 => {
                if ours.try_insert(at, ch).is_ok() {
                    model.insert(at, ch);
                }
            }
            2 => {
                if at < model.len() {
                    assert_eq!(ours.remove(at).unwrap(), model.remove(at));
                }
            }
            3 => {
                ours.truncate(at).unwrap();
                model.truncate(at);
            }
            4 => {
                ours.retain(|c| c != ch);
                model.retain(|c| c != ch);
            }
            5 => {
                assert_eq!(ours.pop(), model.pop());
            }
            _ => {
                let mut encoded = [0; 4];
                let text = ch.encode_utf8(&mut encoded);
                if ours.try_replace_range(at.., text).is_ok() {
                    model.replace_range(at.., text);
                }
            }
        }
        assert_eq!(ours.as_str(), model);
        assert!(ours.len() <= limit);
        assert!(ours.capacity() <= limit);
        assert_eq!(budget.reserved(), 0);
    }
    drop(ours);
    assert_eq!(budget.occupied(), 0);
});
