# Vec implementation contract

Fallible input APIs use the open `TryIntoIterator`/`TryIterator` contracts.
See [fallible iteration](iteration.md) for implementation guidance, standard
interoperability, error propagation, and consumer link checks.

Scope: Rust 1.95 stable Vec inherent functionality and explicit adapters for its
related trait families, as enumerated in [the API mapping](vec-api.md). Verification
and measurement status is recorded separately in the [report](verification/vec/README.md).

## Storage and limits

`Vec<T, P, L>` owns a pointer, initialized length, capacity, allocation policy,
and sealed length-limit policy. The default unmetered/unlimited policies occupy
no storage. A selected runtime element limit adds one usize; a budget policy adds
its reference. These costs must be measured separately from payload allocation.
No external runtime dependency is introduced.

`Vec::with_limit(max_len)` creates an empty vector without allocating. The limit
counts elements, including zero-sized types, and is separate from byte budgets
and current capacity. Nonzero-size storage satisfies `len <= capacity <= max_len`.
Zero-sized storage uses the limit as its logical capacity without allocating.
Default construction uses no artificial element limit.

Growth checks length overflow and the configured limit before allocation. Its
geometric proposal is clamped to the limit and representable layout. Hitting the
limit exactly is valid. Pop, removal, truncation, and clearing permit later reuse.
A limit is fixed for an owner's lifetime. Clone and split preserve the source
limit; replacement cloning preserves the destination limit and policy.

## Failure and ownership

| Operation | Returned-error contract |
| --- | --- |
| Reserve, exact reserve, shrink | Preserve pointer, contents, length, capacity, and budget on refusal |
| Push, insert | Return the rejected value; preserve the original vector |
| Copy extension | Check/reserve once, then bulk copy; no change on refusal |
| Clone extension / resize initialization | Preserve original contents and length; drop completed suffix; successful reservation may retain larger capacity |
| Iterator extension | Same suffix rollback; consumed input and external effects cannot be restored; document any consumed rejected item |
| Append between vectors | Reserve first; refusal preserves both owners; success retains the source's empty allocation |
| Clone replacement (Vec and owned iterator) | Retain destination limit/policy; stage a replacement before commit unless a verified no-allocation Copy hook applies; error preserves destination |
| Standard/Box owner adoption | Check limit, layout, and budget before transfer; rejection returns the original owner |
| Drain | Bounds refusal changes nothing; iterator Drop completes removal without allocation; retained tail restored on unwind |
| Splice | Explicit `finish` reports failure; retains committed replacement prefix and original tail; Drop alone removes without replacement |
| Retain/dedup/extract | Error retains the current and unvisited suffix after prior removals; callback mutations persist; no allocation |
| Internal range cloning | Bounds/reservation checked first; failed clone rolls back the appended suffix |
| Owned resize | Failed storage/clone returns input and cleans appended clones; capacity can remain larger |
| Formatting | Sticky storage error and suffix rollback even if formatter ignores a failed write |
| Flatten / fixed array conversion | Refusal returns original owner; numeric limit and byte charge retained on flatten |
| Spare storage | Checked new length; caller proves newly exposed elements initialized under an unsafe contract |

Capacity errors, layout errors, allocator refusal, and budget refusal remain
separate error categories. Reallocation reserves the entire new request while
the old charge is live. Partial cleanup and final deallocation also run on unwind;
user panics are not converted into Result errors. No implicit allocating Clone,
FromIterator, Extend, or slice-indexing Deref is exposed.

## Reference basis

The implementation references inspected were Rust 1.95 and nightly-2026-10-30
alloc Vec/RawVec and Varlit fallible cloning at
`cd7b0446da3e731fba867802bef5f4b7b3fc328d`. Reuse jzap's checked layout, allocation,
budget, and recorded-initialization boundaries. Reference wrappers do not supply
the requested runtime limit or jzap budget/ownership contracts by themselves.

## Verification and performance protocol

Exercise every allocation/refusal position, limit zero/exact/overflow, geometric
overshoot, zero-sized and over-aligned elements, partial clone/initialization,
destructor unwinding, nested budget propagation, owner conversion, and reuse after
failure. Add std-model state transitions, compile-fail boundaries, Miri in both
borrow models, sanitizer checks, source-linked bounded proofs, fuzzing, and mutation
controls. Record unsupported tools and remaining scope explicitly.

Direct std timing uses separate instrument-free processes, alternating order,
two warmups, and 30 retained cycles per process, with 15 independent processes per
condition initially. Include std controls, unlimited, limited, and metered forms;
measure repeated small and large growth, pre-reserved push, copy, clone, range
editing, and internal extension paths. Splice covers exact, filtered, and absent
length hints.
Use an upper bootstrap 95% ratio bound of 1.15 as the timing criterion; retain
inconclusive samples and diagnose before reporting a performance shortfall.
Requested and allocator-usable live/peak/final bytes, allocation counts, and handle
sizes are measured separately. Unlimited/unmetered capacity schedules should match
std for the selected growth workloads. Report the limit/budget handle words
explicitly; the earlier Box-only size acceptance is not a blanket Vec exemption.

## Reference and implementation review

New range-edit algorithms were reviewed against Rust 1.95 `alloc/src/vec/mod.rs`,
`drain.rs`, `extract_if.rs`, and `splice.rs`, under Rust's MIT OR Apache-2.0 license.
They use the same initialized-prefix/gap/retained-tail ownership model with
jzap-specific fallible callbacks, explicit splice completion, budget checks,
and fixed element limits. No external runtime dependency is added. The maintained
`check_vec_api.py` records the compiler identity and source hash in local output.
Unstable declarations are inventoried separately; a docs.rs nightly build is not
used as evidence of a crate's stable compatibility.

Unhinted splice replacement first fills the drained hole, then spare capacity
from one checked reservation justified by an actual item. If the input exceeds
that gap, only the remainder is staged. If an earlier reservation already grew
the owner, final growth uses the required size instead of doubling it again.
The retained tail is guarded during reservation, iterator callbacks, staging,
and final movement. A returned error keeps the committed prefix and retained
tail; it does not restore removed or consumed replacement values.
