mod common;
#[path = "common/deallocations.rs"]
mod deallocations;
use common::{FaultAllocator, track};
use jzap::{
    Error, TryClone, TryCloneIn,
    boxed::Box as B,
    budget::{Budget, MemoryBudget},
    error::InitError,
};
use std::{
    cell::Cell,
    future::Future,
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll, Waker},
};
#[global_allocator]
static ALLOC: FaultAllocator = FaultAllocator;

#[test]
fn slice_every_partial_failure_is_cleaned_and_budget_restored() {
    struct V<'a>(&'a Cell<usize>);
    impl Drop for V<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for len in 0..9 {
        for fail in 0..=len {
            for mode in 0..3 {
                let drops = Cell::new(0);
                let budget = MemoryBudget::new(1024);
                let (_, stats) = track(usize::MAX, || {
                    let result = B::<[V<'_>], _>::try_init_slice_in(
                        len,
                        Budget::Limited(&budget),
                        |i, slot| {
                            if i == fail && mode == 0 {
                                return Err(Error::Capacity);
                            }
                            if i == fail && mode == 2 {
                                return Ok(());
                            }
                            slot.write(V(&drops)).map_err(|e| e.error)?;
                            if i == fail {
                                return Err(Error::Capacity);
                            }
                            Ok(())
                        },
                    );
                    if fail < len {
                        assert!(result.is_err());
                        assert_eq!(drops.get(), fail + usize::from(mode == 1));
                    } else {
                        assert_eq!(result.as_ref().unwrap().len(), len);
                        drop(result);
                        assert_eq!(drops.get(), len);
                    }
                });
                assert_eq!(stats.live, 0);
                assert_eq!(budget.occupied(), 0);
                assert_eq!(budget.reserved(), 0);
            }
        }
    }
}
#[test]
fn slice_refusal_layout_and_zst() {
    let calls = Cell::new(0);
    let (r, s) = track(1, || {
        B::<[u64]>::try_init_slice(8, |_, slot| {
            calls.set(calls.get() + 1);
            slot.write(0).map_err(|e| e.error)
        })
    });
    assert!(matches!(
        r,
        Err(InitError::Storage(Error::Allocation { .. }))
    ));
    assert_eq!(s.calls, 1);
    assert_eq!(calls.get(), 0);
    let (r, s) = track(1, || B::<[u64]>::try_new_uninit_slice(usize::MAX));
    assert!(matches!(r, Err(Error::Layout)));
    assert_eq!(s.calls, 0);
    #[repr(align(4096))]
    struct Z;
    let (_, s) = track(1, || {
        let z = B::<[Z]>::try_init_slice(4, |_, slot| slot.write(Z).map_err(|e| e.error)).unwrap();
        assert_eq!(B::as_ptr(&z).cast::<Z>().addr() % 4096, 0);
        assert_eq!(z.len(), 4);
        drop(z);
        let empty = B::<[Z]>::empty();
        assert!(empty.is_empty());
        drop(empty);
    });
    assert_eq!((s.calls, s.live), (0, 0));
}
#[test]
fn slice_clone_failure_at_every_allocation_and_output_layout() {
    let source = B::<[B<u8>]>::try_init_slice(4, |i, slot| {
        slot.write(B::try_new(i as u8).unwrap())
            .map_err(|e| e.error)
    })
    .unwrap();
    for fail in 1..=5 {
        let budget = MemoryBudget::new(1024);
        let (_, s) = track(fail, || {
            assert!(source.try_clone_in(Budget::Limited(&budget)).is_err())
        });
        assert_eq!(s.live, 0);
        assert_eq!(budget.occupied(), 0);
        assert_eq!(source.iter().map(|x| **x).collect::<Vec<_>>(), [0, 1, 2, 3]);
    }
    let budget = MemoryBudget::new(4 * std::mem::size_of::<B<u8, Budget<'_>>>() + 4);
    let clone = source.try_clone_in(Budget::Limited(&budget)).unwrap();
    assert_eq!(budget.available(), 0);
    assert_eq!(
        budget.occupied(),
        4 * std::mem::size_of::<B<u8, Budget<'_>>>() + 4
    );
    assert_eq!(clone.iter().map(|x| **x).collect::<Vec<_>>(), [0, 1, 2, 3]);
    drop(clone);
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn strings_validate_and_return_input_without_allocating() {
    let budget = MemoryBudget::new(100);
    let ((), s) = track(usize::MAX, || {
        let text = B::<str, _>::try_from_str_in("가나다", Budget::Limited(&budget)).unwrap();
        assert_eq!(text.get(0..3).unwrap(), "가");
        assert!(text.get(1..3).is_err());
        let ptr = B::as_ptr(&text).cast::<u8>();
        let bytes = text.into_bytes();
        let text = B::<str, _>::try_from_utf8(bytes).unwrap();
        assert_eq!(B::as_ptr(&text).cast::<u8>(), ptr);
        let clone = text.try_clone().unwrap();
        assert_eq!(clone.as_str(), text.as_str());
        drop(clone);
        drop(text);
        let invalid =
            B::<[u8], _>::try_copy_from_slice_in(&[255, 1], Budget::Limited(&budget)).unwrap();
        let ptr = B::as_ptr(&invalid);
        let rejected = B::<str, _>::try_from_utf8(invalid).unwrap_err();
        assert_eq!(B::as_ptr(&rejected.value), ptr);
        assert_eq!(rejected.error.valid_up_to(), 0);
        drop(rejected);
    });
    assert_eq!((s.calls, s.live, budget.occupied()), (3, 0, 0));
}
#[test]
fn raw_array_slice_and_standard_transfers_preserve_allocation() {
    let budget = MemoryBudget::new(100);
    let ((), s) = track(usize::MAX, || {
        let value = B::try_new_in([1u32, 2, 3], Budget::Limited(&budget)).unwrap();
        let ptr = B::as_ptr(&value).cast::<u32>();
        let slice = value.into_slice();
        let slice = slice.try_into_array::<4>().unwrap_err();
        let array = slice.try_into_array::<3>().unwrap();
        assert_eq!(B::as_ptr(&array).cast::<u32>(), ptr);
        let (raw, policy) = B::into_raw_parts(array);
        let array = unsafe { B::from_raw_parts(raw, policy) };
        assert_eq!(B::into_inner(array), [1, 2, 3]);
    });
    assert_eq!((s.calls, s.live, budget.occupied()), (1, 0, 0));
    let value = B::try_new(41u64).unwrap();
    let ptr = B::as_ptr(&value);
    let standard = B::into_std(value);
    assert_eq!(&*standard as *const _, ptr);
    let value = B::from_std(standard);
    let (old, storage) = B::take(value);
    assert_eq!(old, 41);
    let value = B::<MaybeUninit<u64>>::write(storage, 42);
    assert_eq!(B::as_ptr(&value), ptr);
    assert_eq!(*value, 42);
    let vec = B::<[u32]>::try_copy_from_slice(&[1, 2, 3])
        .unwrap()
        .into_std_vec();
    assert_eq!(vec, [1, 2, 3]);
    let string = B::<str>::try_from_str("hello").unwrap().into_std_string();
    assert_eq!(string, "hello");
}
#[test]
fn owned_iteration_drops_only_remaining_values() {
    struct V<'a>(&'a Cell<usize>);
    impl Drop for V<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1)
        }
    }
    for front in 0..=5 {
        for back in 0..=5 - front {
            let drops = Cell::new(0);
            let budget = MemoryBudget::new(100);
            let ((), s) = track(usize::MAX, || {
                let boxed =
                    B::<[V<'_>], _>::try_init_slice_in(5, Budget::Limited(&budget), |_, slot| {
                        slot.write(V(&drops)).map_err(|e| e.error)
                    })
                    .unwrap();
                let mut iter = boxed.into_iter();
                for _ in 0..front {
                    drop(iter.next().unwrap());
                }
                for _ in 0..back {
                    drop(iter.next_back().unwrap());
                }
                assert_eq!(iter.len(), 5 - front - back);
                assert_eq!(drops.get(), front + back);
                drop(iter);
            });
            assert_eq!((drops.get(), s.live, budget.occupied()), (5, 0, 0));
        }
    }
}
#[test]
fn uninitialized_and_zeroed_storage() {
    let ((), s) = track(usize::MAX, || {
        let slot = B::<u64>::try_new_zeroed().unwrap();
        let slot = unsafe { B::<MaybeUninit<u64>>::assume_init(slot) };
        assert_eq!(*slot, 0);
        drop(slot);
        let mut slice = B::<[u64]>::try_new_uninit_slice(4).unwrap();
        for (i, v) in slice.iter_mut().enumerate() {
            v.write(i as u64);
        }
        let slice = unsafe { B::<[MaybeUninit<u64>]>::assume_init(slice) };
        assert_eq!(slice.as_slice(), [0, 1, 2, 3]);
        drop(slice);
        let slice = B::<[u64]>::try_new_zeroed_slice(4).unwrap();
        let slice = unsafe { B::<[MaybeUninit<u64>]>::assume_init(slice) };
        assert_eq!(slice.as_slice(), [0; 4]);
        drop(slice);
    });
    assert_eq!((s.calls, s.live), (3, 0));
}
#[test]
fn unsizing_downcast_and_pinning_do_not_allocate() {
    #[derive(Debug)]
    struct Address {
        first: Cell<usize>,
        _pin: std::marker::PhantomPinned,
    }
    impl Future for Address {
        type Output = usize;
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<usize> {
            let ptr = self.as_ref().get_ref() as *const Self as usize;
            if self.first.get() == 0 {
                self.first.set(ptr);
                Poll::Pending
            } else {
                assert_eq!(self.first.get(), ptr);
                Poll::Ready(ptr)
            }
        }
    }
    let budget = MemoryBudget::new(1024);
    let boxed = B::try_new_in(42u64, Budget::Limited(&budget)).unwrap();
    let ptr = B::as_ptr(&boxed);
    let ((), s) = track(1, || {
        let erased = jzap::box_unsize!(boxed => dyn std::any::Any + Send + Sync);
        let erased = erased.downcast::<u32>().unwrap_err();
        let restored = erased.downcast::<u64>().unwrap();
        assert_eq!(B::as_ptr(&restored), ptr);
        assert_eq!(*restored, 42);
        drop(restored);
    });
    assert_eq!(s.calls, 0);
    assert_eq!(budget.occupied(), 0);
    let mut pinned = B::try_pin(Address {
        first: Cell::new(0),
        _pin: std::marker::PhantomPinned,
    })
    .unwrap();
    let mut cx = Context::from_waker(Waker::noop());
    assert!(Pin::new(&mut pinned).poll(&mut cx).is_pending());
    let mut moved = pinned;
    assert!(Pin::new(&mut moved).poll(&mut cx).is_ready());
    let stdpin = moved.into_std();
    let mut restored = jzap::boxed::Pinned::from_std(stdpin);
    assert!(Pin::new(&mut restored).poll(&mut cx).is_ready());
}

