use super::*;

struct Probe {
    calls: u8,
    reply: Result<Option<u8>, u8>,
    low: usize,
    high: Option<usize>,
}
impl TryIterator for Probe {
    type Item = u8;
    type Error = u8;
    fn try_next(&mut self) -> Result<Option<u8>, u8> {
        self.calls = self.calls.wrapping_add(1);
        self.reply
    }
    fn try_size_hint(&self) -> Result<(usize, Option<usize>), u8> {
        Ok((self.low, self.high))
    }
}

#[kani::proof]
#[kani::solver(cadical)]
fn take_preserves_limit_and_propagates_every_reply() {
    let remaining: usize = kani::any();
    let reply: Result<Option<u8>, u8> = kani::any();
    let mut limited = Probe {
        calls: 0,
        reply,
        low: 0,
        high: None,
    }
    .try_take(remaining);
    let actual = limited.try_next();
    if remaining == 0 {
        assert_eq!(actual, Ok(None));
        assert_eq!(limited.iter.calls, 0);
        assert_eq!(limited.remaining.get(), 0);
    } else {
        assert_eq!(actual, reply);
        assert_eq!(limited.iter.calls, 1);
        assert_eq!(limited.remaining.get(), remaining.checked_sub(1).unwrap());
    }
}

#[kani::proof]
#[kani::solver(cadical)]
fn take_clamps_even_dishonest_hints_without_overflow() {
    let remaining: usize = kani::any();
    let low: usize = kani::any();
    let high: Option<usize> = kani::any();
    let limited = Probe {
        calls: 0,
        reply: Ok(None),
        low,
        high,
    }
    .try_take(remaining);
    let (actual_low, actual_high) = limited.try_size_hint().unwrap();
    assert!(actual_low <= remaining);
    assert!(actual_high.unwrap() <= remaining);
    if high.is_none() || high.unwrap() >= low {
        assert!(actual_high.unwrap() >= actual_low);
    }
}

#[kani::proof]
#[kani::unwind(3)]
#[kani::solver(cadical)]
fn nth_stops_at_first_error_or_end_for_any_skip_count() {
    let count: usize = kani::any();
    let failed: bool = kani::any();
    let code: u8 = kani::any();
    let reply = if failed { Err(code) } else { Ok(None) };
    let mut probe = Probe {
        calls: 0,
        reply,
        low: 0,
        high: None,
    };
    assert_eq!(probe.try_nth(count), reply);
    assert_eq!(probe.calls, 1);
}

#[kani::proof]
#[kani::unwind(3)]
#[kani::solver(cadical)]
fn fold_distinguishes_source_and_operation_failure() {
    let failed: bool = kani::any();
    let code: u8 = kani::any();
    let item: u8 = kani::any();
    let reply = if failed { Err(code) } else { Ok(Some(item)) };
    let mut probe = Probe {
        calls: 0,
        reply,
        low: 0,
        high: None,
    };
    let mut invoked = false;
    let result = probe.try_fold((), |(), x| {
        invoked = true;
        Err::<(), _>(x)
    });
    assert_eq!(probe.calls, 1);
    assert_eq!(invoked, !failed);
    if failed {
        assert_eq!(result, Err(TryFoldError::Source(code)));
    } else {
        assert_eq!(result, Err(TryFoldError::Operation(item)));
    }
}
