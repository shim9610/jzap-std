use super::*;
use core::{
    convert::Infallible,
    iter::{FusedIterator, Peekable},
};

// Keep folds in their callers so canonical leaf-edge construction can eliminate
// pending-owning-cursor cases from the hot path.
macro_rules! inline_fold {
    () => {
        #[inline(always)]
        fn fold<B, F>(self, mut accumulator: B, mut f: F) -> B
        where
            F: FnMut(B, Self::Item) -> B,
        {
            for item in self {
                accumulator = f(accumulator, item);
            }
            accumulator
        }
    };
}

pub(super) fn merged_hint(a: usize, b: usize, union: bool) -> (usize, Option<usize>) {
    (
        if union { a.max(b) } else { a.abs_diff(b) },
        a.checked_add(b),
    )
}

enum DifferenceInner<'a, T, P: AllocationPolicy> {
    Iterate(Iter<'a, T>),
    Search(Iter<'a, T>, &'a BTreeSet<T, P>),
    Stitch(Iter<'a, T>, Peekable<Iter<'a, T>>),
}
pub struct Difference<'a, T, P: AllocationPolicy = Unmetered>(DifferenceInner<'a, T, P>);
enum IntersectionInner<'a, T, P: AllocationPolicy> {
    Answer(Option<&'a T>),
    Search(Iter<'a, T>, &'a BTreeSet<T, P>),
    Stitch(Iter<'a, T>, Iter<'a, T>),
}
pub struct Intersection<'a, T, P: AllocationPolicy = Unmetered>(IntersectionInner<'a, T, P>);
struct Merge<'a, T: 'a, I: Iterator<Item = &'a T> = Iter<'a, T>> {
    a: I,
    b: I,
    pending: Option<(bool, &'a T)>,
}
pub struct Union<'a, T>(Merge<'a, T>);
pub struct SymmetricDifference<'a, T>(Merge<'a, T>);