#[test]
fn named_fields_initialize_in_place_and_clean_all_partial_results() {
    use jzap::init::InitSlot;
    struct V<'a>(&'a Cell<usize>);
    impl Drop for V<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    struct Record<'a> {
        a: V<'a>,
        b: V<'a>,
        c: V<'a>,
    }
    for failed in 0..=3 {
        for after_write in [false, true] {
            let drops = Cell::new(0);
            let budget = MemoryBudget::new(100);
            fn init<'a>(
                i: usize,
                failed: usize,
                after_write: bool,
                drops: &'a Cell<usize>,
                slot: &mut InitSlot<'_, V<'a>>,
            ) -> Result<(), Error> {
                if failed == i && !after_write {
                    return Err(Error::Capacity);
                }
                slot.write(V(drops)).map_err(|e| e.error)?;
                if failed == i {
                    return Err(Error::Capacity);
                }
                Ok(())
            }
            let ((), s) = track(usize::MAX, || {
                let result = B::<Record<'_>, _>::try_init_in(
                    Budget::Limited(&budget),
                    |slot: &mut InitSlot<'_, Record<'_>>| {
                        jzap::try_init_struct!(slot, Record<'_> {
                            a: |slot: &mut InitSlot<'_, _>| init(0,failed,after_write,&drops,slot),
                            b: |slot: &mut InitSlot<'_, _>| init(1,failed,after_write,&drops,slot),
                            c: |slot: &mut InitSlot<'_, _>| init(2,failed,after_write,&drops,slot),
                        })
                    },
                );
                if failed < 3 {
                    assert!(result.is_err());
                    assert_eq!(drops.get(), failed + usize::from(after_write));
                } else {
                    let record = result.unwrap();
                    assert!(std::ptr::eq(record.a.0, record.b.0));
                    assert!(std::ptr::eq(record.a.0, record.c.0));
                    drop(record);
                    assert_eq!(drops.get(), 3);
                }
            });
            assert_eq!((s.live, budget.occupied()), (0, 0));
        }
    }
}
#[test]
fn boxed_callable_consumes_captures_and_releases_after_unwind() {
    let budget = MemoryBudget::new(1024);
    let capture = B::try_new(7u64).unwrap();
    let callback = B::try_new_in(
        move |a: u64, b: u64| *capture + a + b,
        Budget::Limited(&budget),
    )
    .ok()
    .unwrap();
    let callback = jzap::box_unsize!(callback => dyn FnOnce(u64,u64)->u64);
    let (result, s) = track(1, || jzap::box_call_once!(callback; 2,3));
    assert_eq!(result, 12);
    assert_eq!(s.calls, 0);
    assert_eq!(budget.occupied(), 0);
    let value = Cell::new(0);
    let callback = B::try_new_in(
        || {
            value.set(1);
            panic!("callback")
        },
        Budget::Limited(&budget),
    )
    .ok()
    .unwrap();
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || jzap::box_call_once!(callback;),
    ));
    assert!(failed.is_err());
    assert_eq!(value.get(), 1);
    assert_eq!(budget.occupied(), 0);
}

