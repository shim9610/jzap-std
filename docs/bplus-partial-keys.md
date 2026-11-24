# B+ point lookup with partial keys

Status: implemented for typed node slices; verification below. Tracks
part 2 of [issue #10](https://github.com/shim9610/jzap-std/issues/10).

## Goal

Search caller-owned B+ nodes whose keys may be stored only partly, for
example as an inline prefix and a length, with the rest of each key stored
elsewhere. jzap never reads storage. When a comparison needed by the lookup
cannot be decided from what the caller supplied, the lookup returns a request,
and the caller resolves the key and resumes.

The existing complete-key API (`bplus::Node`, `bplus::Lookup`, the strided and
fixed views) keeps its behaviour.

## Correctness requirement

Every answer is exact. A partial lookup returns one of:

- a request (`PartialStep::NeedKey` or `PartialStep::NeedFence`), or
- exactly the result, `Ok` step or `Err`, that the complete-key
  `bplus::Lookup` returns for the same tree with every key replaced by its
  completion.

This includes damage. A node whose completed keys are out of order, a child
whose completed first key differs from its separator, and a resolved key
that contradicts its inline part are refused with `Error::InvalidStructure`.
A damaged node is never answered with `Absent`.

The guarantee holds under two conditions:

1. The key type satisfies the `PartialKey` laws below. jzap provides
   `PrefixKey` and proves the laws for it. A caller-defined key type is
   trusted to satisfy them.
2. Each full key the caller attaches or passes is the stored key's
   completion. jzap checks that it agrees with the inline part (prefix and
   length for `PrefixKey`). Bytes that agree with the inline part but are not
   the stored key cannot be detected, in the same way that the complete-key
   lookup answers for the key bytes it is given.

### Why every adjacent pair is validated

The lookup checks the order of every adjacent key pair in every visited
node, as `bplus::Node` does. Checking only the pairs on the binary-search
path is cheaper in requests, but it is not exact. A damaged leaf
`[1, 5, 3, 7]` searched for `3` compares only `5` and `1`, then answers
`Absent`, while the complete-key lookup refuses the leaf. Full validation
removes that case. Its cost is that an undecidable adjacent pair can require
resolution even when the pair is off the search path.

## Key model

```rust
pub trait PartialKey {
    /// The complete key, e.g. `[u8]`.
    type Full: ?Sized + Ord;
    /// A self-contained copy kept as a fence after the node buffer is reused.
    type Fence: Copy;

    /// Order two stored keys. `None`: the stored parts cannot decide.
    fn cmp_stored(&self, other: &Self) -> Option<Ordering>;
    /// Order this stored key against a complete key.
    fn cmp_full(&self, full: &Self::Full) -> Option<Ordering>;
    /// The complete key, when it is inline or attached by the caller.
    fn full(&self) -> Option<&Self::Full>;

    fn fence(&self) -> Self::Fence;
    /// Order this stored key against a fence.
    fn cmp_fence(&self, fence: &Self::Fence) -> Option<Ordering>;
    /// Order a fence against a complete key.
    fn fence_cmp_full(fence: &Self::Fence, full: &Self::Full) -> Option<Ordering>;
    /// Whether `full` is a possible completion of the fence.
    fn fence_admits(fence: &Self::Fence, full: &Self::Full) -> bool;
}
```

Laws, where a completion of a stored key or fence is the complete key it
represents:

- **Consistency.** A `Some(o)` from any comparison holds for every
  completion of both sides.
- **Resolved keys.** `full()` returns the key's completion.
- **Fences.** `fence()` represents the same completions as the key.
  `fence_admits(fence, full)` is true exactly when `full` is a completion of
  the fence.
- **Determinism.** Every method returns the same result for the same
  arguments.

### `PrefixKey`

```rust
pub struct PrefixKey<'a, const P: usize> {
    prefix: [u8; P],
    len: usize,
    full: Option<&'a [u8]>,
}
```

- `PrefixKey::new(prefix, len)`: the first `min(len, P)` bytes are the key's
  start. When `len <= P`, the key is complete inline.
- `PrefixKey::from_full(bytes)`: copies the prefix and attaches `bytes`.
- `attach(&mut self, full)`: attaches a resolved key. It returns
  `Error::InvalidStructure` unless `full.len() == len` and `full` starts with
  the stored prefix.
- `Fence = PrefixFence<P>`: the prefix and the length, without the borrow.

Stored comparison follows byte order. It decides when the inline bytes
differ, when both keys are complete, or when a complete key is a proper
prefix of the other key's inline bytes. It returns `None` only when both
keys are longer than `P` and their prefixes are equal. A complete key that
equals the other's `P`-byte prefix is shorter and orders first.

## Nodes and steps

`PartialNode::leaf(keys, values)` and `PartialNode::branch(keys, children)`
borrow typed slices. Construction checks only the local shape, which needs
no comparisons: equal key and payload counts, and a non-empty branch. Key
order is checked by the lookup, because it may need resolution.

Partial lookups return their own step type:

```rust
pub enum PartialStep<'a, K, V, I> {
    NeedNode(I),
    /// Attach the full key at this index of the current node, then provide it again.
    NeedKey(usize),
    /// Provide the current node again with the full keys of the fences
    /// that `fence_origins` reports.
    NeedFence,
    Found(&'a K, &'a V),
    Absent,
}
```

`LookupStep`, which the complete-key lookups return, is unchanged. Adding
the two variants to it was measured first and changed the complete-key
lookup code. On the `bplus` benchmark executable (Rust 1.95.0, seed 1,
36,864,000 lookups per method, Callgrind, same checksums):

| `LookupStep` change | `jzap` | `jzap_validate` | `jzap_u32` |
| --- | ---: | ---: | ---: |
| `main` (instructions per lookup) | 483.62 | 860.90 | 497.45 |
| `NeedKey(usize)`, `NeedFence { minimum: bool, ceiling: bool }` before `Found` (ratio to `main`) | 1.0534 | 1.0733 | 0.9862 |
| `NeedKey(usize)`, unit `NeedFence` after `Absent` (ratio to `main`) | 1.0000 | 1.0000 | 1.0369 |

The `bool` payloads sit next to the tag, so the tag stores shrink from eight
bytes to one (`movq` to `movb`) and register allocation in the validated
lookup changes. With the variants last and no payload beside the tag, the
`usize` lookups compile to the same instructions as `main`, but the `u32`
lookup executes about 19 more register moves per lookup. With the separate
`PartialStep`, every function of the `bplus`, `bplus_strided` and
`bplus_range` benchmark executables compiles to the same instructions as
`main`, apart from RIP-relative displacements of linked std functions.

## Continuation

```rust
impl<'q, F: ?Sized + Ord, Fence: Copy, I: Copy + Eq> PartialLookup<'q, F, Fence, I> {
    pub fn new(root: I, height: usize, query: &'q F) -> Result<Self, Error>;
    pub fn requested_node(&self) -> Option<I>;
    /// The node and separator index each fence was copied from.
    pub fn fence_origins(&self) -> (Option<(I, usize)>, Option<(I, usize)>);

    pub fn provide<'a, K, V>(&mut self, id: I, node: &PartialNode<'a, K, V, I>)
        -> Result<PartialStep<'a, K, V, I>, Error>
    where K: PartialKey<Full = F, Fence = Fence>;

    /// `provide` with the full keys of the current fences.
    pub fn provide_fenced<'a, K, V>(
        &mut self,
        id: I,
        node: &PartialNode<'a, K, V, I>,
        minimum: Option<&F>,
        ceiling: Option<&F>,
    ) -> Result<PartialStep<'a, K, V, I>, Error>
    where K: PartialKey<Full = F, Fence = Fence>;
}
```

The lookup is generic over the full and fence types instead of the key type.
Each node can therefore use keys that borrow different storage, and the
caller can reuse its key and page buffers between nodes.

### One pass per call

Each `provide` or `provide_fenced` call runs one complete pass over the
supplied node:

1. Check the expected identifier, the ancestor path and the node kind for
   the remaining height.
2. Check every supplied fence full key with `fence_admits`. A mismatch is
   damage.
3. Compare the first key with the minimum fence (equal) and the last key
   with the ceiling (below).
4. Compare every adjacent key pair (strictly increasing).
5. Search the node as `bplus::Lookup` does: the last separator at or below
   the query in a branch, an equal key in a leaf.

A comparison is decided from the stored parts first, then from an attached
or passed full key. If a comparison needs a key that is not resolved, the
pass returns `NeedKey(index)` for the first such key in the order above (the
minimum fence, the adjacent pairs from the left, the ceiling, then the
search) and does not advance. Of an undecided pair of unresolved keys, the
left key is requested first. If only fence comparisons remain undecided, it
returns `NeedFence`. Otherwise it returns the same step as the complete-key
lookup and advances.

The lookup keeps no resolution state between calls. The caller attaches the
requested key to its node buffer, for example with `PrefixKey::attach`, and
provides the node again. A pass therefore never depends on a previous call
having seen the same node contents, and every answer comes from one pass
whose comparisons were all decided.

Resolution state could instead be retained inside the lookup, with a
`provide_key(index, full)` call as issue #10 sketched. That form is not
used. An undecided pair of incomplete keys needs both full keys at once. If
the lookup forgot one full key when receiving the other, it would request
them alternately forever. If it retained them, it would need either a
caller-chosen lifetime for all resolved keys or an allocation, and a later
call could pair them with different node contents.

### Fences

A fence is copied with `PartialKey::fence` when the lookup descends. It
records the parent identifier and separator index, available through
`fence_origins`. Fence full keys are borrowed only for the call:

- **On request.** `NeedFence` asks for the fences. The caller passes the
  full key of each fence that `fence_origins` reports to `provide_fenced`
  with the same node. A fence that is already decided may be passed too.
- **In advance.** A caller that already holds the separators' full keys can
  pass them to `provide_fenced` on the first call.

Both fences are requested in one step and passed in one call, so the pass
can decide them together.

### Requests

With a caller that keeps every attachment, each node needs at most:

- one `NeedKey` per stored key that is incomplete when the node is first
  supplied, because a resolved key is never requested again; and
- one `NeedFence`.

Each pass compares every adjacent pair, so a node that needs `r` key
requests costs `r + 1` passes of `O(n)` comparisons. Attaching the keys that
share a prefix with a neighbour before the first call, as varlit's parser
already does, needs no request.

### Listing the keys a pass could request

`PartialLookup::unresolved_keys(&node)` lists, in index order, every
unresolved key of a node that a pass over it could request:

- both unresolved keys of each adjacent pair the stored parts cannot decide;
- the first key, when its stored part cannot decide against the minimum
  fence, and the last key against the ceiling;
- each key whose stored part cannot decide against the query.

Attaching all of them leaves no `NeedKey` for that node: one pass then
answers, refuses or returns `NeedFence`. Whether a key is listed depends on
that key, its neighbours' stored parts, the fences and the query, and not on
whether other keys are attached, so a caller can collect the indexes, fetch
those keys in one round and attach them in any order. The list is computed
without fence keys, which can only list more keys than a pass with fence
keys requests. The lookup still keeps no resolution state; a node supplied
later is validated in full again.

The list can contain keys the pass would not reach, for example keys
undecided against the query outside the search path. It trades those extra
fetches for one round per node.

## Errors and guarantees

- No allocation and no panic paths, on the same terms as the link-audited
  complete-key lookup.
- Requests do not advance the lookup. A rejected node, key or fence leaves
  the same node requested, so the caller can retry.
- `Error::InvalidStructure` reports damage: an order or fence violation, a
  wrong node kind, a cycle, a node out of turn, a fence full key that its
  fence does not admit, or a `PrefixKey` attachment that contradicts the
  stored prefix or length.
- Fetch failures stay with the caller.

## Scope

In scope: `PartialKey`, `PrefixKey`, `PrefixFence`, `PartialNode`,
`PartialLookup` and `PartialStep`.

Out of scope for this checkpoint:

- partial keys inside fixed-stride byte pages (`StridedNode`);
- range traversal with partial keys ([issue #13](https://github.com/shim9610/jzap-std/issues/13));
- key storage, fetching, caching and integrity checks, which belong to the
  caller.

## Verification

Toolchains: Rust 1.95.0, Kani 0.68.0 with CBMC 6.11.0, Miri from
`nightly-2026-10-30`. The certificate stage `bplus-partial` runs the Kani,
Miri and link checks below.

| Check | Scope | Result |
| --- | --- | --- |
| Kani, `PrefixKey` laws | Two arbitrary byte strings of up to 4 bytes, 2 inline bytes: every decided comparison holds for the completions; complete keys always decide; `fence_admits` and `attach` accept exactly the consistent completions | Pass |
| Kani, leaf lookup | Leaves of 1 and 2 keys of up to 2 bytes with 1 inline byte, arbitrary bytes, attachments and query: resolving each request gives the result of `bplus::Lookup` over the completions, and only unresolved keys are requested | Pass |
| Kani, branch step | Two separators, each attachment choice in its own harness: the route, `Absent` or error matches the specification that `branch_routes_to_the_last_admitted_minimum` proves for the complete-key lookup, and the stored fences, origins, depth and path are the state the child step starts from | Pass (4 harnesses) |
| Kani, child step | From that state, a one-key leaf with both fences or the minimum only: with fence keys passed in advance, the result equals the complete-key checks; one call without fence keys either gives that result or requests or refuses with the state unchanged | Pass (4 harnesses) |
| Kani, protocol | Out-of-turn identifiers, fence keys passed without a fence, and requests leave the lookup unchanged | Pass |
| Kani, listed keys | Leaves of 1 and 2 keys, the branch step and the child step with both fences or the minimum only, arbitrary attachments: after attaching every key `unresolved_keys` lists, the pass returns no `NeedKey` (with and without fence keys in the child step) | Pass (5 harnesses) |
| Native exhaustive test | Leaves of 0 to 3 keys over bytes {0, 1, 2, 3}, lengths 0 to 2, 1 inline byte, every attachment choice and query: 1,593,795 cases equal `bplus::Lookup`, and in each case a single pass after attaching the listed keys gives the same answer. Four byte values realize every order of three keys and a query at each position | Pass |
| Property test | 300 random trees (leaf width 2 to 6, branch width 2 to 5) of keys with shared prefixes up to 8 bytes and 4 inline bytes, 40 queries each, against `std::collections::BTreeMap`; half of the trees attach the listed keys of each node first. 19,337 key and 8,424 fence requests and 19,844 listed keys; each node requests at most its incomplete keys, and a node whose listed keys were attached requests none | Pass |
| Damage tests | Off-path order damage (`[1, 5, 3, 7]`, plain and behind equal prefixes), duplicate keys, attachments contradicting length or prefix, a fence key the fence does not admit, a fence key for a missing fence, and a child minimum that differs from its separator after the prefix | Refused with `InvalidStructure` |
| Miri | The integration tests on x86-64 and i686 with strict provenance, and with Tree Borrows | Pass |
| Final link | `tools/consumer-audit/cases/bplus_partial.rs` in the release-checked and release-LTO-checked profiles: no retained panic or exit edge; the overflow, bounds and abort controls are detected | Pass |
| Defect controls | Six injected defects (pair order skipped, minimum not required equal, prefix order reversed, attachment length ignored, undecided treated as equal, fence request ignored) fail the tests; three of them also fail the corresponding Kani harness | Detected |

The module adds no `unsafe` code; the unsafe inventory is unchanged at 608
blocks.

Kani limits and how they were handled:

- A single harness over a leaf of 3 keys, or over a root and two leaves,
  exhausts CBMC memory (about 13 GB). The proof is split into the leaf,
  branch and child steps above, which compose through the lookup state; the
  three-key leaf is covered by the native exhaustive test.
- With the default CaDiCaL solver, the branch harnesses peak near 13 GB and
  one failed for memory when all harnesses ran in one process. With Kissat
  the 12 harnesses run in one process in 56 minutes and peak at 5.5 GB.
- Indexing the key array with the symbolic requested index
  (`keys[requested]`) produced a counterexample that contradicts the same
  path's own assertions: the inline-byte equality held for every index
  below `N`, `requested < N` held, and the same equality failed at
  `keys[requested]`. The harnesses select the requested key with a concrete
  loop instead. This counterexample is not counted as a product defect, and
  the native exhaustive test checks the same property.
- Two listed-key harnesses that consumed the `unresolved_keys` iterator
  failed. Kani's concrete playback of each counterexample, inserted into a
  copy of the source and run natively, passes: the assertion holds for the
  reported inputs. The harnesses now evaluate the iterator's filter
  predicate (`needs_key`) for each index and pass; the iterator is
  `(0..len).filter(needs_key)`. The failures are recorded as Kani false
  counterexamples, not product defects, and the native exhaustive and
  property tests use the iterator itself.

## Benchmarks

`tools/throughput-analysis/src/bin/bplus_partial.rs` builds trees in
varlit's disk index shape: 50 leaf and 78 branch entries, 34-byte keys
stored with a 16-byte inline prefix. Two key families:

- `distinct`: random keys, whose prefixes nearly always differ;
- `shared`: a common 26-byte prefix, so every stored comparison is
  undecided and needs the full keys.

Each run uses 4,096 queries, half present and half absent, and checks every
method against `std::collections::BTreeMap` before timing. Methods:

- `complete`: `bplus::Lookup` over prebuilt `Node` views of the full keys;
- `complete_validate`: the same, constructing (and so validating) each node
  per step, as the partial lookup validates each node per pass;
- `partial_attached`: `PartialLookup` over keys with their full bytes
  attached in advance, fence keys passed in advance;
- `partial_requests`: prefix-only keys copied into a stack buffer per node,
  each requested key attached, fence keys passed on request;
- `std`: `BTreeMap<Vec<u8>, u64>::get`.

Linux x86-64 cloud VM, Intel Xeon at 2.30 GHz, one CPU via `taskset`, no
frequency control; Rust 1.95.0 release build. Seeds 1, 2 and 3, method
order alternating by seed, every interval at least 1.12 s. Call-weighted
nanoseconds per lookup:

| Keys | Entries | `complete` | `complete_validate` | `partial_attached` | `partial_requests` | `std` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| distinct | 2,048 | 142.5 | 331.7 | 333.7 | 677.4 | 93.4 |
| distinct | 181,267 | 832.4 | 1,195.3 | 839.8 | 1,719.2 | 373.0 |
| shared | 2,048 | 159.5 | 394.5 | 586.3 | 10,765.2 | 112.4 |
| shared | 181,267 | 857.6 | 1,167.1 | 1,504.5 | 29,217.4 | 437.9 |

Ratios to `complete_validate`, with the range of the per-seed ratios:

| Keys | Entries | `partial_attached` | `partial_requests` |
| --- | ---: | --- | --- |
| distinct | 2,048 | 1.006 (0.953–1.074) | 2.042 (2.031–2.060) |
| distinct | 181,267 | 0.703 (0.668–0.750) | 1.438 (1.386–1.482) |
| shared | 2,048 | 1.486 (1.436–1.557) | 27.287 (26.034–28.127) |
| shared | 181,267 | 1.289 (1.227–1.343) | 25.033 (23.178–25.835) |

Requests per lookup in `partial_requests`, identical across seeds to the
third decimal:

| Keys | Entries | Height | Key requests | Fence requests |
| --- | ---: | ---: | ---: | ---: |
| distinct | 2,048 | 1 | 1.50–1.52 | 1.00 |
| distinct | 181,267 | 2 | 2.50–2.52 | 2.00 |
| shared | 2,048 | 1 | 90.95 | 1.00 |
| shared | 181,267 | 2 | 174.52–174.56 | 2.00 |

What the request counts show:

- With distinct prefixes, each level requests the child's first key and its
  fence, because equality of two incomplete keys with equal prefixes needs
  the full bytes; a found key adds one request. That is one key request and
  one fence request per level plus 0.5 for the half of the queries that are
  present.
- With a shared prefix, every key of every visited node is requested.
  Each request returns from the pass and the next call validates the node
  again, so a node of `n` keys that needs `r` requests costs `r + 1` passes
  of `O(n)` comparisons. This is the stated cost of keeping no resolution
  state between calls; attaching keys that share a prefix in advance avoids
  it (`partial_attached`).

The differences between `partial_attached` and `complete_validate` (faster
with distinct prefixes at 181,267 entries, slower with the shared prefix)
have not been analyzed at the instruction level, so no cause is claimed.

Raw records stay under `target/verification/`. Reproduce:

```sh
cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml \
  --target-dir target/verification/partial-bench --bin bplus_partial
for seed in 1 2 3; do
  taskset -c 2 target/verification/partial-bench/release/bplus_partial "$seed"
done
```
