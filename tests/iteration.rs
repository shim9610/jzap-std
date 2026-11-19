use jzap::{
    TryDoubleEndedIterator, TryExactSizeIterator, TryIntoIterator, TryIterator,
    boxed::Box as B,
    budget::{Budget, MemoryBudget, Unmetered},
    error::InitError,
    iter::{TryFoldError, from_results, from_std},
    vec::Vec as V,
};
use std::cell::Cell;
#[allow(dead_code)]
mod common;
#[global_allocator]
static ALLOCATOR: common::FaultAllocator = common::FaultAllocator;

#[derive(Debug, PartialEq, Eq)]
enum SourceError {
    Convert,
    Hint,
    Next(usize),
}

// Deliberately no std Iterator impl: this is an external user's open impl.
#[derive(Debug)]
struct Source {
    index: usize,
    end: usize,
    fail: Option<usize>,
    hint_error: bool,
}
impl Source {
    fn new(end: usize, fail: Option<usize>) -> Self {
        Self {
            index: 0,
            end,
            fail,
            hint_error: false,
        }
    }
}
impl TryIterator for Source {
    type Item = usize;
    type Error = SourceError;
    fn try_next(&mut self) -> Result<Option<usize>, SourceError> {
        if self.fail == Some(self.index) {
            self.fail = None;
            return Err(SourceError::Next(self.index));
        }
        if self.index == self.end {
            return Ok(None);
        }
        let value = self.index;
        self.index += 1;
        Ok(Some(value))
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), SourceError> {
        if self.hint_error {
            Err(SourceError::Hint)
        } else {
            Ok((0, None))
        }
    }
}
struct ConversionFailure;
impl TryIntoIterator for ConversionFailure {
    type Item = usize;
    type Error = SourceError;
    type IntoIter = Source;
    fn try_into_iter(self) -> Result<Source, SourceError> {
        Err(SourceError::Convert)
    }
}

#[test]
fn open_implementation_flows_into_both_containers() {
    let value = V::try_from_fallible_iter(Source::new(5, None)).unwrap();
    assert_eq!(value.as_slice(), &[0, 1, 2, 3, 4]);
    let value = B::<[usize]>::try_from_fallible_iter_in(Source::new(5, None), Unmetered).unwrap();
    assert_eq!(value.as_slice(), &[0, 1, 2, 3, 4]);
}

#[test]
fn conversion_failure_preserves_existing_vector_and_splice_range() {
    let mut value = V::try_from_array([10, 20, 30]).unwrap();
    let pointer = value.as_ptr();
    assert_eq!(
        value.try_extend_fallible(ConversionFailure),
        Err(InitError::Initialize(SourceError::Convert))
    );
    assert!(matches!(
        value.try_splice_fallible(1..2, ConversionFailure),
        Err(InitError::Initialize(SourceError::Convert))
    ));
    assert_eq!(value.as_slice(), &[10, 20, 30]);
    assert_eq!(value.as_ptr(), pointer);
}

#[test]
fn source_failure_rolls_back_suffix_and_leaves_borrowed_source_resumable() {
    let mut value = V::try_from_array([10]).unwrap();
    let mut source = Source::new(5, Some(2));
    assert_eq!(
        value.try_extend_fallible(&mut source),
        Err(InitError::Initialize(SourceError::Next(2)))
    );
    assert_eq!(value.as_slice(), &[10]);
    value.try_extend_fallible(&mut source).unwrap();
    assert_eq!(value.as_slice(), &[10, 2, 3, 4]);
}

#[test]
fn hints_have_an_error_channel_but_vec_does_not_require_them() {
    let mut source = Source::new(3, None);
    source.hint_error = true;
    assert!(matches!(
        B::<[usize]>::try_from_fallible_iter_in(&mut source, Unmetered),
        Err(InitError::Initialize(SourceError::Hint))
    ));
    assert_eq!(source.index, 0);
    let value = V::try_from_fallible_iter(source).unwrap();
    assert_eq!(value.as_slice(), &[0, 1, 2]);
}

