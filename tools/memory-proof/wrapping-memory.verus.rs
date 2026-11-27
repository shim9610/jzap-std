// Appended to freshly source-generated numeric and closed-state implementations.
// These lemmas connect element spans to byte extents. A valid allocator result
// and the caller contracts of explicit unsafe raw-storage imports are boundaries.
verus! {
// Conditional arithmetic obligation; the shared protocol separately supplies
// the native Linux x86-64 and reduced-count model execution bounds.
proof fn shared_buffered_reservation_extent(live:usize,pending:usize)
    requires 0<live<=isize::MAX as usize,pending<isize::MAX as usize,
    ensures live as int+pending as int+1<=usize::MAX as int,
{}
proof fn shared_release_credit(old: usize)
    requires old > 0,
    ensures old as int - 1 >= 0, old as int - 1 < old,
{}
spec fn live(write: usize, read: usize, total: usize, index: int) -> bool {
    0 <= index < write || read <= index < total
}

// The initialized footprint is a prefix plus an unvisited suffix. Closing the
// real cursor transitions proves that a move writes into a hole, and removal
// excludes an element before its destructor/caller can observe it again.
fn compaction_ownership_step(s: &mut C, keep: bool)
    requires cv(*old(s)),
    ensures cv(*final(s)),
        forall|i:int| live(final(s)@.write, final(s)@.read, final(s)@.original, i)
            == if old(s)@.read == old(s)@.original {
                live(old(s)@.write, old(s)@.read, old(s)@.original, i)
            } else {
                (live(old(s)@.write, old(s)@.read, old(s)@.original, i) && i != old(s)@.read)
                    || (keep && i == old(s)@.write)
            },
{
    if c_peek(s).is_none() { return; }
    if keep { let advanced = c_keep_next(s); assert(advanced); }
    else { let removed = c_remove_next(s); assert(removed.is_some()); }
}

fn extraction_ownership_step(s: &mut E, action: u8)
    requires ev(*old(s)),
    ensures ev(*final(s)), final(s)@.total == old(s)@.total,
        forall|i:int| live(final(s)@.scan.write, final(s)@.scan.read, final(s)@.total, i)
            == if old(s)@.scan.read == old(s)@.scan.original || action > 1 {
                live(old(s)@.scan.write, old(s)@.scan.read, old(s)@.total, i)
            } else {
                (live(old(s)@.scan.write, old(s)@.scan.read, old(s)@.total, i) && i != old(s)@.scan.read)
                    || (action == 0 && i == old(s)@.scan.write)
            },
{
    if action > 1 { e_stop(s); return; }
    if e_remaining(s) == 0 { return; }
    if action == 0 { let advanced = e_keep_next(s); assert(advanced); }
    else { let removed = e_remove_next(s); assert(removed.is_some()); }
}

spec fn restored_index(write: usize, read: usize, index: int) -> int {
    if index < write { index } else { index - read + write }
}
spec fn original_index(write: usize, read: usize, index: int) -> int {
    if index < write { index } else { index - write + read }
}
proof fn restoration_transfers_each_live_index_once(write: usize, read: usize, total: usize)
    requires write <= read <= total,
    ensures
        forall|i:int| live(write, read, total, i) ==>
            0 <= restored_index(write, read, i) < write as int + total as int - read as int,
        forall|i:int,j:int| live(write, read, total, i) && live(write, read, total, j)
            && restored_index(write, read, i) == restored_index(write, read, j) ==> i == j,
        forall|j:int| 0 <= j < write as int + total as int - read as int ==>
            #[trigger] live(write, read, total, original_index(write, read, j)),
{
}
// An unconstrained word covers every possible output of wrapping add/sub/mul.
// The stored byte size is checked before construction; reconstruction cannot
// allocate a smaller object through modular multiplication.
fn external_count_before_allocation(count: usize, size: usize) -> (r: Option<usize>)
    ensures r.is_some() == byte_valid(count, size),
        r.is_some() ==> r.unwrap() as int == count as int * size as int,
{
    let Some(bytes) = checked_array_bytes(count, size) else { return None; };
    let reconstructed = capacity_bytes(count, size);
    assert(bytes == reconstructed);
    Some(bytes)
}

fn checked_index_before_access(index: usize, len: usize, cap: usize, size: usize) -> (r: bool)
    requires len <= cap, byte_valid(cap, size),
    ensures r == (index < len),
{
    if INDEX_REJECT { return false; }
    proof { memory_span(cap, size, index, 1); }
    true
}

// Derive the actual post-reservation capacity rather than assuming that an
// arbitrary allocation is large enough. Storage refusal precedes publication.
fn memory_reserve(len: usize, cap: usize, limit: usize, size: usize,
    additional: usize, exact: bool, storage_accepts: bool) -> (r: Option<(usize, usize)>)
    requires len <= cap <= limit, byte_valid(cap, size),
    ensures match r {
        Some((needed, allocated)) => needed as int == len as int + additional as int
            && needed <= allocated <= limit && cap <= allocated && byte_valid(allocated, size),
        None => true,
    },
{
    let Ok(needed) = needed_length(len, additional, limit) else { return None; };
    if needed <= cap { return Some((needed, cap)); }
    let Ok(allocated)=crate::vec::geometry::growth_for_layout(cap,needed,limit,size,exact)
        else {return None;};
    let Some(bytes) = checked_array_bytes(allocated, size) else { return None; };
    if bytes > 0 && !storage_accepts { return None; }
    Some((needed, allocated))
}

fn insertion_before_moves(len: usize, cap: usize, limit: usize, size: usize,
                          index: usize, allocator_accepts: bool) -> (r: bool)
    requires len <= cap <= limit, byte_valid(cap, size),
    ensures r ==> index <= len && (len as int + 1) <= limit as int,
{
    // The product kernel of Vec::try_insert; the caller routing is checked
    // against the source by verify_wrapping_memory.py.
    let Ok(shifted) = crate::vec::geometry::insertion(crate::Checked::new(len), index)
        else { return false; };
    let shifted = shifted.get();
    let Some((needed, accepted_cap)) = memory_reserve(len, cap, limit, size, 1, false, allocator_accepts)
        else { return false; };
    proof {
        memory_span(accepted_cap, size, index, shifted);
        memory_span(accepted_cap, size, index + 1, shifted);
        memory_span(accepted_cap, size, index, 1);
    }
    true
}

fn removal_before_moves(len: usize, cap: usize, size: usize, index: usize) -> (r: bool)
    requires len <= cap, byte_valid(cap, size),
    ensures r == (index < len),
{
    // The product kernel of Vec::remove.
    let Ok((next, shifted)) = crate::vec::geometry::removal(crate::Checked::new(len), index)
        else { return false; };
    let shifted = shifted.get();
    proof {
        memory_span(cap, size, index, 1);
        memory_span(cap, size, index + 1, shifted);
        memory_span(cap, size, index, shifted);
    }
    true
}

fn swap_remove_before_moves(len: usize, cap: usize, size: usize, index: usize) -> (r: bool)
    requires len <= cap, byte_valid(cap, size),
    ensures r == (index < len),
{
    // The product kernel of Vec::swap_remove.
    let Ok(next) = crate::vec::geometry::shortened(crate::Checked::new(len), index)
        else { return false; };
    let next = next.get();
    proof {
        memory_span(cap, size, index, 1);
        memory_span(cap, size, next, 1);
    }
    true
}

fn reservation_before_bulk_write(len: usize, cap: usize, limit: usize, size: usize,
    additional: usize, allocator_accepts: bool) -> (r: bool)
    requires len <= cap <= limit, byte_valid(cap, size),
    ensures r ==> len as int + additional as int <= limit as int,
{
    let Some((needed, accepted_cap)) = memory_reserve(len, cap, limit, size, additional, false, allocator_accepts)
        else { return false; };
    proof { memory_span(accepted_cap, size, len, additional); }
    true
}

fn spare_view_before_slice(len: usize, cap: usize, size: usize) -> (count: usize)
    requires len <= cap, byte_valid(cap, size),
    ensures len as int + count as int == cap as int,
{
    let count = owner_spare_capacity(len, cap);
    proof { memory_span(cap, size, SPARE_START, count); }
    count
}

fn cursor_before_read(front: usize, back: usize, cap: usize, size: usize, reverse: bool)
    requires front <= back <= cap, byte_valid(cap, size),
{
    if front == back { return; }
    let (new_front, new_back, remaining) = iterator_cursor(front, back, !reverse);
    let index = if reverse { new_back } else { front };
    proof {
        memory_span(cap, size, index, 1);
        memory_span(cap, size, new_front, remaining);
    }
}

fn compaction_before_move(s: C, cap: usize, size: usize)
    requires cv(s), s@.original <= cap, byte_valid(cap, size),
{
    let remaining = c_remaining(&s);
    let restored = c_restored_len(&s);
    proof {
        memory_span(cap, size, COMPACT_SOURCE, remaining);
        memory_span(cap, size, COMPACT_DESTINATION, remaining);
    }
    assert(restored as int == s@.write as int + remaining as int);
}

fn compaction_before_element(s: &mut C, cap: usize, size: usize, kept: bool)
    requires cv(*old(s)), old(s)@.original <= cap, byte_valid(cap, size),
    ensures cv(*final(s)), final(s)@.original <= cap,
{
    let Some(index) = c_peek(s) else { return; };
    let write = c_written(s);
    proof { memory_span(cap, size, index, 1); }
    if kept {
        let advanced = c_keep_next(s);
        assert(advanced);
        proof { memory_span(cap, size, write, 1); }
        if write < index { proof { ordered_elements_do_not_overlap(size, write, 1, index); } }
    } else {
        let removed = c_remove_next(s);
        assert(removed == Some(index));
    }
}

fn extraction_before_element(s: &mut E, cap: usize, size: usize, selected: bool, failed: bool)
    requires ev(*old(s)), old(s)@.total <= cap, byte_valid(cap, size),
    ensures ev(*final(s)), final(s)@.total == old(s)@.total,
{
    let read = e_read(s);
    let end = e_end(s);
    if read == end { return; }
    proof { memory_span(cap, size, read, 1); }
    if failed { e_stop(s); return; }
    if selected {
        let removed = e_remove_next(s);
        assert(removed == Some(read));
    } else {
        let write = e_written(s);
        proof { memory_span(cap, size, write, 1); }
        if write < read { proof { ordered_elements_do_not_overlap(size, write, 1, read); } }
        let advanced = e_keep_next(s);
        assert(advanced);
    }
}

fn internal_copy_before_append(len: usize, cap: usize, limit: usize, size: usize,
    start: usize, end: usize, allocator_accepts: bool) -> (ok: bool)
    requires len <= cap <= limit, byte_valid(cap, size),
{
    let Some(count) = internal_range_length(start, end, len) else { return false; };
    let Some((needed, accepted_cap)) = memory_reserve(len, cap, limit, size, count, false, allocator_accepts)
        else { return false; };
    proof {
        assert(WITHIN_COPY_SOURCE as int + WITHIN_COPY_COUNT as int <= len as int);
        assert(WITHIN_COPY_DESTINATION == len && WITHIN_COPY_COUNT == count);
        memory_span(accepted_cap, size, WITHIN_COPY_SOURCE, WITHIN_COPY_COUNT);
        memory_span(accepted_cap, size, WITHIN_COPY_DESTINATION, WITHIN_COPY_COUNT);
        ordered_elements_do_not_overlap(size, start, count, len);
    }
    true
}

fn clone_range_element_before_write(original: usize, target: usize, cap: usize, size: usize,
    start: usize, end: usize, index: usize, prefix: usize)
    requires start <= index < end <= original, target <= cap, byte_valid(cap, size),
        target as int == original as int + end as int - start as int,
        prefix as int == original as int + index as int - start as int,
{
    let source = CLONE_SOURCE_INDEX;
    let destination = CLONE_DESTINATION_INDEX;
    assert(source < original);
    assert(destination == prefix && prefix < target);
    proof {
        memory_span(cap, size, source, 1);
        memory_span(cap, size, destination, 1);
        ordered_elements_do_not_overlap(size, source, 1, destination);
    }
}

fn extraction_before_move(s: E, cap: usize, size: usize)
    requires ev(s), s@.total <= cap, byte_valid(cap, size),
{
    let remaining = e_unvisited_len(&s);
    let restored = e_restored_len(&s);
    proof {
        memory_span(cap, size, EXTRACT_SOURCE, remaining);
        memory_span(cap, size, EXTRACT_DESTINATION, remaining);
    }
    assert(restored as int == s@.scan.write as int + remaining as int);
}

// The Drain words at `restore_tail`. With an empty tail the length may lie
// above `tail_start`: `Splice::try_finish` then publishes through the owner
// and never moves `tail_start`.
fn drain_before_tail_restore(len: usize, tail_start: usize, tail_len: usize, cap: usize, size: usize)
    requires tail_start as int + tail_len as int <= cap as int,
        len <= tail_start || (tail_len == 0 && len <= cap),
        byte_valid(cap, size),
{
    let restored = DRAIN_RESTORED;
    proof {
        memory_span(cap, size, DRAIN_SOURCE, tail_len);
        memory_span(cap, size, DRAIN_DESTINATION, tail_len);
    }
    assert(restored <= cap);
}

fn splice_before_tail_move(len: usize, tail_start: usize, tail_len: usize,
    additional: usize, cap: usize, size: usize)
    requires len == tail_start, tail_start as int + tail_len as int <= cap as int,
        byte_valid(cap, size),
{
    let Some(destination) = splice_move_tail(len, tail_start, tail_len, additional, cap) else { return; };
    proof {
        memory_span(cap, size, SPLICE_MOVE_SOURCE, tail_len);
        memory_span(cap, size, SPLICE_MOVE_DESTINATION, tail_len);
    }
}

fn staged_splice_before_moves(len: usize, tail_start: usize, tail_len: usize,
    extra_len: usize, cap: usize, size: usize)
    requires len == tail_start,
        len as int + tail_len as int + extra_len as int <= cap as int,
        byte_valid(cap, size),
{
    let (destination, next_len) = staged_splice(len, tail_start, tail_len, extra_len, cap);
    proof {
        memory_span(cap, size, STAGED_TAIL_SOURCE, tail_len);
        memory_span(cap, size, STAGED_TAIL_DESTINATION, tail_len);
        memory_span(cap, size, STAGED_EXTRA_DESTINATION, extra_len);
    }
    assert(next_len as int + tail_len as int <= cap as int);
}
fn initialized_prefix_before_write(prefix: usize, target: usize, cap: usize, size: usize)
    requires prefix < target <= cap, byte_valid(cap, size),
{
    proof { memory_span(cap, size, prefix, 1); }
    let next = prefix.checked_add(1);
    assert(next.is_some() && next.unwrap() <= target);
}
}
fn main() {}
