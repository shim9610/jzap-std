# Fallible allocation-policy duplication

Status: policy ownership is implemented and verified after BTreeMap checkpoint
`546a19b`, including the iterator optimization at `c4c16e2`.
This change prepares policy ownership; an owning budget and new reallocation
accounting modes are separate steps in the agreed implementation order.

## Contract

Rust's `Copy` is an implicit bitwise-copy marker with no error-returning method.
Allocation policies instead use the existing `TryClone` interface. Successful
duplication creates one policy owner for the same accounting domain; it does not
duplicate an allocation charge or copy the budget ledger. A refused duplication
preserves its source owner and accounting ledger. Container operations retain
their documented rollback or streaming-prefix guarantees. Current borrowed policies and the
zero-sized Unmetered policy duplicate without failure or allocation.

Policy accessors return a shared borrow. References to policies implement the
scoped borrowed-policy contract, keeping the original policy alive without
acquiring another owner. Consuming owner/raw conversions move
the policy exactly once. Construction still consumes its policy argument and
returns rejected payloads according to each API's existing contract. Callers
that need to retain a policy explicitly duplicate it before transferring it.

New container owners, split outputs and replacement allocations acquire their
required policy owners before changing existing data. An error must leave the
original container and rejected data intact. Destruction, node movement and
deallocation borrow an existing policy and cannot introduce a fallible copy.
Append reserves all required policy owners before its commit point. Streaming
splice keeps its already committed replacement prefix and restores the old tail
when staging-policy duplication is refused, matching its storage-error contract.

Shared control blocks own their policy once, independently of strong/weak handle
copies. The header stores it in `ManuallyDrop`; the final weak release moves it
out, deallocates storage, refunds the charge, then drops the policy. Failed
initialization follows the same order. An unfinished bulk pillar borrows its
policy instead of duplicating it after append has started committing.

## References and validation

The implementation references Rust 1.95.0 (`59807616e`) `core::marker::Copy`,
`alloc::boxed::Box::allocator` / `into_raw_with_allocator`, and
`alloc::vec::Vec::allocator` / `split_off`. Standard allocator access borrows;
raw ownership transfer moves its allocator. Allocator-aware split requires
`Clone`, whose implicit failure contract is replaced here by `TryClone`.
These references add no runtime dependency or unstable API requirement.

Validation must exercise a non-Copy policy with a fallible owner counter and
observable Drop behavior. Check every policy-duplication refusal, preservation
of source/destination/payloads, transfer without duplication, initializer and
destructor cleanup, and exact refunds before the last policy owner disappears.
Run Miri, source/guard checks, compiler boundaries and existing functionality
suites after updating policy ownership. Compare generated code and repeated
std timings for the existing policies. Renewed verification and repeated timings are recorded in the
[verification report](verification/policy-duplication.md).

## Operation boundaries

| Operation | Policy ownership | Refusal behavior |
| --- | --- | --- |
| Policy accessor | Returns `&P` | No acquisition or error |
| Raw parts, Box/Vec transfer, consuming iterator | Moves `P` once | Existing layout/ownership contract applies |
| Clone, split output, replacement storage | Calls `TryClone` before publishing the new owner | Existing data and charges retained |
| Nonempty BTreeMap append | Acquires every required owner before budget or tree commit | Both input maps retained |
| Streaming splice staging | Acquires a staging owner when needed | Committed prefix retained; old tail restored |
| Shared handle clone/downgrade/upgrade | Changes the shared control-block count | No additional policy owner per handle |
| Final shared weak release | Takes the single header policy | Frees storage and refunds before policy destruction |
| Deliberate Box/Vec leak | Forgets the policy with its live charge | Domain ownership is deliberately leaked too |

This replaces the allocation-policy bound; it does not redefine Rust's `Copy`
marker or make ordinary scalar copies fail. Built-in borrowed policies remain
`Copy`, while their explicit duplication interface is `TryClone`. No blanket
delegation to an arbitrary user's `Clone` is introduced.

The private non-Copy test policy tracks policy owners, duplication attempts and
charges. Fourteen scenarios cover every append acquisition ordinal in shared and distinct
accounting domains,
refused clones and splits, consuming and standard-owner transfers, shared COW,
last-Weak release, failed/missing initialization, budget refusal, streaming
splice, early iterator destruction, deliberate leaks with unsafe recovery, and
one payload-destructor unwind. Its final owner asserts that both occupied and
reserved bytes have already been returned. These scenarios pass native tests,
x86_64/i686 Miri and strict Tree Borrows. They exercise memory paths; the
separate source-connected certificate supplies numeric and guard obligations.
