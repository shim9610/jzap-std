# BTreeMap design and reference review

This document records the implementation contracts and pinned references.
Executed acceptance results are in the [verification report](verification/btree.md).

## Fixed references

| Reference | Identity | Use and limitations |
| --- | --- | --- |
| Rust stable `alloc::collections::btree` | Rust 1.95.0, `59807616e`, 2026-04-14 | Degree six, separate leaf/internal layouts, insertion split points, border repair and ordered traversal. Standard allocation and panic contracts cannot be reused unchanged. |
| Rust nightly `alloc::collections::btree` | nightly-2026-10-30, `92b5b8c87` | Comparison of append, split and node changes. Nightly APIs do not define the stable support inventory. |
| kernel-btree 0.1.0 | `8a0abca445a0ce62e6897b753a27c5a51725b2ad` | Preallocation before insertion and allocation-free deletion. The cached published source compiles with Rust 1.95.0 (`cargo check --offline --lib`); a docs.rs nightly build alone does not establish a nightly requirement. Its arithmetic, ownership and clone contracts need independent review. No product dependency. |
| btree-slab 0.6.1 | `fb67001e9e42a896f3ee20ebccbd0024ffc39f5c` | Arena identifiers and node navigation. Its slab/smallvec/cc-traits dependencies and allocation behavior are not adopted. |
| generic-btree 0.10.7 | `f88a9f7ff4e442fc4f6aebfaa30496652b92decd` | Typed query/path patterns. It uses std collections and arena/cache/sequence semantics, so it is not a drop-in fallible ordered map. No product dependency. |

These are source references, not dependency or maintenance endorsements. Rust and
kernel-btree/btree-slab use MIT OR Apache-2.0; generic-btree uses MIT. The product
implementation uses this repository's existing allocation and ownership contracts.

Stable `map.rs` SHA-256:
`22491e7cf174ca53d6a752f072b264cc54656df8b8a9ea2e4cd47150052d5eee`.
Stable `node.rs` SHA-256:
`2200ca0a4fe38592c314dc7f5f40bbb160e06b6f9e4abd1eb5bc40701540ab25`.
Nightly `map.rs` SHA-256:
`3f7541119364c1fc05ea135b097cae74131326ff818fdc758a03ead453fcb19a`.
Nightly `node.rs` SHA-256:
`7e21ad0d497a06c3186064214916e504c82a34836ed231781f71fa6d8e6f3077`.

## Representation and failure contracts

Nodes contain at most eleven key/value pairs. Internal nodes add twelve child
pointers; leaves do not pay for child storage. The map holds a root, height,
checked length and allocation policy. Parent links support bounded-stack traversal.
Allocation requests use `storage` and the selected existing budget policy.

Insertion searches before modifying storage. Replacing an existing value needs no
allocation. Inserting a new key first checks total length and reserves every node
needed by the full ancestor chain. A refusal returns `Rejected<(K, V)>` and leaves
keys, values, order, shape, length and budget charges unchanged. No allocation or
user comparison follows the insertion commit point. Removed nodes are freed before
their charges are refunded. Deletion and traversal allocate no storage.

User `Ord`, destructors, initializers and fallible callbacks retain their documented
execution contracts. Even inconsistent ordering must not invalidate memory bounds.
Internal count/index transitions have closed node bounds; total length is checked
before mutation. Numeric proofs and memory execution checks have different roles.

Borrowed node slices validate local ordering, key/value/edge lengths and inherited
search bounds before exposing results. A continuation can request another node by
identifier. The caller supplies and owns all slices. Validation of visited nodes
does not certify unseen nodes. No file representation, I/O, cache, mapping,
transaction, persistence or recovery layer is introduced.

## Acceptance routine

The stable API inventory, refusal ownership tests, std differential state machines,
Miri, sanitizers, fuzzing, source-connected numeric proofs, deliberate mutants and
final consumer link roots must cover each new path before it is marked supported.
Loom applies to shared budget accounting through the existing budget tests; an
owned tree has no new reference-count protocol.

Performance uses identical pregenerated shuffled corpora with three seeds. Every
retained product and std timing interval lasts at least one second. Default and
optional loop-alignment builds are separate. Time, requested/actual peak and held
allocation, and handle size are reported separately. The existing baseline target
is within 15% on representative successful work; a regression requires assembly
and controlled cause analysis. Raw outputs stay under ignored `target/verification`.
