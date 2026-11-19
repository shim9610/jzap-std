use crate::corpus::Case;
use jzap::{
    TryClone,
    boxed::Box as JBox,
    budget::{AllocationPolicy, Unmetered},
};
use std::hint::black_box;

pub trait Boxes {
    type Scalar: AsRef<u64>;
    type Slice: AsRef<[u8]>;
    type Text: AsRef<str>;
    fn scalar(&self, value: u64) -> Self::Scalar;
    fn scalar_clone(&self, source: &Self::Scalar) -> Self::Scalar;
    fn scalar_replace(&self, destination: &mut Self::Scalar, source: &Self::Scalar);
    fn slice(&self, input: &[u8]) -> Self::Slice;
    fn slice_clone(&self, source: &Self::Slice) -> Self::Slice;
    fn slice_replace(&self, destination: &mut Self::Slice, source: &Self::Slice);
    fn text(&self, input: &str) -> Self::Text;
    fn text_clone(&self, source: &Self::Text) -> Self::Text;
    fn text_replace(&self, destination: &mut Self::Text, source: &Self::Text);
    fn zeroed(&self, count: usize) -> Self::Slice;
    fn collect(&self, input: &[u8]) -> Self::Slice;
}
pub struct Standard;
impl Boxes for Standard {
    type Scalar = Box<u64>;
    type Slice = Box<[u8]>;
    type Text = Box<str>;
    #[inline(always)]
    fn scalar(&self, v: u64) -> Self::Scalar {
        Box::new(v)
    }
    #[inline(always)]
    fn scalar_clone(&self, v: &Self::Scalar) -> Self::Scalar {
        v.clone()
    }
    #[inline(always)]
    fn scalar_replace(&self, d: &mut Self::Scalar, s: &Self::Scalar) {
        d.clone_from(s);
    }
    #[inline(always)]
    fn slice(&self, v: &[u8]) -> Self::Slice {
        Box::from(v)
    }
    #[inline(always)]
    fn slice_clone(&self, v: &Self::Slice) -> Self::Slice {
        v.clone()
    }
    #[inline(always)]
    fn slice_replace(&self, d: &mut Self::Slice, s: &Self::Slice) {
        d.clone_from(s);
    }
    #[inline(always)]
    fn text(&self, v: &str) -> Self::Text {
        Box::from(v)
    }
    #[inline(always)]
    fn text_clone(&self, v: &Self::Text) -> Self::Text {
        v.clone()
    }
    #[inline(always)]
    fn text_replace(&self, d: &mut Self::Text, s: &Self::Text) {
        d.clone_from(s);
    }
    #[inline(always)]
    fn zeroed(&self, n: usize) -> Self::Slice {
        // SAFETY: every all-zero byte is a valid u8; the allocation is initialized.
        unsafe { Box::<[u8]>::new_zeroed_slice(n).assume_init() }
    }
    #[inline(always)]
    fn collect(&self, v: &[u8]) -> Self::Slice {
        v.iter().copied().collect()
    }
}
pub struct Ours<P: AllocationPolicy = Unmetered>(pub P);
impl<P: AllocationPolicy> Boxes for Ours<P> {
    type Scalar = JBox<u64, P>;
    type Slice = JBox<[u8], P>;
    type Text = JBox<str, P>;
    #[inline(always)]
    fn scalar(&self, v: u64) -> Self::Scalar {
        JBox::try_new_in(v, self.0.try_clone().unwrap()).unwrap()
    }
    #[inline(always)]
    fn scalar_clone(&self, v: &Self::Scalar) -> Self::Scalar {
        v.try_clone().unwrap()
    }
    #[inline(always)]
    fn scalar_replace(&self, d: &mut Self::Scalar, s: &Self::Scalar) {
        d.try_clone_from(s).unwrap();
    }
    #[inline(always)]
    fn slice(&self, v: &[u8]) -> Self::Slice {
        JBox::try_copy_from_slice_in(v, self.0.try_clone().unwrap()).unwrap()
    }
    #[inline(always)]
    fn slice_clone(&self, v: &Self::Slice) -> Self::Slice {
        v.try_clone().unwrap()
    }
    #[inline(always)]
    fn slice_replace(&self, d: &mut Self::Slice, s: &Self::Slice) {
        d.try_clone_from(s).unwrap();
    }
    #[inline(always)]
    fn text(&self, v: &str) -> Self::Text {
        JBox::try_from_str_in(v, self.0.try_clone().unwrap()).unwrap()
    }
    #[inline(always)]
    fn text_clone(&self, v: &Self::Text) -> Self::Text {
        v.try_clone().unwrap()
    }
    #[inline(always)]
    fn text_replace(&self, d: &mut Self::Text, s: &Self::Text) {
        d.try_clone_from(s).unwrap();
    }
    #[inline(always)]
    fn zeroed(&self, n: usize) -> Self::Slice {
        // SAFETY: every all-zero byte is a valid u8; the allocation is initialized.
        unsafe {
            JBox::<[std::mem::MaybeUninit<u8>], P>::assume_init(
                JBox::<[u8], P>::try_new_zeroed_slice_in(n, self.0.try_clone().unwrap()).unwrap(),
            )
        }
    }
    #[inline(always)]
    fn collect(&self, v: &[u8]) -> Self::Slice {
        JBox::try_from_iter_in(v.iter().copied(), self.0.try_clone().unwrap()).unwrap()
    }
}

