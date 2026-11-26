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
- **Stored parts.** For an unresolved key, every comparison depends only on
  its fence; `PartialVisit` relies on this to keep decisions across
  attachments.

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

### Visiting a node with exclusive access

`provide` keeps no state, so after each `NeedKey` it validates the node
again: `r` requests cost `r + 1` passes. That is not forced by the data. It
follows from the caller holding the node mutably between calls, so the
lookup cannot know what changed.

`PartialLookup::visit(id, PartialNodeMut::leaf(&mut keys, &values))` borrows
the keys exclusively for the visit:

- `run(minimum, ceiling)` continues the pass where it stopped and returns
  `VisitStep::NeedKey(index)`, `NeedFence` or `Done`;
- `attach_with(index, |key| key.attach(full))` is the only way to change a
  key. The key must be unresolved, and the change is refused, with the
  visit, unless the key's fence (its stored parts) is unchanged and a
  resolved key is a completion of it;
- `unresolved_keys()` lists the keys the rest of the pass could request, so
  they can be fetched and attached in one round;
- `finish()` applies the answer to the lookup and returns the
  `PartialStep`. Dropping a visit leaves the node requested.

During a visit, the only change to the node is that unresolved keys become
resolved without changing their stored parts. A decided comparison used
only stored parts and resolved keys, so it stays decided, and the pass can
resume instead of starting again: one pass of `O(n)` comparisons plus `r`
resumptions per node. This relies on one more law of `PartialKey`: the
comparisons of an unresolved key depend only on its fence.

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

Toolchains: Rust 1.95.0, Verus 0.2026.11.02 (`8bf68ee`), Kani 0.68.0 with
CBMC 6.11.0 and Kissat, Miri from `nightly-2026-10-30`. Results below are
for source `83c2a5c` and later (the pass split, the `repr(C)` layout and
the harness changes); the certificate stage `bplus-partial` runs the Kani,
Miri and link checks, and the `direct-verus` stage runs the Verus target.

### What is proved with Verus

`pass` is split into `pass_over`, which holds every check and the search
and takes the four comparisons as closures over key indices, and a wrapper
that builds those closures from `order_keys`, `order_fence` and
`order_query`, checks the passed fence keys with `fence_admits` and looks
up the child of a descent. `pass_over` is verified directly on the product
source (`scripts/verify_direct_verus.py --target bplus-partial`, with
`--no-cheating`; 287 verifier obligations for the crate) for every key
count `n` and both Verus word widths.

The model: the completions of the keys, the query and the fences' full keys
are elements of one total order (`PartialKey::Full: Ord`), and finitely many
of them meet in a pass, so a ghost `rank` places them on the integers. The
closures' contracts are that an `Order` result is the order of the ranks, a
`Key(i)` names an unattached key of that comparison, and only a fence
comparison may return `Fence`. The proof is generic over the model, which a
marker trait (`Completions`) supplies; it is erased from Cargo builds.

Proved:

- every check a `Progress` records holds for the completions (the first
  key equals the minimum fence, the recorded pairs are increasing, the last
  key is below the ceiling, the search bounds and an equal key), and a pass
  keeps that true. Attaching keys leaves the completions unchanged, so a
  pass resumed from an earlier progress (`PartialVisit::run`) and a pass
  from scratch (`provide`) give the same answers;
- an answer is the specification over the completions: `Found(i)` only in a
  valid leaf with key `i` equal to the query, `Absent` when no key equals
  it (leaf) or every separator is above it (branch), `Descend(i)` with
  separator `i` the last at or below the query;
- `InvalidStructure` only for a node that is not valid (a minimum, order or
  ceiling violation);
- `NeedKey(i)` names an unattached key below `n`, so a caller that attaches
  each requested key needs at most `n` key requests per node.

Six injected defects in `pass_over` are detected (pair order not checked,
minimum accepted below, ceiling accepted at equal, search bound not
advanced, a branch descending one separator too far, a deferred ceiling
fence ignored).

Not proved with Verus: the comparison helpers and `PrefixKey` (Kani, below);
the wrapper's fence admission and child lookup, `apply`, `descend` and
`check` (Kani lookup and protocol harnesses and the tests); `PartialVisit`
and `unresolved_keys` (Kani and tests).

### Kani and the other checks

