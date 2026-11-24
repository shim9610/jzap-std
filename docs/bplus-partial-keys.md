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

## Verification plan

- **Kani, lookup equivalence.** For small nodes of `PrefixKey<2>` with
  arbitrary bytes, lengths and attachments, each `provide` result is a
  request or equals `bplus::Lookup` over the completed keys, including
  errors.
- **Kani, `PrefixKey` laws.** Consistency of every comparison and of
  `attach` and `fence_admits` against byte-slice order.
- **Kani, protocol.** Requests and refusals leave the lookup state
  unchanged.
- **Property tests.** Random trees with long shared prefixes and random
  attachment choices, compared with `std::collections::BTreeMap`. Damaged
  nodes and wrong attachments are refused. Request counts stay within the
  bound above.
- **Miri** on the lookup tests, and the panic/termination link audit for
  the new public entry points.

## Benchmark plan

- The complete-key lookups (`bplus::Lookup`, strided and fixed views) before
  and after the `LookupStep` change, with the existing paired intervals of at
  least one second.
- The partial lookup with every key complete or attached in advance,
  against `bplus::Lookup` over the same keys.
- Lookups that need requests, reported as passes and requests per lookup.