#[test]
fn missing_field_write_is_rejected_and_completed_fields_are_dropped() {
    use jzap::init::InitSlot;
    struct V<'a>(&'a Cell<usize>);
    impl Drop for V<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1)
        }
    }
    struct S<'a> {
        a: V<'a>,
        b: u64,
    }
    let drops = Cell::new(0);
    let (_, s) = track(usize::MAX, || {
        let result =
            B::<S<'_>>::try_init_in(jzap::budget::Unmetered, |slot: &mut InitSlot<'_, S<'_>>| {
                jzap::try_init_struct!(slot,S<'_>{
                    a: |field:&mut InitSlot<'_,_>|field.write(V(&drops)).map_err(|e|e.error),
                    b: |_field:&mut InitSlot<'_,u64>|Ok::<_,Error>(()),
                })
            });
        assert!(matches!(
            result,
            Err(InitError::Initialize(InitError::Storage(
                Error::Uninitialized
            )))
        ));
    });
    assert_eq!((drops.get(), s.live), (1, 0));
}
#[test]
fn dynamic_drop_unwind_always_releases_storage_and_charge() {
    struct Panics;
    impl Drop for Panics {
        fn drop(&mut self) {
            panic!("drop")
        }
    }
    let budget = MemoryBudget::new(100);
    // Non-ZST outer value, erased through a custom trait.
    trait Marker {}
    impl Marker for (u64, Panics) {}
    let boxed = B::try_new_in((1u64, Panics), Budget::Limited(&budget))
        .ok()
        .unwrap();
    let erased = jzap::box_unsize!(boxed => dyn Marker);
    let address = B::as_ptr(&erased).cast::<u8>().addr();
    let (_, freed) = deallocations::deallocations_during(address, || {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(erased))).is_err());
    });
    assert_eq!(freed, 1);
    assert_eq!(budget.occupied(), 0);
}

