//! Original staging control and owning leaves; no copied push algorithm.
use super::*;

#[kani::proof]
#[kani::unwind(4)]
fn staging_push_preserves_every_admitted_word_owner() {
    let len: usize = kani::any();
    let cap: usize = kani::any();
    let limit: usize = kani::any();
    kani::assume(len <= cap && cap <= limit);
    let mut owner =
        super::super::query_proofs::numeric_unit_owner(len, cap, LengthLimit::new(limit));
    let pointer = owner.ptr;
    let result = stage_push(&mut owner, ());
    if len < limit {
        assert!(result == Ok(owner.capacity()));
        assert!(owner.len() == len + 1);
        assert!(owner.len() <= owner.capacity() && owner.capacity() <= limit);
    } else {
        assert!(result == Err(Error::Capacity));
        assert!(owner.len() == len && owner.capacity() == cap);
    }
    assert!(owner.ptr == pointer && owner.max_len() == limit);
}

#[kani::proof]
#[kani::unwind(4)]
fn staging_controller_preserves_word_extents_and_source_order() {
    let limit: usize = kani::any();
    let cap: usize = kani::any();
    let early_end: bool = kani::any();
    let refuse: bool = kani::any();
    kani::assume(cap <= limit);
    let mut owner = super::super::query_proofs::numeric_unit_owner(0, cap, LengthLimit::new(limit));
    struct Source {
        calls: usize,
        early_end: bool,
    }
    impl TryIterator for Source {
        type Item = ();
        type Error = u8;
        fn try_next(&mut self) -> Result<Option<()>, u8> {
            self.calls += 1;
            if self.early_end {
                Ok(None)
            } else if self.calls == 2 {
                Err(23)
            } else {
                Ok(Some(()))
            }
        }
    }
    let mut source = Source {
        calls: 0,
        early_end,
    };
    let (result, count, capacity) =
        super::super::staging_driver::run((), limit, 0, cap, |index, capacity, value| {
            assert!(index == owner.len() && capacity == owner.capacity());
            assert!(index == source.calls);
            let reply = if refuse {
                Err(InitError::Storage(Error::Capacity))
            } else {
                stage_batch(&mut owner, &mut source, value)
            };
            (reply, owner.len(), owner.capacity())
        });
    assert!(count == owner.len() && capacity == owner.capacity());
    assert!(count == source.calls && count <= capacity && capacity <= limit);
    if limit == 0 {
        assert!(result == Ok(false) && count == 0);
    } else if refuse {
        assert!(result == Err(InitError::Storage(Error::Capacity)) && count == 0);
    } else if early_end {
        assert!(result == Ok(true) && count == 1);
    } else if limit == 1 {
        assert!(result == Ok(false) && count == 1);
    } else {
        assert!(result == Err(InitError::Initialize(23)) && count == 2);
    }
}

#[kani::proof]
#[kani::unwind(5)]
fn staging_push_preserves_every_three_byte_prefix() {
    let values: [u8; 3] = kani::any();
    let value: u8 = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= 3);
    let Ok(mut owner) = Vec::try_from_array_in(values, Unmetered, LengthLimit::new(3)) else {
        return;
    };
    owner.truncate(len);
    let result = stage_push(&mut owner, value);
    if len == 3 {
        assert!(result == Err(Error::Capacity) && owner.len() == 3);
    } else {
        assert!(result == Ok(3) && owner.len() == len + 1);
        assert!(owner.as_slice()[len] == value);
    }
    for index in 0..len {
        assert!(owner.as_slice()[index] == values[index]);
    }
}
