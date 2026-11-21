# Recorded decisions

Product decisions taken during implementation work, with their conditions and
current status. Each entry names the report that holds the measurements.

## FixedTree unchecked node indexing (2026-11-22)

- **Proposal.** `FixedTree::get` and `FixedTree::range` re-check node IDs and
  leaf ranks on every lookup, although `try_new` has already validated every
  child ID, height and parent and the pools are immutably borrowed. The
  proposal moves these checks to construction and uses unchecked indexing
  internally. The public API, results and returned errors stay the same; leaf
  slice checks stay, so an inconsistent `Ord` still returns
  `InvalidStructure`.
- **Approval terms.** Internal `unsafe` is allowed only with thorough formal
  verification of the construction-time height and ID invariant, and no
  `unsafe` may appear in the public API.
- **Status: deferred.** An unproved upper-bound experiment gained about 1 ns
  on empty and single-item ranges, lost 1–2% on 32- and 256-item ranges, and
  left the `brie-tree` gap largely in place. Point-lookup differences could
  not be attributed because unchanged `brie-tree` code doubled in the same
  binary. Work resumes only on a new instruction.
- **Measurements.** [Linux bottleneck follow-up](verification/linux-bottlenecks.md#width-eight-fixedtree-range-leaf-bounds-use-fixed-offsets).
