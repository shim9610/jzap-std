//! Experimental typed extraction scan, copied into an ignored library build.
//! This module has no product proof or adoption claim.
use crate::arithmetic::bounded::wrapping_sub;
use crate::compaction::Extraction;
use core::num::NonZeroUsize;

trait Removed: Copy {
    fn count(self) -> usize;
}

#[derive(Clone, Copy)]
struct Prefix;
impl Removed for Prefix {
    #[inline(always)]
    fn count(self) -> usize {
        0
    }
}

#[derive(Clone, Copy)]
struct Gapped(NonZeroUsize);
impl Removed for Gapped {
    #[inline(always)]
    fn count(self) -> usize {
        self.0.get()
    }
}

impl Removed for usize {
    #[inline(always)]
    fn count(self) -> usize {
        self
    }
}

#[cfg_attr(jzap_phase_inline_hint, inline)]
#[cfg_attr(jzap_phase_outline, inline(never))]
#[cfg_attr(
    not(any(jzap_phase_inline_hint, jzap_phase_outline, jzap_phase_inline_auto)),
    inline(always)
)]
fn scan<const HAS_GAP: bool, C: Removed, E, F, G>(
    state: &mut Extraction,
    removed: C,
    step: &mut F,
    act: &mut G,
) -> Result<Option<usize>, E>
where
    F: FnMut(usize) -> Result<bool, E>,
    G: FnMut(usize, usize),
{
    let end = state.end();
    while state.read() < end {
        let read = state.read();
        if super::extraction_decision(step, read)? {
            let _ = state.remove_next();
            return Ok(Some(read));
        }
        if HAS_GAP {
            act(read, wrapping_sub(read, removed.count()));
        }
        let _ = state.keep_next();
    }
    Ok(None)
}

#[inline(always)]
pub(super) fn run<E, F, G>(
    state: &mut Extraction,
    mut step: F,
    mut act: G,
) -> Result<Option<usize>, E>
where
    F: FnMut(usize) -> Result<bool, E>,
    G: FnMut(usize, usize),
{
    #[cfg(jzap_phase_peel)]
    {
        // Preserve selected-first entry: construct a phase witness only after
        // the first decision retains an element. Every predicate still runs once.
        let read = state.read();
        if read >= state.end() {
            return Ok(None);
        }
        if super::extraction_decision(&mut step, read)? {
            let _ = state.remove_next();
            return Ok(Some(read));
        }
        if !state.untouched() {
            act(read, state.written());
        }
        let _ = state.keep_next();
    }
    // The deletion count is unchanged within a scan: selecting returns at once.
    let removed = wrapping_sub(state.read(), state.written());
    #[cfg(jzap_phase_usize)]
    {
        if removed == 0 {
            scan::<false, _, _, _, _>(state, 0usize, &mut step, &mut act)
        } else {
            scan::<true, _, _, _, _>(state, removed, &mut step, &mut act)
        }
    }
    #[cfg(not(jzap_phase_usize))]
    {
        match NonZeroUsize::new(removed) {
            None => scan::<false, _, _, _, _>(state, Prefix, &mut step, &mut act),
            Some(removed) => scan::<true, _, _, _, _>(state, Gapped(removed), &mut step, &mut act),
        }
    }
}
