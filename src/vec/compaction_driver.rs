//! Executable compaction control; physical ownership stays in the caller's leaves.
use crate::compaction::{Compaction, Extraction};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[inline(always)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(state).wf(),old(state)@.original==end,old(state)@.read==old(state)@.write,
        forall|i:usize| old(state)@.read<=i<end ==> call_requires(*step,(i,i)),
    ensures final(state).wf(),final(state)@.original==end,
        forall|read:usize,write:usize| call_requires(*final(step),(read,write))==call_requires(*old(step),(read,write)),
        old(state)@.read<=final(state)@.read<=end,final(state)@.write==final(state)@.read,
        match out {Ok(done)=>done==(final(state)@.read==end),Err(_)=>final(state)@.read<end},
))]
fn prefix<E, F: FnMut(usize, usize) -> Result<bool, E>>(
    state: &mut Compaction,
    end: usize,
    step: &mut F,
) -> Result<bool, E> {
    #[cfg(all(kani, jzap_loop_contracts))]
    let start = state.read();
    #[cfg(verus_keep_ghost)]
    let mut history = verus_exec_expr!(Ghost(Seq::<(usize, F)>::empty()));
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant state.wf(),state@.original==end,state@.write==state@.read,
            forall|read:usize,write:usize| call_requires(*step,(read,write))==call_requires(*old(step),(read,write)),
            old(state)@.read<=state@.read<=end,
            history@.len()==state@.read as int-old(state)@.read as int,
            forall|j:int| 0<=j<history@.len() ==> history@[j].0 as int==old(state)@.read as int+j
                && call_ensures(history@[j].1,(history@[j].0,history@[j].0),Ok(true)),
            forall|i:usize| old(state)@.read<=i<end ==> call_requires(*step,(i,i)),
        decreases end-state@.read,
    ))]
    #[cfg_attr(all(kani,jzap_loop_contracts),kani::loop_invariant(state.valid_for(end) && start<=state.read() && state.read()==state.written()))]
    while state.read() < end {
        let index = state.read();
        #[cfg(verus_keep_ghost)]
        let before = verus_exec_expr!(Ghost(*step));
        let result = step(index, index);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(call_ensures(before@,(index,index),result));
            if result matches Ok(true) {*history=history@.push((index,before@));}
        }
        if !result? {
            return Ok(false);
        }
        let _ = state.keep_next();
    }
    Ok(true)
}

#[inline(always)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(state).wf(),old(state)@.write<old(state)@.read<old(state)@.original,
        call_requires(*step,(old(state)@.read,old(state)@.write)),
        forall|keep:bool| call_requires(*act,(old(state)@.read,old(state)@.write,keep)),
    ensures final(state).wf(),final(state)@.original==old(state)@.original,
        call_ensures(*old(step),(old(state)@.read,old(state)@.write),out),
        match out {Ok(keep)=>call_ensures(*old(act),(old(state)@.read,old(state)@.write,keep),()),Err(_)=>true},
        forall|read:usize,write:usize| call_requires(*final(step),(read,write))==call_requires(*old(step),(read,write)),
        forall|read:usize,write:usize,keep:bool| call_requires(*final(act),(read,write,keep))==call_requires(*old(act),(read,write,keep)),
        final(state)@.write<final(state)@.read,
        match out {
            Ok(keep)=>final(state)@.read as int==old(state)@.read as int+1
                && final(state)@.write as int==old(state)@.write as int+if keep {1int}else{0int},
            Err(_)=>final(state)@.read==old(state)@.read && final(state)@.write==old(state)@.write,
        },
))]
fn compact_one<E, F: FnMut(usize, usize) -> Result<bool, E>, G: FnMut(usize, usize, bool)>(
    state: &mut Compaction,
    step: &mut F,
    act: &mut G,
) -> Result<bool, E> {
    let read = state.read();
    let write = state.written();
    let keep = step(read, write)?;
    if keep {
        state.keep_live();
    } else {
        state.remove_live();
    }
    act(read, write, keep);
    Ok(keep)
}

#[inline(always)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(state).wf(),old(state)@.original==end,old(state)@.write<old(state)@.read,
        forall|read:usize,write:usize| old(state)@.read<=read<end && old(state)@.write<=write<read
            ==> call_requires(*step,(read,write)),
        forall|read:usize,write:usize,keep:bool| old(state)@.read<=read<end && old(state)@.write<=write<read
            ==> call_requires(*act,(read,write,keep)),
    ensures final(state).wf(),final(state)@.original==end,
        old(state)@.read<=final(state)@.read<=end,
        old(state)@.write<=final(state)@.write<final(state)@.read,
        out.is_ok() ==> final(state)@.read==end,out.is_err() ==> final(state)@.read<end,
))]
fn sweep<
    const PAIRED: bool,
    E,
    F: FnMut(usize, usize) -> Result<bool, E>,
    G: FnMut(usize, usize, bool),