#[test]
fn collection_growth_and_shrink_failures_clean_consumed_values() {
    struct V<'a>(&'a Cell<usize>);
    impl Drop for V<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1)
        }
    }
    for fail in 1..=6 {
        let created = Cell::new(0);
        let dropped = Cell::new(0);
        let budget = MemoryBudget::new(4096);
        let (_, s) = track(fail, || {
            let iterator = (0..7).filter(|_| true).map(|_| {
                created.set(created.get() + 1);
                V(&dropped)
            });
            let result = B::<[V<'_>], _>::try_from_iter_in(iterator, Budget::Limited(&budget));
            if fail <= 5 {
                assert!(result.is_err());
            } else {
                assert_eq!(result.as_ref().unwrap().len(), 7);
            }
            drop(result);
        });
        assert_eq!(created.get(), dropped.get());
        assert_eq!((s.live, budget.occupied(), budget.reserved()), (0, 0, 0));
    }
    let budget = MemoryBudget::new(32);
    assert!(matches!(
        B::<[u64], _>::try_from_iter_in((0..4).filter(|_| true), Budget::Limited(&budget)),
        Err(Error::Budget { .. })
    ));
    assert_eq!(budget.occupied(), 0);
    let budget = MemoryBudget::new(100);
    let result = B::<[u64], _>::try_from_fallible_iter_in(
        jzap::iter::from_results((0..8).map(|i| if i == 3 { Err("stop") } else { Ok(i) })),
        Budget::Limited(&budget),
    );
    assert!(matches!(result, Err(InitError::Initialize("stop"))));
    assert_eq!(budget.occupied(), 0);
}
#[test]
fn standard_owner_adoption_returns_original_on_budget_or_realloc_failure() {
    for fail in [0, 1] {
        let mut value = Vec::with_capacity(16);
        value.extend_from_slice(&[1u64, 2, 3]);
        let ptr = value.as_ptr();
        let cap = value.capacity();
        let budget = MemoryBudget::new(if fail == 0 { 1 } else { 1024 });
        let (rejected, s) = track(1, || {
            B::<[u64], _>::try_from_std_vec_in(value, Budget::Limited(&budget)).unwrap_err()
        });
        assert_eq!(rejected.value.as_ptr(), ptr);
        assert_eq!(rejected.value.capacity(), cap);
        assert_eq!(rejected.value, [1, 2, 3]);
        assert_eq!(s.calls, fail);
        assert_eq!(budget.occupied(), 0);
        assert_eq!(budget.reserved(), 0);
    }
    let source = vec![1u64, 2, 3];
    let ptr = source.as_ptr();
    let budget = MemoryBudget::new(24);
    let (owned, s) = track(1, || {
        B::<[u64], _>::try_from_std_vec_in(source, Budget::Limited(&budget)).unwrap()
    });
    assert_eq!(s.calls, 0);
    assert_eq!(B::as_ptr(&owned).cast::<u64>(), ptr);
    assert_eq!(budget.occupied(), 24);
    drop(owned);
    assert_eq!(budget.occupied(), 0);
    let mut text = String::with_capacity(100);
    text.push_str("가나다");
    let ptr = text.as_ptr();
    let cap = text.capacity();
    let (rejected, _) = track(1, || {
        B::<str, _>::try_from_std_string_in(text, jzap::budget::Unmetered).unwrap_err()
    });
    assert_eq!(rejected.value, "가나다");
    assert_eq!(rejected.value.as_ptr(), ptr);
    assert_eq!(rejected.value.capacity(), cap);
}
#[test]
fn replacement_clone_preserves_destination_on_failure_and_its_budget() {
    let budget = MemoryBudget::new(64);
    let mut dest = B::try_new_in(1u64, Budget::Limited(&budget)).unwrap();
    let source = B::try_new_in(2u64, Budget::Unmetered).unwrap();
    let ptr = B::as_ptr(&dest);
    let (result, stats) = track(1, || dest.try_clone_from(&source));
    assert!(result.is_ok());
    assert_eq!(stats.calls, 0);
    assert_eq!(*dest, 2);
    assert_eq!(B::as_ptr(&dest), ptr);
    assert_eq!(budget.occupied(), 8);
    drop(dest);
    assert_eq!(budget.occupied(), 0);
    let budget = MemoryBudget::new(4096);
    #[derive(Debug)]
    struct Fallible([u64; 128]);
    impl TryClone for Fallible {
        fn try_clone(&self) -> Result<Self, Error> {
            Ok(Self(self.0))
        }
    }
    let mut dest = B::try_new_in(Fallible([1; 128]), Budget::Limited(&budget)).unwrap();
    let source = B::try_new_in(Fallible([2; 128]), Budget::Unmetered).unwrap();
    let ptr = B::as_ptr(&dest);
    let (result, _) = track(1, || dest.try_clone_from(&source));
    assert!(result.is_err());
    assert_eq!(dest.0, [1; 128]);
    assert_eq!(B::as_ptr(&dest), ptr);
    assert_eq!(budget.occupied(), 1024);
    dest.try_clone_from(&source).unwrap();
    assert_eq!(dest.0, [2; 128]);
    drop(dest);
    assert_eq!(budget.occupied(), 0);
    let mut dest = B::<[u32]>::try_copy_from_slice(&[1, 2]).unwrap();
    let source = B::<[u32]>::try_copy_from_slice(&[3, 4]).unwrap();
    let ptr = B::as_ptr(&dest);
    let (result, stats) = track(1, || dest.try_clone_from(&source));
    assert!(result.is_ok());
    assert_eq!(stats.calls, 0);
    assert_eq!(B::as_ptr(&dest), ptr);
    assert_eq!(dest.as_slice(), source.as_slice());
    let mut text = B::<str>::try_from_str("abc").unwrap();
    let source = B::<str>::try_from_str("xyz").unwrap();
    let ptr = B::as_ptr(&text);
    let (result, s) = track(1, || text.try_clone_from(&source));
    assert!(result.is_ok());
    assert_eq!(s.calls, 0);
    assert_eq!(B::as_ptr(&text), ptr);
    assert_eq!(text.as_str(), "xyz");
}
#[test]
fn defaults_and_pinned_trait_object_preserve_failure_and_address() {
    use jzap::TryDefault;
    let value = B::<[[u64; 4]; 3]>::try_default().unwrap();
    assert_eq!(*value, [[0; 4]; 3]);
    let (result, s) = track(1, || B::<u64>::try_default());
    assert!(result.is_err());
    assert_eq!(s.calls, 1);
    let value = B::try_pin(async { 42u64 }).ok().unwrap();
    let mut erased = jzap::pin_box_unsize!(value => dyn Future<Output=u64>);
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(Pin::new(&mut erased).poll(&mut cx), Poll::Ready(42));
}

#[test]
fn text_collection_observes_each_chunk_once() {
    struct Changing(Cell<usize>);
    impl AsRef<str> for Changing {
        fn as_ref(&self) -> &str {
            let n = self.0.get();
            self.0.set(n + 1);
            if n == 0 { "é" } else { "ab" }
        }
    }
    let chunk = Changing(Cell::new(0));
    let text = B::<str, _>::try_from_texts_in([&chunk], jzap::budget::Unmetered).unwrap();
    assert_eq!(text.as_str().as_bytes(), "é".as_bytes());
    assert_eq!(chunk.0.get(), 1);
}

