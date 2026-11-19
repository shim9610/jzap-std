use vstd::arithmetic::div_mod::*;
// Appended to the freshly generated budget model. This proves numeric state
// transitions; allocator validity, Rust ownership and callback contracts remain
// explicit boundaries, not assumptions hidden inside assume/external_body.
verus! {
fn allocation_accounting(limit: usize, occupied: usize, available: usize,
                         bytes: usize, allocator_accepts: bool) -> (r: Option<(usize, usize)>)
    requires occupied as int + available as int <= limit as int,
    ensures r.is_some() ==> r.unwrap() == (occupied, available),
{
    let Some(remaining) = reserve(limit, occupied, available, bytes) else { return None; };
    if !allocator_accepts {
        let restored = cancel(limit, occupied, remaining, bytes);
        assert(restored == available);
        return None;
    }
    let charged = commit(limit, occupied, remaining, bytes);
    // Commit creates this owner's charge, establishing release's precondition.
    assert(bytes <= charged);
    let restored = release(limit, charged, remaining, bytes);
    assert(restored == (occupied, available));
    Some(restored)
}

fn reallocation_accounting(limit: usize, occupied: usize, available: usize,
                           old_bytes: usize, new_bytes: usize, allocator_accepts: bool)
    -> (r: (usize, usize))
    requires occupied as int + available as int <= limit as int, old_bytes <= occupied,
    ensures r.0 as int + r.1 as int <= limit as int,
            r == (occupied, available) ||
                (r.0 as int == occupied as int - old_bytes as int + new_bytes as int
                 && r.1 as int == available as int + old_bytes as int - new_bytes as int
                 && new_bytes <= r.0),
{
    if old_bytes == new_bytes { return (occupied, available); }
    let Some(remaining) = reserve(limit, occupied, available, new_bytes) else {
        return (occupied, available);
    };
    if !allocator_accepts {
        let restored = cancel(limit, occupied, remaining, new_bytes);
        assert(restored == available);
        return (occupied, restored);
    }
    let charged = commit(limit, occupied, remaining, new_bytes);
    let restored = release(limit, charged, remaining, old_bytes);
    assert(new_bytes <= restored.0);
    restored
}

// Other owners may reserve, commit, cancel or release while this owner remains
// live. Each transition preserves this owner's existing release entitlement.
fn other_owner_transition(limit: usize, occupied: usize, available: usize,
                          owned: usize, pending: usize, bytes: usize, action: u8) -> (r: (usize, usize))
    requires occupied as int + available as int <= limit as int, owned <= occupied,
        pending as int <= limit as int - occupied as int - available as int,
    ensures owned <= r.0, r.0 as int + r.1 as int <= limit as int,
        pending as int <= limit as int - r.0 as int - r.1 as int,
{
    if action == 0 {
        match reserve(limit, occupied, available, bytes) {
            Some(a) => (occupied, a), None => (occupied, available),
        }
    } else if action == 1 {
        if bytes > reserved(limit, occupied, available) - pending {
            return (occupied, available);
        }
        (commit(limit, occupied, available, bytes), available)
    } else if action == 2 {
        if bytes > reserved(limit, occupied, available) - pending {
            return (occupied, available);
        }
        (occupied, cancel(limit, occupied, available, bytes))
    } else {
        if bytes > occupied - owned { return (occupied, available); }
        release(limit, occupied, available, bytes)
    }
}


fn internal_range_length(start: usize, end: usize, len: usize) -> (r: Option<usize>)
    ensures r.is_some() == (start <= end && end <= len),
        r.is_some() ==> r.unwrap() as int == end as int - start as int,
{
    if !normalized_range(start, end, len) { return None; }
    Some(INTERNAL_RANGE_LENGTH)
}

fn clone_input_offset(start: usize, index: usize, target: usize, source_len: usize) -> (r: usize)
    requires start <= index < target,
        target as int == start as int + source_len as int,
    ensures r < source_len, r as int == index as int - start as int,
{ CLONE_INPUT_OFFSET }

fn split_length(len: usize, at: usize) -> (r: Option<usize>)
    ensures r.is_some() == (at <= len),
        r.is_some() ==> r.unwrap() as int == len as int - at as int,
{
    if SPLIT_REJECT { return None; }
    Some(SPLIT_LENGTH)
}

fn drain_tail_length(start: usize, end: usize, len: usize) -> (r: Option<usize>)
    ensures r.is_some() == (start <= end && end <= len),
        r.is_some() ==> r.unwrap() as int == len as int - end as int,
{
    if !normalized_range(start, end, len) { return None; }
    Some(DRAIN_TAIL_LENGTH)
}

fn capacity_reservation(len: usize, cap: usize, limit: usize, additional: usize)
    -> (r: Option<usize>)
    requires len <= cap <= limit,
    ensures match r {
        Some(needed) => needed as int == len as int + additional as int && needed <= limit,
        None => len as int + additional as int > limit as int,
    },
{
    // The existing spare-capacity fast path establishes checked addition's
    // precondition without computing the sum on the success path.
    if !(RESERVE_GROW) {
        assert(len as int + additional as int <= cap as int);
        return Some(len + additional);
    }
    let Some(needed) = len.checked_add(additional) else { return None; };
    if needed > limit { return None; }
    Some(needed)
}

fn owned_resize_prefix(original: usize, target: usize, cap: usize, fail_at: usize)
    -> (r: (bool, usize))
    requires original < target <= cap,
    ensures r.0 ==> r.1 == target, !r.0 ==> r.1 == original,
{
    let mut len = original;
    while OWNED_RESIZE_GUARD
        invariant original <= len <= target - 1, target <= cap,
        decreases target - len,
    {
        if len == fail_at { return (false, original); }
        OWNED_RESIZE_ADVANCE
    }
    OWNED_RESIZE_FINAL
    (true, len)
}

fn owned_resize_operation(len: usize, cap: usize, limit: usize, target: usize,
                          storage_accepts: bool, fail_at: usize) -> (r: (bool, usize))
    requires len <= cap <= limit,
    ensures r.1 <= limit, r.0 ==> r.1 == target, !r.0 ==> r.1 == len,
{
    if RESIZE_SHRINK { return (true, target); }
    let additional = RESIZE_ADDITIONAL;
    let Some(needed) = capacity_reservation(len, cap, limit, additional) else {
        return (false, len);
    };
    assert(needed == target);
    if needed > cap && !storage_accepts { return (false, len); }
    owned_resize_prefix(len, target, cap.max(needed), fail_at)
}

fn clone_within_prefix(original: usize, cap: usize, start: usize, end: usize, fail_at: usize)
    -> (r: (bool, usize))
    requires start <= end <= original,
             original as int + (end as int - start as int) <= cap as int,
    ensures r.0 ==> r.1 as int == original as int + end as int - start as int,
            !r.0 ==> r.1 == original,
{
    let count = end - start;
    let mut len = original;
    let mut visited = 0usize;
    // The product uses Range<usize>; its iteration count is end-start.
    while visited < count
        invariant visited <= count, len as int == original as int + visited as int,
                  len <= cap, original as int + count as int <= cap as int,
        decreases count - visited,
    {
        assert(len < cap);
        if visited == fail_at { return (false, original); }
        WITHIN_ADVANCE
        visited += 1;
    }
    (true, len)
}

// Callback outcomes are modeled independently: returning an error, returning
// Ok without writing, and returning Ok after a write. Source-linked Slot proofs
// establish that only the third outcome lets finish() transfer the value.
fn initialized_resize_prefix(original: usize, target: usize, cap: usize,
                             fail_at: usize, callback_ok: bool, slot_written: bool)
    -> (r: (bool, usize))
    requires original < target <= cap,
    ensures r.0 ==> r.1 == target, !r.0 ==> r.1 == original,
{
    let mut len = original;
    while INIT_RESIZE_GUARD
        invariant original <= len <= target <= cap,
        decreases target - len,
    {
        assert(original <= len < target && len < cap);
        if len == fail_at && (!callback_ok || !slot_written) { return (false, original); }
        INIT_RESIZE_ADVANCE
    }
    (true, len)
}

fn initialized_resize_operation(len: usize, cap: usize, limit: usize, target: usize,
                               storage_accepts: bool, fail_at: usize,
                               callback_ok: bool, slot_written: bool) -> (r: (bool, usize))
    requires len <= cap <= limit,
    ensures r.1 <= limit, r.0 ==> r.1 == target, !r.0 ==> r.1 == len,
{
    if INIT_RESIZE_SHRINK { return (true, target); }
    let additional = INIT_RESIZE_ADDITIONAL;
    let Some(needed) = capacity_reservation(len, cap, limit, additional) else {
        return (false, len);
    };
    assert(needed == target);
    if needed > cap && !storage_accepts { return (false, len); }
    initialized_resize_prefix(len, target, cap.max(needed), fail_at, callback_ok, slot_written)
}

fn clone_within_operation(len: usize, cap: usize, limit: usize, start: usize, end: usize,
                          storage_accepts: bool, fail_at: usize) -> (r: (bool, usize))
    requires len <= cap <= limit,
    ensures r.1 <= limit, !r.0 ==> r.1 == len,
            r.0 ==> start <= end <= len && r.1 as int == len as int + end as int - start as int,
{
    if !normalized_range(start, end, len) { return (false, len); }
    let count = end - start;
    let Some(needed) = capacity_reservation(len, cap, limit, count) else {
        return (false, len);
    };
    if needed > cap && !storage_accepts { return (false, len); }
    clone_within_prefix(len, cap.max(needed), start, end, fail_at)
}

fn utf8_byte_cursor(mut position: usize, mut len: usize, encoded_len: usize)
    -> (r: (usize, usize, usize))
    requires position <= len <= 4, 1 <= encoded_len <= 4,
    ensures r.0 <= r.1 <= 4, r.2 == r.1 - r.0,
{
    if position == len { len = encoded_len; position = 0; }
    // The product's bytes.get(position)? also enforces position < 4.
    assert(position < len);
    UTF8_ADVANCE
    (position, len, UTF8_REMAINING)
}

fn splice_move_tail(owner_len: usize, tail_start: usize, tail_len: usize,
                    additional: usize, limit: usize) -> (r: Option<usize>)
    requires owner_len == tail_start,
             owner_len as int + tail_len as int <= limit as int,
    ensures match r {
        Some(start) => start as int == tail_start as int + additional as int
                        && start as int + tail_len as int <= limit as int,
        None => owner_len as int + tail_len as int + additional as int > limit as int,
    },
{
    let Some(required) = tail_len.checked_add(additional) else { return None; };
    let Some(needed) = owner_len.checked_add(required) else { return None; };
    if needed > limit { return None; }
    let destination = SPLICE_TAIL_DESTINATION;
    let mut tail_start = tail_start;
    SPLICE_TAIL_ADVANCE
    assert(destination == tail_start);
    Some(destination)
}

fn splice_spare(owner_len: usize, tail_len: usize, cap: usize, limit: usize)
    -> (r: (usize, usize))
    requires owner_len as int + tail_len as int <= cap as int, cap <= limit,
    ensures r.0 as int == limit as int - owner_len as int - tail_len as int,
            r.1 as int == cap as int - owner_len as int - tail_len as int,
{
    (SPLICE_AVAILABLE, SPLICE_SPARE)
}

fn splice_first_reservation(owner_len: usize, tail_len: usize, limit: usize) -> (n: usize)
    requires (owner_len as int + tail_len as int) < limit as int,
    ensures n as int == tail_len as int + 1,
            owner_len as int + n as int <= limit as int,
{ SPLICE_FIRST_RESERVATION }

fn splice_first_write(mut owner_len: usize, tail_start: usize, tail_len: usize, cap: usize)
    -> (len: usize)
    requires owner_len < tail_start, tail_start as int + tail_len as int <= cap as int,
    ensures len <= tail_start, len as int + tail_len as int <= cap as int,
{
    SPLICE_ADVANCE_ONE
    owner_len
}

fn staged_growth_count(owner_len: usize, tail_len: usize, extra_len: usize, limit: usize)
    -> (r: Option<usize>)
    requires owner_len <= limit,
    ensures match r {
        Some(count) => count as int == tail_len as int + extra_len as int
                      && owner_len as int + count as int <= limit as int,
        None => owner_len as int + tail_len as int + extra_len as int > limit as int,
    },
{
    let Some(total) = owner_len.checked_add(tail_len) else { return None; };
    if total > limit { return None; }
    let Some(needed) = total.checked_add(extra_len) else { return None; };
    if needed > limit { return None; }
    Some(STAGED_GROWTH_COUNT)
}

fn splice_fill_progress(start: usize, tail_start: usize, cap: usize, stop_at: usize)
    -> (r: (bool, usize))
    requires start <= tail_start <= cap,
    ensures start <= r.1 <= tail_start, r.0 ==> r.1 == tail_start,
{
    let mut state=crate::Checked::new(start);
    let result=crate::init::driver::run(&mut state,tail_start,
        |index:usize|
            requires start<=index<tail_start,
        {
            if index-start==stop_at {Err(())} else {Ok(())}
        },
    );
    (result.is_ok(),state.get())
}

fn collect_excess(available: usize, requested: u128) -> (r: (bool, usize))
    ensures r.1 <= available,
            r.0 ==> r.1 as int == requested as int,
            !r.0 ==> requested as int > available as int && r.1 == available,
{
    let mut extra_len = 0usize;
    let mut remaining = requested;
    while remaining > 0
        invariant extra_len <= available,
                  extra_len as int + remaining as int == requested as int,
        decreases remaining,
    {
        // try_reserve(1) rejects before the product writes at its fixed limit.
        if extra_len == available { return (false, extra_len); }
        EXCESS_ADVANCE
        remaining -= 1;
    }
    (true, extra_len)
}

fn staged_splice(owner_len: usize, tail_start: usize, tail_len: usize, extra_len: usize, cap: usize)
    -> (r: (usize, usize))
    requires owner_len == tail_start,
             owner_len as int + tail_len as int + extra_len as int <= cap as int,
    ensures r.0 == r.1, r.1 as int + tail_len as int <= cap as int,
{
    let destination = STAGED_TAIL_DESTINATION;
    let mut tail_start = tail_start;
    let mut owner_len = owner_len;
    STAGED_TAIL_ADVANCE
    STAGED_OWNER_ADVANCE
    assert(destination == tail_start);
    (tail_start, owner_len)
}
}
fn main() {}
