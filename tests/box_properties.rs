use jzap::{
    TryClone,
    boxed::Box as B,
    budget::{Budget, MemoryBudget},
};
use proptest::prelude::*;
proptest! {
 #![proptest_config(ProptestConfig::with_cases(2048))]
 #[test]
 fn slice_matches_std_after_mixed_iteration(values in prop::collection::vec(any::<u32>(),0..128), choices in prop::collection::vec(any::<bool>(),0..160)) {
    let budget=MemoryBudget::new(values.len()*4*2);
    let boxed=B::<[u32],_>::try_copy_from_slice_in(&values,Budget::Limited(&budget)).unwrap();
    let copy=boxed.try_clone().unwrap();
    prop_assert_eq!(copy.as_slice(),values.as_slice()); drop(copy);
    let mut ours=boxed.into_iter(); let mut standard=values.into_boxed_slice().into_iter();
    for back in choices { prop_assert_eq!(if back {ours.next_back()} else {ours.next()},if back {standard.next_back()} else {standard.next()}); prop_assert_eq!(ours.len(),standard.len()); }
    prop_assert_eq!(ours.by_ref().collect::<Vec<_>>(),standard.collect::<Vec<_>>());
    drop(ours); prop_assert_eq!(budget.occupied(),0); prop_assert_eq!(budget.reserved(),0);
 }
 #[test]
 fn utf8_conversion_matches_std(bytes in prop::collection::vec(any::<u8>(),0..128)) {
    let owner=B::<[u8]>::try_copy_from_slice(&bytes).unwrap(); let ptr=B::as_ptr(&owner).cast::<u8>();
    let expected=std::str::from_utf8(&bytes);
    match B::<str>::try_from_utf8(owner) {
      Ok(text)=>{prop_assert_eq!(Some(text.as_str()),expected.ok());prop_assert_eq!(text.as_str().as_ptr(),ptr);let roundtrip=text.into_bytes();prop_assert_eq!(roundtrip.as_slice(),bytes.as_slice());},
      Err(rejected)=>{prop_assert!(expected.is_err());prop_assert_eq!(rejected.value.as_slice(),bytes.as_slice());prop_assert_eq!(B::as_ptr(&rejected.value).cast::<u8>(),ptr);}
    }
 }
 #[test]
 fn unicode_collection_matches_std(chars in prop::collection::vec(any::<char>(),0..64)) {
    let expected: String=chars.iter().collect();
    let budget=MemoryBudget::new(2048);
    let boxed=B::<str,_>::try_from_chars_in(chars,Budget::Limited(&budget)).unwrap();
    prop_assert_eq!(boxed.as_str(),expected.as_str());prop_assert_eq!(budget.occupied(),expected.len());drop(boxed);prop_assert_eq!(budget.occupied(),0);
 }
}
