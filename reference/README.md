# Borrowed source provenance

These are reference snapshots, not compiled runtime dependencies.

- Varlit fallible.rs: cd7b0446da3e731fba867802bef5f4b7b3fc328d,
  ownership ancestor e84d0d93af5b689057f9b1c7e7b1f332002a9c62,
  crates/varlit-core/src/fallible.rs, accompanying MIT/Apache licenses.

Selected: scalar/reference/Option/tuple fallible cloning, allocate-before-clone
ordering. Excluded: standard Arc/Rc Clone delegation, Varlit-specific error paths
and standard owning-container exposure. Added: target-budget propagation, array
partial cleanup, recorded initialization, typed allocation rejection.

Vec range editing was reviewed against Rust 1.95 (`59807616e`) alloc Vec,
Drain, ExtractIf and Splice, and nightly-2026-10-30 (`92b5b8c87`). Rust uses
MIT OR Apache-2.0 licensing; source and licenses are available in
[the pinned Rust tree](https://github.com/rust-lang/rust/tree/59807616e/library/alloc/src/vec).
No Rust source snapshot or external runtime dependency is added here. The
[Vec contract](../docs/vec-contracts.md) describes algorithmic references and
jzap-specific changes. Notably, jzap writes drained holes as `MaybeUninit<T>`;
nightly also avoids constructing a slice of uninitialized `T` in splice filling.
