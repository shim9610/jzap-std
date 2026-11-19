use crate::corpus::Case;
use jzap::{
    TryClone,
    budget::{AllocationPolicy, Budget, MemoryBudget, Unmetered},
    vec::{LengthLimit, LengthPolicy, Unlimited, Vec as JVec},
};
use std::hint::black_box;

pub struct NoHint<I>(I);
impl<I: Iterator> Iterator for NoHint<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

pub trait Owner: Sized {
    fn reserve(&mut self, n: usize);
    fn push(&mut self, value: u64);
    fn copy(&mut self, input: &[u64]);
    fn clone_owner(&self) -> Self;
    fn clear(&mut self);
    fn slice(&self) -> &[u64];
    fn edit<const OP: usize, const VERIFY: bool>(&mut self, c: &Case) -> (u64, usize);
}
// Budgeted owners borrow the process-local budget through this bounded lifetime.
pub trait NewOwner<'a>: Owner {
    fn new(c: &Case, budget: &'a MemoryBudget) -> Self;
}

#[inline(always)]
fn observe<const VERIFY: bool>(result: &mut (u64, usize), value: u64) {
    black_box(value);
    if VERIFY {
        result.0 = result.0.wrapping_mul(0x100000001b3) ^ value;
        result.1 += 1;
    }
}

impl Owner for Vec<u64> {
    #[inline(always)]
    fn reserve(&mut self, n: usize) {
        self.reserve_exact(n);
    }
    #[inline(always)]
    fn push(&mut self, x: u64) {
        self.push(x);
    }
    #[inline(always)]
    fn copy(&mut self, x: &[u64]) {
        self.extend_from_slice(x);
    }
    #[inline(always)]
    fn clone_owner(&self) -> Self {
        self.clone()
    }
    #[inline(always)]
    fn clear(&mut self) {
        self.clear();
    }
    #[inline(always)]
    fn slice(&self) -> &[u64] {
        self
    }
    #[inline(always)]
    fn edit<const OP: usize, const VERIFY: bool>(&mut self, c: &Case) -> (u64, usize) {
        let mut result = (0, 0);
        match OP {
            4 => self.retain(|x| *x < c.threshold),
            5 => self.dedup(),
            6 => drop(self.drain(c.start..c.end)),
            7 | 12 | 13 => {
                for x in self.extract_if(c.start..c.end, |x| *x < c.threshold) {
                    observe::<VERIFY>(&mut result, x);
                }
            }
            8 => drop(self.splice(c.start..c.end, c.replacement.iter().copied())),
            10 => drop(self.splice(
                c.start..c.end,
                c.replacement.iter().copied().filter(|_| true),
            )),
            11 => drop(self.splice(c.start..c.end, NoHint(c.replacement.iter().copied()))),
            9 => self.extend_from_within(c.start..c.end),
            14 => {
                self.insert(c.index, c.value);
                observe::<VERIFY>(&mut result, self.remove(c.index));
            }
            15 => self.resize(c.resize, c.value),
            16 => {
                let tail = self.split_off(c.index);
                black_box(&tail);
                drop(tail);
            }
            17 => {
                let mut other = self.clone();
                self.append(&mut other);
                black_box(&other);
            }
            18 => {
                self.truncate(c.index);
                self.shrink_to_fit();
            }
            19 => {
                while let Some(x) = self.pop() {
                    observe::<VERIFY>(&mut result, x);
                }
            }
            _ => unreachable!(),
        }
        result
    }
}
impl<'a> NewOwner<'a> for Vec<u64> {
    fn new(_: &Case, _: &'a MemoryBudget) -> Self {
        std::vec::Vec::new()
    }
}