pub struct Source<B: Boxes> {
    pub scalar: B::Scalar,
    pub scalar_target: B::Scalar,
    pub slice: B::Slice,
    pub slice_target: B::Slice,
    pub text: B::Text,
    pub text_target: B::Text,
}
pub fn prepare<B: Boxes>(boxes: &B, cases: &[Case]) -> Vec<Source<B>> {
    cases
        .iter()
        .map(|c| Source {
            scalar: boxes.scalar(c.value),
            scalar_target: boxes.scalar(!c.value),
            slice: boxes.slice(c.text.as_bytes()),
            slice_target: boxes.zeroed(c.text.len()),
            text: boxes.text(&c.text),
            text_target: boxes.text(&" ".repeat(c.text.len())),
        })
        .collect()
}

#[inline(never)]
pub fn epochs<B: Boxes, const OP: usize>(
    boxes: &B,
    cases: &[Case],
    sources: &mut [Source<B>],
    count: u64,
) {
    for _ in 0..count {
        for (c, source) in cases.iter().zip(sources.iter_mut()) {
            let c = black_box(c);
            match OP {
                20 => {
                    let v = boxes.scalar(black_box(c.value));
                    black_box(v.as_ref());
                    drop(v);
                }
                21 => {
                    let v = boxes.scalar_clone(black_box(&source.scalar));
                    black_box(v.as_ref());
                    drop(v);
                }
                22 => {
                    boxes.scalar_replace(
                        black_box(&mut source.scalar_target),
                        black_box(&source.scalar),
                    );
                    black_box(source.scalar_target.as_ref());
                }
                23 => {
                    let v = boxes.slice(black_box(c.text.as_bytes()));
                    black_box(v.as_ref());
                    drop(v);
                }
                24 => {
                    let v = boxes.slice_clone(black_box(&source.slice));
                    black_box(v.as_ref());
                    drop(v);
                }
                25 => {
                    boxes.slice_replace(
                        black_box(&mut source.slice_target),
                        black_box(&source.slice),
                    );
                    black_box(source.slice_target.as_ref());
                }
                26 => {
                    let v = boxes.text(black_box(&c.text));
                    black_box(v.as_ref());
                    drop(v);
                }
                27 => {
                    let v = boxes.text_clone(black_box(&source.text));
                    black_box(v.as_ref());
                    drop(v);
                }
                28 => {
                    boxes.text_replace(black_box(&mut source.text_target), black_box(&source.text));
                    black_box(source.text_target.as_ref());
                }
                29 => {
                    let v = boxes.zeroed(black_box(c.text.len()));
                    black_box(v.as_ref());
                    drop(v);
                }
                30 => {
                    let v = boxes.collect(black_box(c.text.as_bytes()));
                    black_box(v.as_ref());
                    drop(v);
                }
                _ => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
pub fn verify<B: Boxes>(boxes: &B, cases: &[Case]) {
    for c in cases {
        let scalar = boxes.scalar(c.value);
        assert_eq!(*scalar.as_ref(), c.value);
        assert_eq!(*boxes.scalar_clone(&scalar).as_ref(), c.value);
        let mut target = boxes.scalar(!c.value);
        boxes.scalar_replace(&mut target, &scalar);
        assert_eq!(*target.as_ref(), c.value);
        let slice = boxes.slice(c.text.as_bytes());
        assert_eq!(slice.as_ref(), c.text.as_bytes());
        assert_eq!(boxes.slice_clone(&slice).as_ref(), c.text.as_bytes());
        let mut target = boxes.zeroed(c.text.len());
        assert!(target.as_ref().iter().all(|x| *x == 0));
        boxes.slice_replace(&mut target, &slice);
        assert_eq!(target.as_ref(), c.text.as_bytes());
        assert_eq!(boxes.collect(c.text.as_bytes()).as_ref(), c.text.as_bytes());
        let text = boxes.text(&c.text);
        assert_eq!(text.as_ref(), c.text);
        assert_eq!(boxes.text_clone(&text).as_ref(), c.text);
        let mut target = boxes.text("");
        boxes.text_replace(&mut target, &text);
        assert_eq!(target.as_ref(), c.text);
    }
}
