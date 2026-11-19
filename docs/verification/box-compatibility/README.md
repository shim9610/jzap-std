# Box syntax compatibility study

This historical compiler study separates standard Box's compiler privileges from
functionality that an ordinary library type can provide. Current explicit
adapters are implemented; see the [Box API matrix](../box-completion/README.md).

Seven cases were compiled on both Rust 1.95.0 and nightly-2026-10-30, yielding
14 expected outcomes. Successful cases were also executed. Expected compilation failures are evidence of language
restrictions, not failed product tests.

| Case | Stable 1.95 | Pinned nightly | Functional replacement |
| --- | --- | --- | --- |
| Standard Box control | Compiles and runs | Compiles and runs | Reference behavior |
| Move out through ordinary Deref | Rejected | Rejected | `Box::into_inner` |
| Implicit unsizing without a custom implementation | Rejected | Rejected | `box_unsize!`, `pin_box_unsize!` |
| Consume a FnOnce through ordinary Deref | Rejected | Rejected | `box_call_once!` |
| Implement CoerceUnsized | Unstable-feature rejection | Compiles with feature enabled | Explicit compiler-checked pointer coercion |
| Implement FnOnce directly | Unstable-feature rejection | Compiles with feature enabled | Explicit consuming-call adapter |
| Explicit-control alternatives | Compiles and runs | Compiles and runs | Stable library approach |

The probes live in [tools/box-compatibility](../../../tools/box-compatibility).
The product does not require nightly merely to supply these explicit operations.
Unstable allocator and callable-trait experiments do not establish stable support
for those exact trait implementations.

```sh
python3 scripts/check_box_compatibility.py
```

Run from the repository root with both named toolchains installed. The current
product's adapters additionally have runtime and compile-fail tests; this study
alone is not their verification report.