#[test]
fn c_strings_validate_transfer_and_refund_without_hidden_allocations() {
    use std::ffi::CStr;
    let budget = MemoryBudget::new(64);
    for input in [c"", c"hello", c"한글"] {
        let (_, stats) = track(usize::MAX, || {
            let owner = B::<CStr, _>::try_from_c_str_in(input, Budget::Limited(&budget)).unwrap();
            assert_eq!(owner.as_c_str(), input);
            let ptr = B::as_ptr(&owner).cast::<u8>();
            let bytes = owner.try_into_bytes_with_nul().unwrap();
            assert_eq!(B::as_ptr(&bytes).cast::<u8>(), ptr);
            let owner = B::<CStr, _>::try_from_bytes_with_nul(bytes).unwrap();
            assert_eq!(owner.as_c_str(), input);
            drop(owner);
        });
        assert_eq!((stats.calls, stats.live, budget.occupied()), (1, 0, 0));
    }
    for invalid in [b"abc".as_slice(), b"a\0b\0", b""] {
        let bytes = B::<[u8]>::try_copy_from_slice(invalid).unwrap();
        let ptr = B::as_ptr(&bytes);
        let (rejected, stats) = track(1, || B::<CStr>::try_from_bytes_with_nul(bytes).unwrap_err());
        assert_eq!(B::as_ptr(&rejected.value), ptr);
        assert_eq!(rejected.value.as_slice(), invalid);
        assert_eq!(stats.calls, 0);
    }
    let input = std::ffi::CString::new("owned").unwrap();
    let ptr = input.as_ptr();
    let zero = MemoryBudget::new(0);
    let (rejected, stats) = track(1, || {
        B::<CStr, _>::try_from_std_c_string_in(input, Budget::Limited(&zero)).unwrap_err()
    });
    assert_eq!(rejected.value.as_ptr(), ptr);
    assert_eq!(stats.calls, 0);
    let (owner, stats) = track(1, || {
        B::<CStr, _>::try_from_std_c_string_in(rejected.value, Budget::Limited(&budget)).unwrap()
    });
    assert_eq!(stats.calls, 0);
    assert_eq!(owner.as_c_str().as_ptr(), ptr);
    drop(owner);
    assert_eq!(budget.occupied(), 0);
    let (err, stats) = track(1, || B::<CStr>::try_from_c_str(c"x"));
    assert!(matches!(err, Err(Error::Allocation { .. })));
    assert_eq!(stats.live, 0);
}

#[cfg(feature = "std")]
#[test]
fn os_strings_and_paths_preserve_native_bytes_and_reject_shrink_failure() {
    use std::{
        ffi::{OsStr, OsString},
        path::{Path, PathBuf},
    };
    let budget = MemoryBudget::new(128);
    let (_, stats) = track(usize::MAX, || {
        let os = B::<OsStr, _>::try_from_os_str_in(OsStr::new("한글/a"), Budget::Limited(&budget))
            .unwrap();
        let address = B::as_ptr(&os).cast::<u8>();
        let path = B::<Path, _>::try_from_os_str(os).unwrap();
        assert_eq!(path.as_path(), Path::new("한글/a"));
        assert_eq!(B::as_ptr(&path).cast::<u8>(), address);
        let os = path.try_into_os_str().unwrap();
        assert_eq!(B::as_ptr(&os).cast::<u8>(), address);
        drop(os);
    });
    assert_eq!((stats.calls, stats.live, budget.occupied()), (1, 0, 0));
    let mut source = OsString::with_capacity(128);
    source.push("a/b");
    let pointer = source.as_encoded_bytes().as_ptr();
    let (rejected, stats) = track(1, || {
        B::<Path>::try_from_std_path_buf_in(PathBuf::from(source), jzap::budget::Unmetered)
            .unwrap_err()
    });
    assert_eq!(
        rejected.value.as_os_str().as_encoded_bytes().as_ptr(),
        pointer
    );
    assert_eq!(rejected.value, PathBuf::from("a/b"));
    assert_eq!((stats.calls, stats.live), (1, 0));
    let (boxed, stats) = track(usize::MAX, || {
        B::<Path>::try_from_std_path_buf_in(rejected.value, jzap::budget::Unmetered).unwrap()
    });
    assert_eq!(stats.calls, 1);
    assert_eq!(boxed.into_std_path_buf(), PathBuf::from("a/b"));
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let invalid_utf8 = OsStr::from_bytes(b"x\xff/y");
        let boxed = B::<OsStr>::try_from_os_str(invalid_utf8).unwrap();
        assert_eq!(boxed.as_os_str().as_bytes(), invalid_utf8.as_bytes());
    }
}