| Check | Scope | Result |
| --- | --- | --- |
| Kani, `PrefixKey` laws | Two arbitrary byte strings of up to 4 bytes, 2 inline bytes, and up to 9 bytes with 8 inline bytes: every decided comparison holds for the completions; complete keys always decide; `fence_admits` and `attach` accept exactly the consistent completions. Two lemmas prove the word-wise `cmp_short` and `cmp_prefixes` equal to byte order without stubs | Pass (4 harnesses) |
| Kani, decision contracts | The closure contracts of the Verus proof for `order_keys`, `order_fence` (fence key passed or not) and `order_query`, over two keys of up to 4 bytes with 2 inline bytes and every attachment choice | Pass (3 harnesses) |
| Kani, leaf lookup | Leaves of 1 and 2 keys of up to 2 bytes with 1 inline byte, arbitrary bytes, attachments and query: resolving each request gives the result of `bplus::Lookup` over the completions | Pass (2 harnesses) |
| Kani, branch step | Two separators, each attachment choice in its own harness: the route, `Absent` or error matches the complete-key specification, and the stored fences, origins, depth and path are the state the child step starts from | Pass (4 harnesses) |
| Kani, child step | From that state, a one-key leaf with both fences or the minimum only, fence keys in advance or on request | Pass (4 harnesses) |
| Kani, listed keys | Leaves of 1 and 2 keys, the branch step and the child step: after attaching every listed key the pass returns no `NeedKey` | Pass (5 harnesses) |
| Kani, visits and protocol | A visit of a one-key leaf and of a child (fences on request or the minimum in advance) gives the complete-key result; `attach_with` refuses changed stored parts; out-of-turn identifiers, unadmitted fence keys and requests leave the lookup unchanged | Pass (5 harnesses) |
| Native exhaustive tests | Leaves of 0 to 3 keys over bytes {0, 1, 2, 3}, lengths 0 to 2, 1 inline byte, every attachment choice and query (1,593,795 cases including the visit and listed paths), and 37,044 branch visits, against `bplus::Lookup` | Pass |
| Property tests | Random trees with shared prefixes against `std::collections::BTreeMap`, through `provide` and through `visit` | Pass |
| Miri | The ten integration tests on x86-64 and i686 with strict provenance | Pass |
| Final link | `tools/consumer-audit/cases/bplus_partial.rs` in the release-checked and release-LTO-checked profiles: no retained panic or exit edge; the overflow, bounds and abort controls are detected | Pass |

The 27 Kani harnesses were run one per process with `--solver kissat`,
`-Z stubbing` and `--object-bits 12` (6 to 728 s each). The lookup-level
harnesses replace `cmp_short` and `cmp_prefixes` by byte order, which the two
lemmas prove equal.

Kani limits and how they were handled:

- A single harness over a leaf of 3 keys, or over a root and two leaves,
  exhausts CBMC memory. The proof is split into the leaf, branch and child
  steps, which compose through the lookup state; the three-key leaf is
  covered by the native exhaustive test.
- Kani harnesses for the resumption lemma (a resumed pass equals a pass from
  scratch) passed for two-key leaves but the branch harnesses ran out of
  memory at 15 GB, also when split by attachment choice and by input
  region (CBMC builds the whole formula before an assumption takes effect).
  They are replaced by the Verus proof above, which covers every key count.
- False counterexamples over `[PrefixKey]` came from a CBMC fault (6.7.0 to
  at least 6.11.0) that resolves a pointer to an array field at a nonzero
  offset of an array element with a symbolic index to the wrong element;
  see `docs/verification/kani-false-counterexamples.md`. `PrefixKey` and
  `PrefixFence` keep the prefix at offset 0 (`repr(C)`, prefix first); a
  copy with this layout verified the leaf resumption lemma that failed with
  the default layout (offset 24), and the sizes are unchanged on 64-bit
  targets. CBMC 6.6.0, which predates the fault, ran out of memory on the
  same goto program.
- The harnesses select requested keys with concrete loops and evaluate the
  `unresolved_keys` predicate per index instead of consuming the iterator;
  the native tests use the iterator itself.

The module adds no `unsafe` code.

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
- `partial_attached`: `PartialLookup::provide` over keys with their full
  bytes attached in advance, fence keys passed in advance;
- `partial_requests`: prefix-only keys copied into a stack buffer per node,
  each requested key attached and the node provided again, fence keys
  passed on request;
- `partial_listed`: as `partial_requests`, but the keys
  `unresolved_keys` lists are attached in one round first;
- `partial_visit`, `partial_visit_listed`: the same two strategies through
  `PartialLookup::visit`, which resumes the pass instead of starting again;
- `std`: `BTreeMap<Vec<u8>, u64>::get`.

Linux x86-64 cloud VM, Intel Xeon at 2.30 GHz, one CPU via `taskset`, no
frequency control; Rust 1.95.0 release build. Seeds 1, 2 and 3, method
order alternating by seed, the call counts pinned with
`BPLUS_PARTIAL_BATCHES_<family>_<size>` (3,600, 1,000, 2,800 and 700
batches of 4,096 lookups), every interval at least 1.21 s. Call-weighted
nanoseconds per lookup, source `a56fceb` (the build of the last paired
comparison below):

| Keys | Entries | `complete` | `complete_validate` | `partial_attached` | `partial_listed` | `partial_requests` | `partial_visit` | `partial_visit_listed` | `std` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| distinct | 2,048 | 168.2 | 374.9 | 369.8 | 1,022.0 | 728.7 | 511.2 | 865.6 | 116.0 |
| distinct | 181,267 | 1,053.8 | 1,454.4 | 1,007.4 | 2,430.4 | 1,863.2 | 1,293.9 | 2,068.7 | 420.2 |
| shared | 2,048 | 191.1 | 438.7 | 632.3 | 1,723.5 | 12,229.6 | 2,051.8 | 1,458.0 | 126.3 |
| shared | 181,267 | 930.1 | 1,280.2 | 1,750.7 | 4,206.7 | 30,275.5 | 4,640.3 | 3,605.3 | 447.7 |