#[test]
fn iterator_and_fold_errors_remain_distinct_and_stop_at_first_failure() {
    let mut source = Source::new(5, Some(2));
    assert_eq!(
        source.try_fold(0, |sum, x| Ok::<_, &str>(sum + x)),
        Err(TryFoldError::Source(SourceError::Next(2)))
    );
    assert_eq!(source.index, 2);
    assert_eq!(
        source.try_for_each(|x| if x == 3 { Err("callback") } else { Ok(()) }),
        Err(TryFoldError::Operation("callback"))
    );
    assert_eq!(source.index, 4);
    assert_eq!(source.try_next(), Ok(Some(4)));
}

#[test]
fn take_zero_does_not_call_source_and_errors_consume_one_attempt() {
    let mut source = Source::new(4, Some(0));
    {
        let mut empty = source.try_by_ref().try_take(0);
        assert_eq!(empty.try_size_hint(), Ok((0, Some(0))));
        assert_eq!(empty.try_next(), Ok(None));
    }
    assert_eq!(source.fail, Some(0));
    let mut limited = source.try_take(2);
    assert_eq!(limited.try_next(), Err(SourceError::Next(0)));
    assert_eq!(limited.try_next(), Ok(Some(0)));
    assert_eq!(limited.try_next(), Ok(None));
}

#[test]
fn nth_and_both_ends_match_std() {
    let mut borrowed = [1u8, 2].iter();
    assert_eq!(
        TryIterator::try_fold(&mut borrowed, 0, |sum, item| Ok::<_, ()>(sum + item)),
        Ok(3)
    );
    let mut values = from_std([10, 20, 30, 40]);
    assert_eq!(values.try_len(), Ok(4));
    assert_eq!(values.try_nth(1), Ok(Some(20)));
    assert_eq!(values.try_next_back(), Ok(Some(40)));
    assert_eq!(values.try_nth_back(0), Ok(Some(30)));
    assert_eq!(values.try_is_empty(), Ok(true));
    assert_eq!(values.try_nth(usize::MAX), Ok(None));
    let mut source = Source::new(3, Some(1));
    assert_eq!(source.try_nth(2), Err(SourceError::Next(1)));
    assert_eq!(source.try_next(), Ok(Some(1)));
}

#[test]
fn result_adapter_keeps_source_errors_and_has_no_implicit_fusing() {
    let mut source = from_results([Ok(1), Err("source"), Ok(3)]);
    assert_eq!(
        source.try_fold(0, |a, b| Ok::<_, ()>(a + b)),
        Err(TryFoldError::Source("source"))
    );
    assert_eq!(source.try_next(), Ok(Some(3)));
    assert_eq!(source.try_next(), Ok(None));
}

#[test]
fn container_owners_borrows_drains_and_extractors_use_mirrors() {
    let mut value = V::try_from_fallible_iter([1, 2, 3]).unwrap();
    {
        let mut iter = (&mut value).try_into_iter().unwrap();
        *iter.try_next().unwrap().unwrap() = 9;
        assert_eq!(iter.try_len(), Ok(2));
    }
    let mut drain = value.try_drain(1..).unwrap();
    assert_eq!(drain.try_next_back(), Ok(Some(3)));
    assert_eq!(drain.try_next(), Ok(Some(2)));
    drop(drain);
    let mut extract = value
        .try_extract_if(.., |_| Err::<bool, _>("filter"))
        .unwrap();
    assert_eq!(extract.try_next(), Err("filter"));
    assert_eq!(extract.try_next(), Ok(None));
    drop(extract);
    let boxed = B::<[i32]>::try_from_fallible_iter_in(value, Unmetered).unwrap();
    let copy = V::try_from_fallible_iter(boxed).unwrap();
    assert_eq!(copy.as_slice(), &[9]);
    let mut dynamic: B<dyn TryIterator<Item = usize, Error = SourceError>> = jzap::box_unsize!(B::try_new(Source::new(2, None)).unwrap() => dyn TryIterator<Item = usize, Error = SourceError>);
    assert_eq!(dynamic.try_next(), Ok(Some(0)));
}

