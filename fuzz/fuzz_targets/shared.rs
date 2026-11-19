#![no_main]
use jzap::{
    TryClone,
    budget::{AllocationPolicy, Budget, MemoryBudget, OwnedBudget, OwnedConcurrentBudget},
    shared::{
        Strong,
        count::{AtomicCount, Counter, LocalCount},
    },
};
use libfuzzer_sys::fuzz_target;
fn exercise<C: Counter, P: AllocationPolicy>(
    input: &[u8],
    limit: usize,
    policy: P,
    query: impl Fn() -> (usize, usize, usize),
) {
    let Ok(a) = Strong::<u64, C, P>::try_new_in(17, policy) else {
        assert_eq!(query().1, 0);
        return;
    };
    let mut actual = Some(a);
    let mut model = Some(std::rc::Rc::new(17u64));
    let mut aliases = Vec::new();
    let mut refs = Vec::new();
    let mut weaks = Vec::new();
    let mut weak_refs = Vec::new();
    for &op in input.iter().take(128) {
        match op % 8 {
            0 if aliases.len() < 4 => {
                if let (Some(a), Some(m)) = (&actual, &model) {
                    aliases.push(Strong::try_clone(a).unwrap());
                    refs.push(m.clone());
                }
            }
            1 => {
                aliases.pop();
                refs.pop();
            }
            2 if weaks.len() < 4 => {
                if let (Some(a), Some(m)) = (&actual, &model) {
                    weaks.push(Strong::try_downgrade(a).unwrap());
                    weak_refs.push(std::rc::Rc::downgrade(m));
                }
            }
            3 => {
                weaks.pop();
                weak_refs.pop();
            }
            4 => {
                if let (Some(a), Some(m)) = (&mut actual, &mut model) {
                    if let Ok(value) = Strong::<u64, C, _>::try_make_mut(a) {
                        *value = value.wrapping_add(1);
                        let v = std::rc::Rc::make_mut(m);
                        *v = v.wrapping_add(1);
                    }
                }
            }
            5 => {
                for (w, r) in weaks.iter().zip(&weak_refs) {
                    assert_eq!(
                        w.try_upgrade().unwrap().map(|a| *a),
                        r.upgrade().map(|a| *a)
                    );
                }
            }
            6 => {
                if let (Some(a), Some(m)) = (&actual, &model) {
                    assert_eq!(Strong::strong_count(a), std::rc::Rc::strong_count(m));
                    assert_eq!(Strong::weak_count(a), std::rc::Rc::weak_count(m));
                }
            }
            7 => {
                actual.take();
                model.take();
            }
            _ => {}
        };
        assert_eq!(query().0.checked_add(query().1), Some(limit));
        assert_eq!(query().2, 0);
    }
    drop((actual, aliases, weaks));
    assert_eq!(query().1, 0);
}
fuzz_target!(|input: &[u8]| {
    let limit = input.first().copied().unwrap_or(0) as usize * 8;
    let local = MemoryBudget::new(limit);
    exercise::<LocalCount, _>(input, limit, Budget::Limited(&local), || {
        (local.available(), local.occupied(), local.reserved())
    });
    exercise::<AtomicCount, _>(input, limit, Budget::Limited(&local), || {
        (local.available(), local.occupied(), local.reserved())
    });
    if let Ok(owned) = OwnedBudget::try_new(limit) {
        exercise::<LocalCount, _>(input, limit, owned.try_clone().unwrap(), || {
            (owned.available(), owned.occupied(), owned.reserved())
        });
    }
    if let Ok(owned) = OwnedConcurrentBudget::try_new(limit) {
        exercise::<AtomicCount, _>(input, limit, owned.try_clone().unwrap(), || {
            (owned.available(), owned.used(), 0)
        });
    }
});