Keys attached and key rounds (calls that ended in `NeedKey`) per lookup,
equal across seeds to within 0.02:

| Keys | Entries | Height | Keys attached | Rounds, per request | Rounds, listed | Fence requests |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| distinct | 2,048 | 1 | 1.50 | 1.50 | 1.01 | 1.00 |
| distinct | 181,267 | 2 | 2.50 | 2.50 | 2.00 | 2.00 |
| shared | 2,048 | 1 | 90.95 | 90.95 | 2.00 | 1.00 |
| shared | 181,267 | 2 | 174.52–174.56 | 174.52–174.56 | 3.00 | 2.00 |

What the counts show:

- With distinct prefixes, each level requests the child's first key and its
  fence, because equality of two incomplete keys with equal prefixes needs
  the full bytes; a found key adds one request.
- With a shared prefix, every key of every visited node is needed. Per
  request, `provide` validates the node again for each of them, so a node of
  `n` keys that needs `r` requests costs `r + 1` passes (`partial_requests`);
  `visit` resumes instead, one pass plus `r` resumptions
  (`partial_visit`, 0.15 to 0.17 of `partial_requests`).
- Listing reduces the rounds to about one per level but attaches every
  listed key. With distinct prefixes the listing compares every key against
  the query and its neighbours, so `partial_listed` is slower than
  `partial_requests` there; with a shared prefix it is faster.

### Changes and their paired comparisons

Each change was compared against the build it replaced, with the same
seeds, pinned call counts and alternating order; `complete` and `std` are
unchanged controls. Executed instructions come from Callgrind over the timed
region only (4,096 lookups, `BPLUS_PARTIAL_INSTRUCTIONS=1`).

| Change | Partial methods, new/previous time | Controls | Instructions | Finding |
| --- | --- | --- | --- | --- |
| Word-wise prefix comparison (`cmp_short`, `cmp_prefixes`) | 0.626–0.958 in all 12 partial conditions | not recorded here | 193 fewer `memcmp` calls per lookup | kept |
| `PrefixKey` fields in `repr(C)` order, prefix first | 0.927–1.087 | 0.85–1.06 | equal or 0.2–2.2% fewer in all 12 conditions | kept; the largest ratio (1.087, shared 181,267 `partial_attached`) has identical instruction counts and is within the controls' movement |
| `pass` split into `pass_over` and closures (Verus) | 0.904–1.033 | 0.958–1.098 | `pass` 3.2% fewer instructions, 21.8% more data reads per lookup | kept; no partial ratio is outside the controls' movement |
| Attached key tails compared as slices instead of `Option<&[u8]>` (`72ebbdb`) | shared 181,267 `partial_attached` 1.26 (per seed 1.16–1.36); the other partial conditions 0.94–1.08. Over six seeds a build with only this expression restored measured 0.87–1.03 of `main` where the changed build measured 1.04–1.40 | 0.93–1.08 | `pass` 12,282 against 13,157 per lookup; simulated cache misses equal | rejected, reverted in `440f963`: two predicted `cmp $0x10; jb` per compared pair became `sub $0x10` and four `cmovb`, so both `memcmp` pointer arguments depend on the key length loads |
| Listing only the keys the pass's search could compare (`a21c8ae`) | three seeds, all partial methods: 0.93–1.09; six seeds, listed methods: 0.96–1.05 | 0.91–1.05; 0.96–1.07 | distinct 181,267 `partial_listed`: 87.3M against 107.1M over 4,096 lookups; simulated conditional mispredictions of the listing 145,652 against 51,294 | rejected, reverted in `c7a3442`: no condition faster beyond the controls' movement |

The split first passed the closures by reference with captures by
reference; `pass` then reloaded the key slice through the closure
environment (1,071 stack-operand instructions per lookup against 553).
Passing the closures by value with moved captures restored the static stack
loads of `pass` to the previous 52; three stack reads per pair-loop
iteration remain (the loop end and two values kept across `memcmp`), which
the timings above include.

The two rejected changes were measured on 2026-11-26 against `main`
(`5806316`); the source is `main`'s again.

Raw records stay under `target/verification/`. Reproduce:

```sh
cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml \
  --target-dir target/verification/partial-bench --bin bplus_partial
export BPLUS_PARTIAL_BATCHES_distinct_2048=3600 BPLUS_PARTIAL_BATCHES_distinct_181267=1000 \
  BPLUS_PARTIAL_BATCHES_shared_2048=2800 BPLUS_PARTIAL_BATCHES_shared_181267=700
for seed in 1 2 3; do
  taskset -c 2 target/verification/partial-bench/release/bplus_partial "$seed"
done
python3 tools/perf-audit/paired_ab.py --base BASE-seed*.csv --new NEW-seed*.csv --controls complete,std
```
