//! Actual UTF-8 owners: callback/error cleanup and drain ownership.
use super::*;

fn two_scalars(first: char, second: char) -> Option<String> {
    let Ok(mut owner) = String::try_with_capacity(8) else {
        return None;
    };
    assert!(owner.try_push(first).is_ok());
    assert!(owner.try_push(second).is_ok());
    Some(owner)
}

fn snapshot(owner: &String) -> [u8; 8] {
    let mut bytes = [0u8; 8];
    for i in 0..8 {
        if i < owner.len() {
            bytes[i] = owner.as_bytes()[i];
        }
    }
    bytes
}

fn remaining(
    owner: &String,
    source: &[u8; 8],
    first_width: usize,
    second_width: usize,
    keep_first: bool,
    keep_second: bool,
) {
    let offset = if keep_first { first_width } else { 0 };
    assert!(owner.len() == offset + if keep_second { second_width } else { 0 });
    for i in 0..4 {
        if keep_first && i < first_width {
            assert!(owner.as_bytes()[i] == source[i]);
        }
        if keep_second && i < second_width {
            assert!(owner.as_bytes()[offset + i] == source[first_width + i]);
        }
    }
}

fn retain_case(first: char, second: char) {
    let replies: [u8; 2] = kani::any();
    kani::assume(replies[0] <= 2 && replies[1] <= 2);
    let Some(mut owner) = two_scalars(first, second) else {
        return;
    };
    let source = snapshot(&owner);
    let mut calls = 0usize;
    let reply: Result<(), u8> = owner.try_retain(|ch| {
        assert!(calls < 2);
        assert!(ch == if calls == 0 { first } else { second });
        let next = replies[calls];
        calls += 1;
        if next == 2 { Err(7) } else { Ok(next == 1) }
    });
    assert!(calls == if replies[0] == 2 { 1 } else { 2 });
    assert!(reply.is_err() == (replies[0] == 2 || replies[1] == 2));
    remaining(
        &owner,
        &source,
        first.len_utf8(),
        second.len_utf8(),
        replies[0] != 0,
        replies[0] == 2 || replies[1] != 0,
    );
}

fn drain_case(first: char, second: char) {
    let start: usize = kani::any();
    let end: usize = kani::any();
    let consume_front: bool = kani::any();
    let consume_back: bool = kani::any();
    kani::assume(start <= end && end <= 2);
    let Some(mut owner) = two_scalars(first, second) else {
        return;
    };
    let source = snapshot(&owner);
    let positions = [0, first.len_utf8(), owner.len()];
    {
        let mut drain = owner.try_drain(positions[start]..positions[end]).unwrap();
        if consume_front {
            let _ = drain.next();
        }
        if consume_back {
            let _ = drain.next_back();
        }
    }
    remaining(
        &owner,
        &source,
        first.len_utf8(),
        second.len_utf8(),
        start != 0 || end == 0,
        end < 2 || start == 2,
    );
}

// As in the existing String heap witnesses, isolate scalar width from heap
// branch expansion. Unrestricted Unicode encoding and word geometry retain
// their separate proofs; these actual-owner probes cover every scalar width.
#[kani::proof]
#[kani::unwind(10)]
fn retain_cursors_and_error_cleanup_preserve_scalar_contents() {
    retain_case('a', '😀');
    retain_case('é', '한');
    retain_case('한', 'é');
    retain_case('😀', 'a');
}

#[kani::proof]
#[kani::unwind(10)]
fn drain_cleanup_removes_only_the_validated_scalar_range() {
    drain_case('a', '😀');
    drain_case('é', '한');
    drain_case('한', 'é');
    drain_case('😀', 'a');
}