#[test]
fn text_cow_chars_and_error_boxes_cover_success_and_each_refusal() {
    use jzap::budget::Unmetered;
    use std::borrow::Cow;
    for input in ["", "abc", "한글🙂"] {
        let chars = B::<str>::try_from_chars_in(input.chars(), Unmetered).unwrap();
        assert_eq!(chars.as_str(), input);
        let cow = B::<str>::try_from_cow_in(Cow::Borrowed(input), Unmetered).unwrap();
        assert_eq!(cow.as_str(), input);
    }
    let input = String::from("cow");
    let ptr = input.as_ptr();
    let (cow, stats) = track(1, || {
        B::<str>::try_from_cow_in(Cow::Owned(input), Unmetered).unwrap()
    });
    assert_eq!(cow.as_str().as_ptr(), ptr);
    assert_eq!(stats.calls, 0);
    let (rejected, stats) = track(1, || {
        B::<str>::try_from_cow_in(Cow::Borrowed("x"), Unmetered).unwrap_err()
    });
    assert!(matches!(rejected.value, Cow::Borrowed("x")));
    assert_eq!(stats.live, 0);
    let bytes = B::<[u32]>::try_from_cow_in(Cow::Borrowed(&[1, 2]), Unmetered).unwrap();
    assert_eq!(bytes.as_slice(), &[1, 2]);
    for fail in 1..=3 {
        let (result, stats) = track(fail, || {
            let e = B::<dyn std::error::Error + Send + Sync>::try_from_message("message")?;
            assert!(e.as_ref().source().is_none());
            drop(e);
            Ok::<_, Error>(())
        });
        if fail <= 2 {
            assert!(result.is_err());
            assert_eq!(stats.live, 0);
        }
    }
}

#[test]
fn malicious_size_hints_never_control_memory_safety() {
    struct Hint {
        position: usize,
        end: usize,
        hint: usize,
    }
    impl Iterator for Hint {
        type Item = u16;
        fn next(&mut self) -> Option<u16> {
            if self.position == self.end {
                return None;
            }
            let value = self.position as u16;
            self.position += 1;
            Some(value)
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.hint, Some(self.hint))
        }
    }
    for end in 0..12 {
        for hint in 0..15 {
            let b = B::<[u16]>::try_from_iter_in(
                Hint {
                    position: 0,
                    end,
                    hint,
                },
                jzap::budget::Unmetered,
            )
            .unwrap();
            assert_eq!(b.as_slice(), (0..end as u16).collect::<Vec<_>>());
        }
    }
    let (_, stats) = track(1, || {
        assert!(matches!(
            B::<[u16]>::try_from_iter_in(
                Hint {
                    position: 0,
                    end: 0,
                    hint: usize::MAX
                },
                jzap::budget::Unmetered
            ),
            Err(Error::Layout)
        ));
    });
    assert_eq!(stats.calls, 0);
    struct NonFused(u8);
    impl Iterator for NonFused {
        type Item = u8;
        fn next(&mut self) -> Option<u8> {
            self.0 += 1;
            match self.0 {
                1 => Some(9),
                2 => None,
                _ => Some(7),
            }
        }
    }
    let b = B::<[u8]>::try_from_iter_in(NonFused(0), jzap::budget::Unmetered).unwrap();
    assert_eq!(b.as_slice(), &[9]);
}

#[test]
fn struct_packed_fields_are_checked_before_creating_references() {
    #[repr(C, packed)]
    struct Packed {
        a: u8,
        b: u64,
    }
    let result = B::<Packed>::try_init_in(
        jzap::budget::Unmetered,
        |slot: &mut jzap::init::InitSlot<'_, Packed>| {
            jzap::try_init_struct!(
                slot,
                Packed {
                    a: |s: &mut jzap::init::InitSlot<'_, u8>| s.write(1).map_err(|e| e.error),
                    b: |s: &mut jzap::init::InitSlot<'_, u64>| s.write(2).map_err(|e| e.error),
                }
            )
        },
    );
    // The allocator might supply an unusual address; either aligned initialized
    // storage or a clean Layout rejection is valid, never an unaligned reference.
    if let Err(e) = result {
        assert!(matches!(
            e,
            InitError::Initialize(InitError::Storage(Error::Layout))
        ));
    }
}

#[test]
fn borrowed_callable_hasher_and_iterator_adapters_preserve_behavior() {
    use std::hash::{Hash, Hasher};
    let mut function = B::try_new(|x: u64| x + 1).ok().unwrap();
    assert_eq!(function.as_ref()(7), 8);
    assert_eq!(function.as_mut()(8), 9);
    let mut iter = B::try_new(2..7).unwrap();
    assert_eq!(iter.next(), Some(2));
    assert_eq!(iter.next_back(), Some(6));
    assert_eq!(iter.len(), 3);
    let mut dynamic = jzap::box_unsize!(iter => dyn Iterator<Item=i32>);
    assert_eq!(dynamic.as_mut().next(), Some(3));
    #[derive(Debug)]
    struct Distinct(u64);
    impl Hasher for Distinct {
        fn finish(&self) -> u64 {
            self.0
        }
        fn write(&mut self, _: &[u8]) {
            self.0 = 11;
        }
        fn write_u64(&mut self, value: u64) {
            self.0 = value + 100;
        }
    }
    let mut hasher = B::try_new(Distinct(0)).unwrap();
    hasher.write_u64(7);
    assert_eq!(hasher.finish(), 107);
    9u64.hash(&mut hasher);
    assert_eq!(hasher.finish(), 109);
    let mut dynamic = jzap::box_unsize!(hasher => dyn Hasher);
    dynamic.write_u64(13);
    assert_eq!(dynamic.finish(), 113);
    #[derive(Debug, PartialEq)]
    struct E(u64);
    impl std::fmt::Display for E {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
    impl std::error::Error for E {}
    let error =
        B::<dyn std::error::Error + Send + Sync>::try_from_error_in(E(42), jzap::budget::Unmetered)
            .unwrap();
    let error = error.downcast::<std::fmt::Error>().unwrap_err();
    assert_eq!(*error.downcast::<E>().unwrap(), E(42));
}

#[test]
fn consuming_call_argument_panic_releases_owner_and_budget() {
    let budget = MemoryBudget::new(64);
    let drops = Cell::new(0);
    struct D<'a>(&'a Cell<usize>);
    impl Drop for D<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1)
        }
    }
    let captured = D(&drops);
    let function = B::try_new_in(move |_: u8| drop(captured), Budget::Limited(&budget))
        .ok()
        .unwrap();
    fn argument() -> u8 {
        panic!("argument failure")
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || jzap::box_call_once!(function;argument()),
    ));
    assert!(result.is_err());
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.occupied(), 0);
}