>(
    state: &mut Compaction,
    end: usize,
    step: &mut F,
    act: &mut G,
) -> Result<(), E> {
    #[cfg(all(kani, jzap_loop_contracts))]
    let minimum = state.written();
    let boundary = if PAIRED { end.saturating_sub(1) } else { end };
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant state.wf(),state@.original==end,state@.write<state@.read,
            old(state)@.read<=state@.read<=end,old(state)@.write<=state@.write,
            boundary as int==end as int-if PAIRED {1int}else{0int},
            forall|read:usize,write:usize| old(state)@.read<=read<end && old(state)@.write<=write<read
                ==> call_requires(*step,(read,write)),
            forall|read:usize,write:usize,keep:bool| old(state)@.read<=read<end && old(state)@.write<=write<read
                ==> call_requires(*act,(read,write,keep)),
        decreases end-state@.read,
    ))]
    #[cfg_attr(all(kani,jzap_loop_contracts),kani::loop_invariant(state.valid_for(end) && minimum<=state.written() && state.written()<state.read()))]
    while state.read() < boundary {
        let _ = compact_one(state, step, act)?;
        if PAIRED {
            let _ = compact_one(state, step, act)?;
        }
    }
    if PAIRED && state.read() < end {
        let _ = compact_one(state, step, act)?;
    }
    Ok(())
}

#[inline(always)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(state).wf(),old(state)@.original==end,old(state)@.write==old(state)@.read,
        forall|read:usize,write:usize| old(state)@.read<=read<end && old(state)@.write<=write<=read
            ==> call_requires(step,(read,write)),
        forall|read:usize,write:usize,keep:bool| old(state)@.read<=read<end && old(state)@.write<=write<=read
            && (keep ==> write<read) ==> call_requires(act,(read,write,keep)),
    ensures final(state).wf(),final(state)@.original==end,
        old(state)@.read<=final(state)@.read<=end,old(state)@.write<=final(state)@.write<=final(state)@.read,
        out.is_ok() ==> final(state)@.read==end,out.is_err() ==> final(state)@.read<end,
))]
pub(crate) fn run<
    const PAIRED: bool,
    E,
    F: FnMut(usize, usize) -> Result<bool, E>,
    G: FnMut(usize, usize, bool),
>(
    state: &mut Compaction,
    end: usize,
    mut step: F,
    mut act: G,
) -> Result<(), E> {
    if prefix(state, end, &mut step)? {
        return Ok(());
    }
    let Some(read) = state.remove_next() else {
        return Ok(());
    };
    act(read, state.written(), false);
    sweep::<PAIRED, E, F, G>(state, end, &mut step, &mut act)
}

#[inline(always)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires call_requires(*step,(read,)),
    ensures call_ensures(*old(step),(read,),out),
        forall|i:usize| call_requires(*final(step),(i,))==call_requires(*old(step),(i,)),
))]
fn extraction_decision<E, F: FnMut(usize) -> Result<bool, E>>(
    step: &mut F,
    read: usize,
) -> Result<bool, E> {
    step(read)
}

#[inline]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(state).wf(),
        forall|read:usize| old(state)@.scan.read<=read<old(state)@.scan.original ==> call_requires(step,(read,)),
        forall|read:usize,write:usize| old(state)@.scan.read<=read<old(state)@.scan.original
            && old(state)@.scan.write<=write<read ==> call_requires(act,(read,write)),
    ensures final(state).wf(),final(state)@.total==old(state)@.total,
        old(state)@.scan.read<=final(state)@.scan.read<=old(state)@.scan.original,
        old(state)@.scan.write<=final(state)@.scan.write,
        match out {
            Ok(None)=>final(state)@.scan.original==old(state)@.scan.original
                && final(state)@.scan.read==old(state)@.scan.original,
            Ok(Some(index))=>old(state)@.scan.read<=index<old(state)@.scan.original
                && final(state)@.scan.read as int==index as int+1
                && final(state)@.scan.write<=index
                && final(state)@.scan.original==old(state)@.scan.original,
            Err(_)=>final(state)@.scan.original==old(state)@.scan.original
                && final(state)@.scan.read<old(state)@.scan.original,
        },
))]
pub(crate) fn extract_next<E, F: FnMut(usize) -> Result<bool, E>, G: FnMut(usize, usize)>(
    state: &mut Extraction,
    mut step: F,
    mut act: G,
) -> Result<Option<usize>, E> {
    let end = state.end();
    #[cfg(all(kani, jzap_loop_contracts))]
    let (minimum_read, minimum_write, total) = (state.read(), state.written(), state.total());
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant state.wf(),state@.total==old(state)@.total,
            state@.scan.original==old(state)@.scan.original,end==state@.scan.original,
            old(state)@.scan.read<=state@.scan.read<=end,
            old(state)@.scan.write<=state@.scan.write,
            forall|read:usize| old(state)@.scan.read<=read<end ==> call_requires(step,(read,)),
            forall|read:usize,write:usize| old(state)@.scan.read<=read<end
                && old(state)@.scan.write<=write<read ==> call_requires(act,(read,write)),
        decreases end-state@.scan.read,
    ))]
    #[cfg_attr(all(kani,jzap_loop_contracts),kani::loop_invariant(state.valid_progress(end, total, minimum_read, minimum_write)))]
    while state.read() < end {
        let read = state.read();
        if extraction_decision(&mut step, read)? {
            let _ = state.remove_next();
            return Ok(Some(read));
        }
        if !state.untouched() {
            act(read, state.written());
        }
        let _ = state.keep_next();
    }
    Ok(None)
}