impl<P: AllocationPolicy, L: LengthPolicy> Owner for JVec<u64, P, L> {
    #[inline(always)]
    fn reserve(&mut self, n: usize) {
        self.try_reserve_exact(n).unwrap();
    }
    #[inline(always)]
    fn push(&mut self, x: u64) {
        self.try_push(x).unwrap();
    }
    #[inline(always)]
    fn copy(&mut self, x: &[u64]) {
        self.try_extend_copy(x).unwrap();
    }
    #[inline(always)]
    fn clone_owner(&self) -> Self {
        self.try_clone().unwrap()
    }
    #[inline(always)]
    fn clear(&mut self) {
        self.clear();
    }
    #[inline(always)]
    fn slice(&self) -> &[u64] {
        self.as_slice()
    }
    #[inline(always)]
    fn edit<const OP: usize, const VERIFY: bool>(&mut self, c: &Case) -> (u64, usize) {
        let mut result = (0, 0);
        match OP {
            4 => self.retain(|x| *x < c.threshold),
            5 => self.dedup(),
            6 => drop(self.try_drain(c.start..c.end).unwrap()),
            7 | 12 | 13 => {
                for x in self
                    .try_extract_if(c.start..c.end, |x| Ok::<_, ()>(*x < c.threshold))
                    .unwrap()
                {
                    observe::<VERIFY>(&mut result, x.unwrap());
                }
            }
            8 => self
                .try_splice(c.start..c.end, c.replacement.iter().copied())
                .unwrap()
                .finish()
                .unwrap(),
            10 => self
                .try_splice(
                    c.start..c.end,
                    c.replacement.iter().copied().filter(|_| true),
                )
                .unwrap()
                .finish()
                .unwrap(),
            11 => self
                .try_splice(c.start..c.end, NoHint(c.replacement.iter().copied()))
                .unwrap()
                .finish()
                .unwrap(),
            9 => self.try_extend_from_within(c.start..c.end).unwrap(),
            14 => {
                self.try_insert(c.index, c.value).unwrap();
                observe::<VERIFY>(&mut result, self.remove(c.index).unwrap());
            }
            15 => self.try_resize(c.resize, &c.value).unwrap(),
            16 => {
                let tail = self.try_split_off(c.index).unwrap();
                black_box(&tail);
                drop(tail);
            }
            17 => {
                let mut other = self.try_clone().unwrap();
                self.try_append(&mut other).unwrap();
                black_box(&other);
            }
            18 => {
                self.truncate(c.index);
                self.try_shrink_to_fit().unwrap();
            }
            19 => {
                while let Some(x) = self.pop() {
                    observe::<VERIFY>(&mut result, x);
                }
            }
            _ => unreachable!(),
        }
        result
    }
}
impl<'a> NewOwner<'a> for JVec<u64> {
    #[inline(always)]
    fn new(_: &Case, _: &'a MemoryBudget) -> Self {
        JVec::new()
    }
}
impl<'a> NewOwner<'a> for JVec<u64, Unmetered, LengthLimit> {
    #[inline(always)]
    fn new(c: &Case, _: &'a MemoryBudget) -> Self {
        JVec::with_limit(c.limit)
    }
}
impl<'a> NewOwner<'a> for JVec<u64, Budget<'a>, Unlimited> {
    #[inline(always)]
    fn new(_: &Case, b: &'a MemoryBudget) -> Self {
        JVec::new_in(Budget::Limited(b))
    }
}
impl<'a> NewOwner<'a> for JVec<u64, Budget<'a>, LengthLimit> {
    #[inline(always)]
    fn new(c: &Case, b: &'a MemoryBudget) -> Self {
        JVec::<u64, _>::with_limit_in(c.limit, Budget::Limited(b))
    }
}

#[inline(never)]
pub fn epochs<'a, T: NewOwner<'a>, const OP: usize>(
    cases: &[Case],
    sources: &mut [T],
    budget: &'a MemoryBudget,
    count: u64,
) {
    for _ in 0..count {
        for (c, source) in cases.iter().zip(sources.iter_mut()) {
            let c = black_box(c);
            match OP {
                0 => {
                    let mut value = T::new(c, budget);
                    for x in &c.input {
                        value.push(black_box(*x));
                    }
                    black_box(value.slice());
                    drop(value);
                }
                1 => {
                    source.clear();
                    for x in &c.input {
                        source.push(black_box(*x));
                    }
                    black_box(source.slice());
                }
                2 => {
                    let mut value = T::new(c, budget);
                    value.copy(black_box(&c.input));
                    black_box(value.slice());
                    drop(value);
                }
                3 => {
                    let value = black_box(&*source).clone_owner();
                    black_box(value.slice());
                    drop(value);
                }
                4..=19 => {
                    let mut value = black_box(&*source).clone_owner();
                    value.edit::<OP, false>(c);
                    black_box(value.slice());
                    drop(value);
                }
                _ => unreachable!(),
            }
        }
    }
}
