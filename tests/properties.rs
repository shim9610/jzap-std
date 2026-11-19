#![allow(clippy::unwrap_used)]
use jzap::{TryClone, budget::MemoryBudget, clone::try_copy_slice};
use proptest::prelude::*;
proptest! {
 #![proptest_config(ProptestConfig::with_cases(2048))]
 #[test]fn budget_sequences(limit in any::<usize>(),a in any::<usize>(),b in any::<usize>(),commit in any::<bool>()){
  let budget=MemoryBudget::new(limit);let first=budget.reserve(a);
  prop_assert_eq!(first.is_ok(),a<=limit);
  if let Ok(r)=first{let available=limit-a;let second=budget.reserve(b);prop_assert_eq!(second.is_ok(),b<=available);drop(second);if commit{let c=r.commit();prop_assert_eq!(budget.occupied(),a);drop(c)}else{drop(r)}}
  prop_assert_eq!(budget.available(),limit);prop_assert_eq!(budget.occupied(),0);prop_assert_eq!(budget.reserved(),0);
 }
 #[test]fn copy_matches_std(source in prop::array::uniform32(any::<u64>()),short in 0usize..33){
  let mut a=[0;32];let mut b=[0;32];try_copy_slice(&source,&mut a).unwrap();b.copy_from_slice(&source);prop_assert_eq!(a,b);prop_assert_eq!(source.try_clone().unwrap(),source);
  let old=a;let r=try_copy_slice(&source[..short],&mut a);prop_assert_eq!(r.is_ok(),short==32);if short<32{prop_assert_eq!(a,old)}
 }
}