#[test]
fn ownership_borrow_pin_and_default_adapters_keep_the_allocation() {
    use jzap::{TryDefault, budget::Unmetered};
    use std::borrow::{Borrow, BorrowMut};
    let mut value = B::try_new(41u64).unwrap();
    let borrowed: &u64 = value.borrow();
    assert_eq!(*borrowed, 41);
    let borrowed: &mut u64 = value.borrow_mut();
    *borrowed = 42;
    let pointer = B::as_mut_ptr(&mut value);
    let leaked = B::leak(value);
    assert_eq!(*leaked, 42);
    // SAFETY: recover the sole leaked owner after its last reference use.
    let value = unsafe { B::from_raw(pointer) };
    let slice = B::into_boxed_slice(value);
    assert_eq!(slice.as_slice(), &[42]);
    let original = slice.try_into_array::<2>().unwrap_err();
    assert_eq!(original.len(), 1);
    let mut slice = original;
    assert!(slice.copy_from(&[7, 8]).is_err());
    assert_eq!(slice.as_slice(), &[42]);
    slice.copy_from(&[9]).unwrap();
    for value in &mut slice {
        *value += 1;
    }
    assert_eq!((&slice).into_iter().copied().sum::<u64>(), 10);
    *slice.get_mut(0).unwrap() = 12;
    assert!(slice.get_mut(1).is_err());
    assert_eq!(slice.into_std_vec(), vec![12]);
    let empty = B::<[u64]>::try_default().unwrap();
    assert!(empty.is_empty());
    let empty = B::<str>::try_default().unwrap();
    assert!(empty.is_empty());
    let mut text = B::<str>::try_from_str("ab").unwrap();
    text.as_mut_str().make_ascii_uppercase();
    text.get_mut(1..2).unwrap().make_ascii_lowercase();
    assert_eq!(text.as_str(), "Ab");
    assert!(text.get_mut(3..4).is_err());
    let source = B::<str>::try_from_str("longer").unwrap();
    let (r, s) = track(1, || text.try_clone_from(&source));
    assert!(r.is_err());
    assert_eq!(s.live, 0);
    assert_eq!(text.as_str(), "Ab");
    text.try_clone_from(&source).unwrap();
    assert_eq!(text.len(), 6);
    let budget = MemoryBudget::new(64);
    let copy = text.try_clone_in(Budget::Limited(&budget)).unwrap();
    assert_eq!(copy.as_str(), "longer");
    drop(copy);
    assert_eq!(budget.occupied(), 0);
    let pinned = B::<u64>::try_pin_init_in(Unmetered, |s: &mut jzap::init::InitSlot<'_, u64>| {
        s.write(5).map_err(|e| e.error)
    })
    .unwrap();
    assert_eq!(*pinned.as_ref(), 5);
    assert_eq!(B::into_inner(pinned.into_inner()), 5);
    let mut future = B::try_new(std::future::ready(7)).unwrap();
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(Pin::new(&mut future).poll(&mut cx), Poll::Ready(7));
    let scalar = B::<u32>::try_new_zeroed().unwrap();
    assert_eq!(*unsafe { B::<MaybeUninit<u32>>::assume_init(scalar) }, 0);
    let empty = B::<[()]>::try_from_std_vec_in(vec![(); 7], Unmetered).unwrap();
    assert_eq!(empty.len(), 7);
    assert_eq!(
        <(u8, Option<u8>, &str, &[u8])>::try_default().unwrap(),
        (0, None, "", &[][..])
    );
}

#[test]
fn foreign_clone_defaults_and_failures_keep_destination_and_budget() {
    use jzap::TryDefault;
    use std::ffi::CStr;
    let budget = MemoryBudget::new(64);
    let source = B::<CStr>::try_from_c_str(c"source").unwrap();
    let mut destination = B::<CStr>::try_default().unwrap();
    let ptr = B::as_ptr(&destination);
    let (r, s) = track(1, || destination.try_clone_from(&source));
    assert!(r.is_err());
    assert_eq!(s.live, 0);
    assert_eq!(B::as_ptr(&destination), ptr);
    destination.try_clone_from(&source).unwrap();
    assert_eq!(destination.as_c_str(), c"source");
    let copy = source.try_clone().unwrap();
    assert_eq!(copy.into_std_c_string().as_c_str(), c"source");
    let copy = source.try_clone_in(Budget::Limited(&budget)).unwrap();
    assert_eq!(budget.occupied(), 7);
    drop(copy);
    assert_eq!(budget.occupied(), 0);
    #[cfg(feature = "std")]
    {
        use std::{ffi::OsStr, path::Path};
        let source = B::<OsStr>::try_from_os_str(OsStr::new("source")).unwrap();
        let mut dest = B::<OsStr>::try_default().unwrap();
        let (r, _) = track(1, || dest.try_clone_from(&source));
        assert!(r.is_err());
        assert!(dest.as_os_str().is_empty());
        dest.try_clone_from(&source).unwrap();
        assert_eq!(dest.into_std_os_string(), "source");
        let clone = source.try_clone().unwrap();
        assert_eq!(clone.as_os_str(), source.as_os_str());
        drop(source.try_clone_in(Budget::Limited(&budget)).unwrap());
        assert_eq!(budget.occupied(), 0);
        let source = B::<Path>::try_from_path(Path::new("a/b")).unwrap();
        let mut dest = B::<Path>::try_default().unwrap();
        let (r, _) = track(1, || dest.try_clone_from(&source));
        assert!(r.is_err());
        assert!(dest.as_path().as_os_str().is_empty());
        dest.try_clone_from(&source).unwrap();
        assert_eq!(dest.as_path(), source.as_path());
        assert_eq!(source.try_clone().unwrap().as_path(), Path::new("a/b"));
        drop(source.try_clone_in(Budget::Limited(&budget)).unwrap());
        assert_eq!(budget.occupied(), 0);
    }
}