#[test]
fn splice_failures_restore_tail_at_every_insertion_stage() {
    for unknown in [false, true] {
        for fail in 0..12 {
            struct Hints(Source, bool);
            impl TryIterator for Hints {
                type Item = usize;
                type Error = SourceError;
                fn try_next(&mut self) -> Result<Option<usize>, SourceError> {
                    self.0.try_next()
                }
                fn try_size_hint(&self) -> Result<(usize, Option<usize>), SourceError> {
                    if self.1 {
                        Ok((0, None))
                    } else {
                        Ok((12 - self.0.index, Some(12 - self.0.index)))
                    }
                }
            }
            let mut value = V::try_from_array([100, 200, 300, 400]).unwrap();
            let result = value
                .try_splice_fallible(1..3, Hints(Source::new(12, Some(fail)), unknown))
                .unwrap()
                .try_finish();
            assert_eq!(result, Err(InitError::Initialize(SourceError::Next(fail))));
            assert_eq!(value.first(), Some(&100));
            assert_eq!(value.last(), Some(&400));
            value.try_push(500).unwrap();
        }
    }
}

#[test]
fn splice_batch_error_drops_only_uncommitted_values_once() {
    struct Value<'a> {
        id: usize,
        drops: &'a [Cell<usize>],
    }
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.drops[self.id].set(self.drops[self.id].get() + 1);
        }
    }

    let drops = std::array::from_fn::<_, 13, _>(|_| Cell::new(0));
    let drop_count = |id: usize| drops[id].get();
    let make = |id| Value { id, drops: &drops };
    let mut owner = V::try_with_capacity(4).unwrap();
    for id in 0..4 {
        assert!(owner.try_push(make(id)).is_ok());
    }

    let mut next = 0;
    let replacement = std::iter::from_fn(|| {
        let index = next;
        next += 1;
        Some(if index == 9 {
            Err(SourceError::Next(index))
        } else {
            Ok(make(4 + index))
        })
    });
    let result = owner
        .try_splice_fallible(1..3, from_results(replacement))
        .unwrap()
        .try_finish();
    assert_eq!(result, Err(InitError::Initialize(SourceError::Next(9))));
    assert_eq!(next, 10);
    assert!(
        owner
            .iter()
            .map(|value| value.id)
            .eq([0, 4, 5, 6, 7, 8, 9, 3])
    );
    for id in 0..13 {
        assert_eq!(drop_count(id), usize::from(matches!(id, 1 | 2 | 10..=12)));
    }
    drop(owner);
    assert!((0..13).all(|id| drop_count(id) == 1));
}

#[test]
fn dropping_splice_never_calls_replacement() {
    let mut source = Source::new(4, Some(0));
    let mut value = V::try_from_array([10, 20, 30]).unwrap();
    drop(value.try_splice_fallible(1..2, &mut source).unwrap());
    assert_eq!(value.as_slice(), &[10, 30]);
    assert_eq!(source.index, 0);
    assert_eq!(source.fail, Some(0));
}

#[test]
fn fallible_utf8_character_and_chunk_sources_propagate_errors() {
    let value =
        B::<str>::try_from_fallible_chars_in(from_std(['a', '한', '🦀']), Unmetered).unwrap();
    assert_eq!(value.as_str(), "a한🦀");
    assert!(matches!(
        B::<str>::try_from_fallible_chars_in(from_results([Ok('한'), Err(7)]), Unmetered),
        Err(InitError::Initialize(7))
    ));
    assert!(matches!(
        B::<str>::try_from_fallible_texts_in(from_results([Ok("a"), Err(9)]), Unmetered),
        Err(InitError::Initialize(9))
    ));
}

#[test]
fn refusal_and_partial_source_errors_release_all_storage_and_budget() {
    for fail in 1..8 {
        let budget = MemoryBudget::new(2048);
        let (_, state) = common::track(fail, || {
            drop(B::<[usize], _>::try_from_fallible_iter_in(
                Source::new(20, Some(17)),
                Budget::Limited(&budget),
            ));
        });
        assert_eq!(state.live, 0);
        assert_eq!(budget.occupied(), 0);
        assert_eq!(budget.reserved(), 0);
    }
    let drops = Cell::new(0);
    struct Payload<'a>(&'a Cell<usize>);
    impl Drop for Payload<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let mut value = V::new();
    value
        .try_push(Payload(&drops))
        .unwrap_or_else(|_| unreachable!());
    let result = value.try_extend_fallible(from_results([
        Ok(Payload(&drops)),
        Ok(Payload(&drops)),
        Err(17),
    ]));
    assert_eq!(result, Err(InitError::Initialize(17)));
    assert_eq!(drops.get(), 2);
    drop(value);
    assert_eq!(drops.get(), 3);
}