impl<T: Ord, P: AllocationPolicy> BTreeSet<T, P> {
    pub fn difference<'s>(&'s self, other: &'s Self) -> Difference<'s, T, P> {
        use DifferenceInner::*;
        let inner = match (self.first(), self.last(), other.first(), other.last()) {
            (Some(a), Some(z), Some(b), Some(y)) => match (a.cmp(y), z.cmp(b)) {
                (Ordering::Greater, _) | (_, Ordering::Less) => Iterate(self.iter()),
                (Ordering::Equal, _) => {
                    let mut i = self.iter();
                    i.next();
                    Iterate(i)
                }
                (_, Ordering::Equal) => {
                    let mut i = self.iter();
                    i.next_back();
                    Iterate(i)
                }
                _ if self.len() <= other.len() / 16 => Search(self.iter(), other),
                _ => Stitch(self.iter(), other.iter().peekable()),
            },
            _ => Iterate(self.iter()),
        };
        Difference(inner)
    }
    #[inline(always)]
    pub fn intersection<'s>(&'s self, other: &'s Self) -> Intersection<'s, T, P> {
        use IntersectionInner::*;
        let inner = match (self.first(), self.last(), other.first(), other.last()) {
            (Some(a), Some(z), Some(b), Some(y)) => match (a.cmp(y), z.cmp(b)) {
                (Ordering::Greater, _) | (_, Ordering::Less) => Answer(None),
                (Ordering::Equal, _) => Answer(Some(a)),
                (_, Ordering::Equal) => Answer(Some(z)),
                _ if self.len() <= other.len() / 16 => Search(self.iter(), other),
                _ if other.len() <= self.len() / 16 => Search(other.iter(), self),
                _ => Stitch(self.iter(), other.iter()),
            },
            _ => Answer(None),
        };
        Intersection(inner)
    }
    #[inline(always)]
    pub fn union<'s>(&'s self, other: &'s Self) -> Union<'s, T> {
        Union(Merge::new(self.iter(), other.iter()))
    }
    #[inline(always)]
    pub fn symmetric_difference<'s>(&'s self, other: &'s Self) -> SymmetricDifference<'s, T> {
        SymmetricDifference(Merge::new(self.iter(), other.iter()))
    }
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.intersection(other).next().is_none()
    }
    pub fn is_subset(&self, other: &Self) -> bool {
        if self.len() > other.len() {
            return false;
        }
        let (Some(a), Some(z)) = (self.first(), self.last()) else {
            return true;
        };
        let (Some(b), Some(y)) = (other.first(), other.last()) else {
            return false;
        };
        let mut small = self.iter();
        match a.cmp(b) {
            Ordering::Less => return false,
            Ordering::Equal => {
                small.next();
            }
            _ => {}
        }
        match z.cmp(y) {
            Ordering::Greater => return false,
            Ordering::Equal => {
                small.next_back();
            }
            _ => {}
        }
        if small.len() <= other.len() / 16 {
            return small.all(|k| other.contains(k));
        }
        let mut large = other.iter();
        large.next();
        large.next_back();
        small.all(|key| {
            for candidate in large.by_ref() {
                match candidate.cmp(key) {
                    Ordering::Less => continue,
                    Ordering::Equal => return true,
                    Ordering::Greater => return false,
                }
            }
            false
        })
    }
    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }
}
impl<T: Ord + TryClone, P: AllocationPolicy> BTreeSet<T, P> {
    fn try_own<'a>(items: impl Iterator<Item = &'a T>, policy: P) -> Result<Self, Error>
    where
        T: 'a,
    {
        BTreeMap::try_from_sorted_source(
            items.map(|key| key.try_clone().map(|key| (key, ()))),
            policy,
        )
        .map(|map| Self { map })
    }
    pub fn try_union(&self, other: &Self) -> Result<Self, Error> {
        Self::try_own(self.union(other), self.allocation_policy().try_clone()?)
    }
    pub fn try_intersection(&self, other: &Self) -> Result<Self, Error> {
        Self::try_own(
            self.intersection(other),
            self.allocation_policy().try_clone()?,
        )
    }
    pub fn try_difference(&self, other: &Self) -> Result<Self, Error> {
        Self::try_own(
            self.difference(other),
            self.allocation_policy().try_clone()?,
        )
    }
    pub fn try_symmetric_difference(&self, other: &Self) -> Result<Self, Error> {
        Self::try_own(
            self.symmetric_difference(other),
            self.allocation_policy().try_clone()?,
        )
    }
}
impl<'a, T: Ord, P: AllocationPolicy> Iterator for Difference<'a, T, P> {
    type Item = &'a T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            DifferenceInner::Iterate(i) => i.next(),
            DifferenceInner::Search(i, set) => i.find(|key| !set.contains(*key)),
            DifferenceInner::Stitch(a, b) => {
                for key in a.by_ref() {
                    loop {
                        match b.peek() {
                            Some(other) => match key.cmp(other) {
                                Ordering::Less => return Some(key),
                                Ordering::Equal => {
                                    b.next();
                                    break;
                                }
                                Ordering::Greater => {
                                    b.next();
                                }
                            },
                            None => return Some(key),
                        }
                    }
                }
                None
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            DifferenceInner::Iterate(i) => i.size_hint(),
            DifferenceInner::Search(i, _) | DifferenceInner::Stitch(i, _) => (0, Some(i.len())),
        }
    }
}
impl<'a, T: Ord, P: AllocationPolicy> Iterator for Intersection<'a, T, P> {
    type Item = &'a T;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IntersectionInner::Answer(answer) => answer.take(),
            IntersectionInner::Search(iter, set) => iter.find(|key| set.contains(*key)),
            IntersectionInner::Stitch(a, b) => {
                let mut left = a.next()?;
                let mut right = b.next()?;
                loop {
                    match left.cmp(right) {
                        Ordering::Less => left = a.next()?,
                        Ordering::Greater => right = b.next()?,
                        Ordering::Equal => return Some(left),
                    }
                }
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            0,
            Some(match &self.0 {
                IntersectionInner::Answer(a) => usize::from(a.is_some()),
                IntersectionInner::Search(i, _) => i.len(),
                IntersectionInner::Stitch(a, b) => a.len().min(b.len()),
            }),
        )
    }
    inline_fold!();
}
impl<'a, T, I: Iterator<Item = &'a T>> Merge<'a, T, I> {
    fn new(a: I, b: I) -> Self {
        Self {
            a,
            b,
            pending: None,
        }
    }
}
impl<'a, T: Ord, I: ExactSizeIterator<Item = &'a T> + FusedIterator> Merge<'a, T, I> {
    #[inline(always)]
    fn next(&mut self) -> (Option<&'a T>, Option<&'a T>) {
        // The sources are fused. At most one value needs to be buffered after
        // comparing their heads; two general-purpose Peekables are unnecessary.
        let (a, b) = match self.pending.take() {
            Some((true, key)) => (Some(key), self.b.next()),
            Some((false, key)) => (self.a.next(), Some(key)),
            None => (self.a.next(), self.b.next()),
        };
        match (a, b) {
            (Some(left), Some(right)) => match left.cmp(right) {
                Ordering::Less => {
                    self.pending = Some((false, right));
                    (a, None)
                }
                Ordering::Greater => {
                    self.pending = Some((true, left));
                    (None, b)
                }
                Ordering::Equal => (a, b),
            },
            _ => (a, b),
        }
    }
    fn size_hint(&self, union: bool) -> (usize, Option<usize>) {
        let a = self
            .a
            .len()
            .saturating_add(usize::from(matches!(self.pending, Some((true, _)))));
        let b = self
            .b
            .len()
            .saturating_add(usize::from(matches!(self.pending, Some((false, _)))));
        merged_hint(a, b, union)
    }
}
impl<'a, T: Ord> Iterator for Union<'a, T> {
    type Item = &'a T;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let (a, b) = self.0.next();
        a.or(b)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint(true)
    }
    inline_fold!();
}
impl<'a, T: Ord> Iterator for SymmetricDifference<'a, T> {
    type Item = &'a T;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.0.next() {
                (Some(_), Some(_)) => {}
                (a, b) => return a.or(b),
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint(false)
    }
    inline_fold!();
}
impl<T, P: AllocationPolicy> Clone for Difference<'_, T, P> {
    fn clone(&self) -> Self {
        Self(match &self.0 {
            DifferenceInner::Iterate(i) => DifferenceInner::Iterate(i.clone()),
            DifferenceInner::Search(i, s) => DifferenceInner::Search(i.clone(), s),
            DifferenceInner::Stitch(a, b) => DifferenceInner::Stitch(a.clone(), b.clone()),
        })
    }
}
impl<T, P: AllocationPolicy> Clone for Intersection<'_, T, P> {
    fn clone(&self) -> Self {
        Self(match &self.0 {
            IntersectionInner::Answer(a) => IntersectionInner::Answer(*a),
            IntersectionInner::Search(i, s) => IntersectionInner::Search(i.clone(), s),
            IntersectionInner::Stitch(a, b) => IntersectionInner::Stitch(a.clone(), b.clone()),
        })
    }
}
impl<'a, T, I: Iterator<Item = &'a T> + Clone> Clone for Merge<'a, T, I> {
    fn clone(&self) -> Self {
        Self {
            a: self.a.clone(),
            b: self.b.clone(),
            pending: self.pending,
        }
    }
}
macro_rules! merged_clone {
    ($name:ident) => {
        impl<T> Clone for $name<'_, T> {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }
    };
}
merged_clone!(Union);
merged_clone!(SymmetricDifference);
macro_rules! mirrors{
    ([$($generic:tt)*] $ty:ty)=>{
        impl<$($generic)*> FusedIterator for $ty{}
        impl<$($generic)*> TryIterator for $ty{
            type Item=<Self as Iterator>::Item;type Error=Infallible;
            #[inline(always)]
            fn try_next(&mut self)->Result<Option<Self::Item>,Infallible>{Ok(self.next())}
            fn try_size_hint(&self)->Result<(usize,Option<usize>),Infallible>{Ok(self.size_hint())}
        }
        impl<$($generic)*> crate::TryFusedIterator for $ty{}
    }
}
mirrors!(['a,T:Ord,P:AllocationPolicy] Difference<'a,T,P>);
mirrors!(['a,T:Ord,P:AllocationPolicy] Intersection<'a,T,P>);
mirrors!(['a,T:Ord] Union<'a,T>);
mirrors!(['a,T:Ord] SymmetricDifference<'a,T>);
macro_rules! debug{
    ([$($generic:tt)*] $ty:ty)=>{impl<$($generic)*> core::fmt::Debug for $ty{
        fn fmt(&self,f:&mut core::fmt::Formatter<'_>)->core::fmt::Result{f.debug_list().entries(self.clone()).finish()}
    }}
}
debug!(['a,T:Ord+core::fmt::Debug,P:AllocationPolicy] Difference<'a,T,P>);
debug!(['a,T:Ord+core::fmt::Debug,P:AllocationPolicy] Intersection<'a,T,P>);
debug!(['a,T:Ord+core::fmt::Debug] Union<'a,T>);
debug!(['a,T:Ord+core::fmt::Debug] SymmetricDifference<'a,T>);

#[cfg(kani)]
mod merge_proofs {
    use super::Merge;
    #[kani::proof]
    #[kani::unwind(9)]
    fn sorted_prefixes_preserve_values_progress_and_hints() {
        let left: [u8; 3] = kani::any();
        let right: [u8; 3] = kani::any();
        let a: usize = kani::any();
        let b: usize = kani::any();
        kani::assume(a <= 3 && b <= 3);
        for i in 1..3 {
            if i < a {
                kani::assume(left[i - 1] < left[i]);
            }
            if i < b {
                kani::assume(right[i - 1] < right[i]);
            }
        }
        let mut expected = a + b;
        for i in 0..3 {
            for j in 0..3 {
                if i < a && j < b && left[i] == right[j] {
                    expected -= 1;
                }
            }
        }
        let mut merge = Merge::new(left[..a].iter(), right[..b].iter());
        let mut produced = 0;
        let mut previous = None;
        loop {
            let (lower, upper) = merge.size_hint(true);
            assert!(lower <= expected - produced);
            assert!(upper.is_some_and(|n| expected - produced <= n));
            let (x, y) = merge.next();
            if let (Some(x), Some(y)) = (x, y) {
                assert_eq!(x, y);
            }
            let Some(value) = x.or(y) else { break };
            let mut found = false;
            for i in 0..3 {
                found |= i < a && left[i] == *value;
                found |= i < b && right[i] == *value;
            }
            assert!(found);
            if let Some(previous) = previous {
                assert!(previous < *value);
            }
            previous = Some(*value);
            produced += 1;
            assert!(produced <= expected);
        }
        assert_eq!(produced, expected);
        assert_eq!(merge.next(), (None, None));
    }
}