#[test]
fn unicode_collection_each_allocation_failure_releases_every_byte() {
    use jzap::budget::Unmetered;
    let (_, baseline) = track(usize::MAX, || {
        drop(B::<str>::try_from_chars_in("🙂".chars().cycle().take(33), Unmetered).unwrap());
    });
    assert!(baseline.calls > 1);
    assert_eq!(baseline.live, 0);
    for fail in 1..=baseline.calls {
        let (r, stats) = track(fail, || {
            B::<str>::try_from_chars_in("🙂".chars().cycle().take(33), Unmetered)
        });
        assert!(matches!(r, Err(Error::Allocation { .. })));
        assert_eq!(stats.live, 0);
    }
    let message = B::<dyn std::error::Error>::try_from_message("visible").unwrap();
    assert_eq!(message.to_string(), "visible");
}

#[test]
fn equality_order_hash_format_and_consuming_async_call_match_std() {
    use std::hash::{Hash, Hasher};
    let a = B::try_new(3u64).unwrap();
    let b = B::try_new(7u64).unwrap();
    assert!(a < b);
    assert_eq!(a.cmp(&b), std::cmp::Ordering::Less);
    assert_eq!(a, a);
    assert_eq!(format!("{a:?} {a}"), "3 3");
    assert_eq!(format!("{a:p}"), format!("{:p}", B::as_ptr(&a)));
    let mut ours = std::hash::DefaultHasher::new();
    let mut standard = std::hash::DefaultHasher::new();
    a.hash(&mut ours);
    3u64.hash(&mut standard);
    assert_eq!(ours.finish(), standard.finish());
    let mut iter = B::try_new(0..10).unwrap();
    assert_eq!(iter.size_hint(), (10, Some(10)));
    assert_eq!(iter.nth(1), Some(1));
    assert_eq!(iter.nth_back(1), Some(8));
    let capture = String::from("async");
    let function = B::try_new(async move |extra: usize| capture.len() + extra)
        .ok()
        .unwrap();
    let future = jzap::box_call_once!(function;2usize);
    let mut pinned = B::try_pin(future).ok().unwrap();
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(Pin::new(&mut pinned).poll(&mut cx), Poll::Ready(7));
}

#[test]
fn small_user_clone_from_reuses_storage_and_preserves_value_on_error() {
    #[derive(Debug, PartialEq)]
    struct V(u64);
    impl TryClone for V {
        fn try_clone(&self) -> Result<Self, Error> {
            if self.0 == 0 {
                Err(Error::Capacity)
            } else {
                Ok(Self(self.0))
            }
        }
    }
    let mut destination = B::try_new(V(1)).unwrap();
    let source = B::try_new(V(2)).unwrap();
    let bad = B::try_new(V(0)).unwrap();
    let pointer = B::as_ptr(&destination);
    let (result, stats) = track(1, || destination.try_clone_from(&source));
    assert!(result.is_ok());
    assert_eq!(stats.calls, 0);
    assert_eq!(B::as_ptr(&destination), pointer);
    assert_eq!(*destination, V(2));
    assert_eq!(destination.try_clone_from(&bad), Err(Error::Capacity));
    assert_eq!(*destination, V(2));
    assert_eq!(B::as_ptr(&destination), pointer);
}

#[test]
fn nested_array_clone_from_reuses_heap_without_staging_the_whole_value() {
    let source = B::<[[u64; 128]; 128]>::try_init_array_in(jzap::budget::Unmetered, |i, s| {
        s.try_init_array(|j, v| v.write((i + j) as u64).map_err(|e| e.error))
    })
    .unwrap();
    let mut destination =
        B::<[[u64; 128]; 128]>::try_init_array_in(jzap::budget::Unmetered, |_, s| {
            s.try_init_array(|_, v| v.write(0).map_err(|e| e.error))
        })
        .unwrap();
    let ptr = B::as_ptr(&destination);
    let (result, stats) = track(1, || destination.try_clone_from(&source));
    assert!(result.is_ok());
    assert_eq!(stats.calls, 0);
    assert_eq!(B::as_ptr(&destination), ptr);
    assert_eq!(*destination, *source);
    let huge = [[(); usize::MAX]; 2];
    let mut target = huge;
    assert_eq!(
        <[(); usize::MAX]>::try_copy_slice_from(&huge, &mut target),
        Some(Err(Error::Capacity))
    );
    let mut empty = [[]; 2];
    assert_eq!(
        <[u8; 0]>::try_copy_slice_from(&[[], []], &mut empty),
        Some(Ok(()))
    );
    assert_eq!(
        <[u8; 0]>::try_copy_slice_from(&[[]], &mut empty),
        Some(Err(Error::Bounds))
    );
}
#[test]
fn maximum_unit_slice_partially_iterates_and_releases_zero_budget() {
    let budget = jzap::budget::MemoryBudget::new(0);
    let owner = jzap::boxed::Box::<[()], _>::try_new_zeroed_slice_in(
        usize::MAX,
        jzap::budget::Budget::Limited(&budget),
    )
    .unwrap();
    // SAFETY: unit elements have no bytes and every zero-bit unit is valid.
    let owner = unsafe { jzap::boxed::Box::<[core::mem::MaybeUninit<()>], _>::assume_init(owner) };
    let mut it = owner.into_iter();
    assert_eq!(it.next(), Some(()));
    assert_eq!(it.next_back(), Some(()));
    assert_eq!(it.len(), usize::MAX - 2);
    drop(it);
    assert_eq!(budget.occupied(), 0);
    assert_eq!(budget.reserved(), 0);
}
