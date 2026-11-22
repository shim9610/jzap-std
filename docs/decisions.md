# Recorded decisions

Product decisions taken during implementation work, with their conditions and
current status. Each entry names the report that holds the measurements.

## FixedTree unchecked node indexing (2026-11-22)

- **Proposal.** `FixedTree::get` and `FixedTree::range` re-check node IDs and
  leaf ranks on every lookup, although `try_new` has already validated every
  child ID, height and parent and the pools are immutably borrowed. The
  proposal moves these checks to construction and uses unchecked indexing
  internally. The public API, results and returned errors stay the same.
- **Approval terms.** Internal `unsafe` is allowed only with thorough formal
  verification that the `try_new` validation (child IDs, heights, unique
  parents, children before parents, root last) keeps every lookup index in
  bounds. No `unsafe` may appear in the public API, and memory safety must
  hold for an inconsistent user `Ord`. The signature
  `Result<Option<&V>, Error>` and the results for keys that keep the
  documented stable order stay identical.
- **First experiment (deferred).** An unproved upper-bound experiment gained
  about 1 ns on empty and single-item ranges and lost 1–2% on 32- and 256-item
  ranges. Its point-lookup effect could not be attributed, because unchanged
  `brie-tree` code doubled in the same binary; this was not a finding of no
  gain.
- **Status: implemented for `get` only (approved 2026-11-22).** `get` reads
  the branch, child, leaf and value records without bounds checks, routes
  each branch with a branch-free `floor_slot` that returns no `Option`, and
  checks the first-key fence at the root only. `range` keeps its checks. The
  proofs, defect controls and paired measurements are in the [Linux
  bottleneck follow-up](verification/linux-bottlenecks.md#fixedtree-point-lookup-reads-the-validated-pools-unchecked).
  Timings on the pinned Linux PC are a separate step and have not been run.
