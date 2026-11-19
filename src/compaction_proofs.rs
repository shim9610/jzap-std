use super::*;

fn invariant(s: &Compaction) -> bool {
    s.write <= s.read && s.read <= s.original
}
#[kani::proof]
#[kani::solver(z3)]
fn constructors_establish_every_numeric_bound() {
    let start: usize = kani::any();
    let original: usize = kani::any();
    assert!(invariant(&Compaction::full(original)));
    assert!(invariant(&Compaction::first_kept(original)));
    match Compaction::try_prefix(start, original) {
        Ok(s) => {
            assert!(invariant(&s));
            assert!(s.read() == start && s.written() == start);
        }
        Err(_) => assert!(start > original),
    }
}
#[kani::proof]
#[kani::solver(z3)]
fn both_transitions_close_all_invariant_states_and_queries() {
    let read: usize = kani::any();
    let write: usize = kani::any();
    let original: usize = kani::any();
    kani::assume(write <= read && read <= original);
    let state = Compaction {
        read: Checked::new(read),
        write: Checked::new(write),
        original: Checked::new(original),
    };
    assert!(state.remaining() == original - read);
    if write > 0 {
        assert!(state.previous_kept() == write - 1);
    }
    assert!(state.restored_len() == original - (read - write));
    let (mut keep, mut remove) = (state, state);
    let (k, r) = (keep.keep_next(), remove.remove_next());
    assert!(invariant(&keep) && invariant(&remove));
    if read == original {
        assert!(!k && r.is_none());
    } else {
        assert!(k && r == Some(read));
        assert!(keep.read() == read + 1 && keep.written() == write + 1);
        assert!(remove.read() == read + 1 && remove.written() == write);
        assert!(keep.restored_len() == state.restored_len());
        assert!(remove.restored_len() == state.restored_len() - 1);
        let (mut live_keep, mut live_remove) = (state, state);
        live_keep.keep_live();
        live_remove.remove_live();
        assert!(live_keep.read() == keep.read() && live_keep.written() == keep.written());
        assert!(live_remove.read() == remove.read() && live_remove.written() == remove.written());
    }
}

#[kani::proof]
#[kani::solver(z3)]
fn constructors_and_all_transitions_preserve_range_and_cleanup() {
    let start: usize = kani::any();
    let end: usize = kani::any();
    let total: usize = kani::any();
    match Extraction::try_range(start, end, total) {
        Ok(s) => {
            assert!(s.written() <= s.read() && s.read() <= s.end() && s.end() <= s.total())
        }
        Err(_) => assert!(start > end || end > total),
    }
    let write: usize = kani::any();
    let read: usize = kani::any();
    let Ok(state) = Extraction::from_cursor_state(write, read, end, total) else {
        return;
    };
    assert!(state.restored_len() == total - (read - write));
    assert!(state.remaining() == end - read);
    assert!(state.unvisited_len() == total - read);
    let mut kept = Extraction::from_cursor_state(write, read, end, total).unwrap();
    let mut removed = Extraction::from_cursor_state(write, read, end, total).unwrap();
    let old = state.restored_len();
    let (k, r) = (kept.keep_next(), removed.remove_next());
    if read < end {
        assert!(k && r == Some(read));
        assert!(kept.restored_len() == old && removed.restored_len() == old - 1);
    } else {
        assert!(!k && r.is_none());
    }
    assert!(
        kept.written() <= kept.read() && kept.read() <= kept.end() && kept.end() <= kept.total()
    );
    assert!(
        removed.written() <= removed.read()
            && removed.read() <= removed.end()
            && removed.end() <= removed.total()
    );
    let kept_before = kept.restored_len();
    let removed_before = removed.restored_len();
    kept.stop();
    removed.stop();
    assert!(kept.remaining() == 0 && kept.restored_len() == kept_before);
    assert!(removed.remaining() == 0 && removed.restored_len() == removed_before);
}
