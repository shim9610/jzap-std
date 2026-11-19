// Temporary call adapters for the remaining caller models.
// Every executable body calls an original product method; no state body is copied.
use crate::compaction::{Compaction as C, Extraction as E};
use crate::Error;
use vstd::prelude::*;
verus! {
spec fn cv(s:C)->bool { s.wf() }
spec fn ev(s:E)->bool { s.wf() }
fn c_full(original:usize)->(r:C)
 ensures cv(r), r@.read==0, r@.write==0, r@.original==original,
 { C::full(original) }
fn c_first_kept(original:usize)->(r:C)
 ensures cv(r), r@.read==r@.write, r@.read==if original==0 {0usize}else{1usize}, r@.original==original,
 { C::first_kept(original) }
fn c_try_prefix(start:usize,original:usize)->(r:Result<C,Error>)
 ensures match r { Ok(s)=>cv(s) && s@.read==start && s@.write==start && s@.original==original, Err(_)=>start>original },
 { C::try_prefix(start, original) }
fn c_read(s:&C)->(r:usize)
 ensures r==s@.read,
 { s.read() }
fn c_written(s:&C)->(r:usize)
 ensures r==s@.write,
 { s.written() }
fn c_peek(s:&C)->(r:Option<usize>)
 requires cv(*s), ensures match r {Some(v)=>s@.read<s@.original && v==s@.read,None=>s@.read==s@.original},
 { s.peek() }
fn c_previous_kept(s:&C)->(r:usize)
 requires cv(*s), s@.write>0, ensures r as int==s@.write as int-1,
 { s.previous_kept() }
fn c_remaining(s:&C)->(r:usize)
 requires cv(*s), ensures r as int==s@.original as int-s@.read as int, r<=s@.original,
 { s.remaining() }
fn c_restored_len(s:&C)->(r:usize)
 requires cv(*s), ensures r as int==s@.original as int-(s@.read as int-s@.write as int), r<=s@.original,
 { s.restored_len() }
fn c_keep_next(s:&mut C)->(r:bool)
 requires cv(*old(s)),
        ensures cv(*final(s)), final(s)@.original==old(s)@.original,
        if r {old(s)@.read<old(s)@.original && final(s)@.read as int==old(s)@.read as int+1 && final(s)@.write as int==old(s)@.write as int+1 } else {old(s)@.read==old(s)@.original && final(s)@.read==old(s)@.read && final(s)@.write==old(s)@.write },
 { s.keep_next() }
fn c_remove_next(s:&mut C)->(r:Option<usize>)
 requires cv(*old(s)),
        ensures cv(*final(s)), final(s)@.original==old(s)@.original,
        match r { Some(v)=>old(s)@.read<old(s)@.original && v==old(s)@.read && final(s)@.read as int==old(s)@.read as int+1 && final(s)@.write==old(s)@.write,
                   None=>old(s)@.read==old(s)@.original && final(s)@.read==old(s)@.read && final(s)@.write==old(s)@.write },
 { s.remove_next() }
fn c_keep_live(s:&mut C)
 requires cv(*old(s)),old(s)@.read<old(s)@.original,
            ensures cv(*final(s)),final(s)@.original==old(s)@.original,final(s)@.read as int==old(s)@.read as int+1,final(s)@.write as int==old(s)@.write as int+1,
 { s.keep_live() }
fn c_remove_live(s:&mut C)
 requires cv(*old(s)),old(s)@.read<old(s)@.original,
            ensures cv(*final(s)),final(s)@.original==old(s)@.original,final(s)@.read as int==old(s)@.read as int+1,final(s)@.write==old(s)@.write,
 { s.remove_live() }
fn e_try_range(start:usize,end:usize,total:usize)->(r:Result<E,Error>)
 ensures match r {Ok(s)=>ev(s) && s@.scan.read==start && s@.scan.write==start && s@.scan.original==end && s@.total==total,Err(_)=>start>end || end>total},
 { E::try_range(start, end, total) }
fn e_read(s:&E)->(r:usize)
 ensures r==s@.scan.read,
 { s.read() }
fn e_written(s:&E)->(r:usize)
 requires ev(*s), ensures r==s@.scan.write,
 { s.written() }
fn e_end(s:&E)->(r:usize)
 ensures r==s@.scan.original,
 { s.end() }
fn e_remaining(s:&E)->(r:usize)
 requires ev(*s), ensures r as int==s@.scan.original as int-s@.scan.read as int,
 { s.remaining() }
fn e_untouched(s:&E)->(r:bool)
 requires ev(*s), ensures r==(s@.scan.read==s@.scan.write),
 { s.untouched() }
fn e_unvisited_len(s:&E)->(r:usize)
 requires ev(*s), ensures r as int==s@.total as int-s@.scan.read as int,
 { s.unvisited_len() }
fn e_restored_len(s:&E)->(r:usize)
 requires ev(*s), ensures r as int==s@.total as int-(s@.scan.read as int-s@.scan.write as int), r<=s@.total,
 { s.restored_len() }
fn e_stop(s:&mut E)
 requires ev(*old(s)), ensures ev(*final(s)), final(s)@.scan.original==final(s)@.scan.read, final(s)@.scan.read==old(s)@.scan.read, final(s)@.scan.write==old(s)@.scan.write, final(s)@.total==old(s)@.total,
 { s.stop() }
fn e_keep_next(s:&mut E)->(r:bool)
 requires ev(*old(s)),
        ensures ev(*final(s)), final(s)@.total==old(s)@.total, final(s)@.scan.original==old(s)@.scan.original,
        if r {old(s)@.scan.read<old(s)@.scan.original && final(s)@.scan.read as int==old(s)@.scan.read as int+1 && final(s)@.scan.write as int==old(s)@.scan.write as int+1 } else {old(s)@.scan.read==old(s)@.scan.original && final(s)@.scan.read==old(s)@.scan.read && final(s)@.scan.write==old(s)@.scan.write},
 { s.keep_next() }
fn e_remove_next(s:&mut E)->(r:Option<usize>)
 requires ev(*old(s)),
        ensures ev(*final(s)), final(s)@.total==old(s)@.total, final(s)@.scan.original==old(s)@.scan.original,
        match r {Some(v)=>old(s)@.scan.read<old(s)@.scan.original && v==old(s)@.scan.read && final(s)@.scan.read as int==old(s)@.scan.read as int+1 && final(s)@.scan.write==old(s)@.scan.write,
                  None=>old(s)@.scan.read==old(s)@.scan.original && final(s)@.scan.read==old(s)@.scan.read && final(s)@.scan.write==old(s)@.scan.write},
 { s.remove_next() }
}

// Numeric caller contracts invoke the original closed iterator state.
verus! {
fn cursor_remaining(front: usize, back: usize) -> (r: usize)
    requires front <= back,
    ensures r as int == back as int - front as int,
{
    crate::span::Span::proof_from_state(front, back).len()
}
fn iterator_cursor(front: usize, back: usize, take_front: bool) -> (r: (usize, usize, usize))
    requires front <= back,
    ensures front <= r.0 <= r.1 <= back, r.2 as int == r.1 as int - r.0 as int,
        r.0 as int == front as int + if take_front && front < back { 1int } else { 0int },
        r.1 as int == back as int - if !take_front && front < back { 1int } else { 0int },
{
    let mut span = crate::span::Span::proof_from_state(front, back);
    if take_front { let _ = span.next(); } else { let _ = span.next_back(); }
    let start = span.front();
    let remaining = span.len();
    (start, start + remaining, remaining)
}
}
