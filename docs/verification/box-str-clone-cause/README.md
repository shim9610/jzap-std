# Four 1MiB string clones: timing and assembly

The reported **6.4% slowdown** was the point estimate for unmetered `Box<str>`
cloning four 1MiB strings. It was not a confirmed fixed overhead. An additional
30-process run of the identical executable produced a **2.65%** point estimate;
both 95% ratio intervals include parity. Broader measurement remains pending.

| Dataset | std median, ns | jzap median, ns | jzap / std | 95% ratio interval |
| --- | ---: | ---: | ---: | --- |
| Original, unmetered | 905,958.50 | 963,786.00 | 1.06383 | 0.99202–1.12240 |
| Repeat, unmetered | 1,008,086.25 | 1,034,850.25 | 1.02655 | 0.96260–1.12673 |
| Repeat, metered | 1,008,086.25 | 989,918.75 | 0.98198 | 0.90809–1.07503 |

Each dataset contains 30 independent processes per implementation, with two warmup
and 30 retained cycles. Destination handle-vector allocation and pushes are inside
the timed interval. CPU affinity and alternating execution order were retained.
The product source and executable were unchanged. The original and repeat samples
remain separate. The std-vs-std control itself differed by +2.698% originally and
−0.695% in the repeat. No quantitative attribution to cache behavior, page faults,
or scheduling was established.

## Actual execution path

A separate diagnostic run traced `memcpy` return addresses in the original timing
executable. Its LD_PRELOAD-instrumented times were excluded from the performance
statistics. Uninstrumented timing used the original executable with SHA-256:

`8a6d1866248b5682e8bb161a1de327c4bca7f22f3f65ead61d5551e0c43d9dd5`

| Operation | std `Box<str>::clone` | Unmetered `try_clone` |
| --- | --- | --- |
| Execution location | Called at `main+0x2741`; function at `0x53550` | Inlined into main; loop at `0x18d83` |
| Empty-length test | `0x5355e test`, `0x53561 je` | `0x18d99 test`, `0x18d9c je` |
| Allocation shim check | `0x53563 call` | `0x18d9e call` |
| Allocation | `0x53571 call` to `0x1aa90 __rust_alloc` | `0x18dac call` to the same target |
| Null check | `0x5357a test`, `0x5357d jne` | `0x18db5 test`, `0x18db8 jne` |
| Content copy | `0x5359b call memcpy` | `0x18d4a call memcpy` |
| Copy work | One 1MiB copy per string | Same |
| UTF-8 revalidation | None | None |
| Budget access | None | None |
| Success output | Data pointer and length | Same two values; inlining removes Result construction/discrimination |
| Separate clone call/return | call/ret and rbp/r14/rbx save/restore | None; clone executes within the loop |

Both allocations use GOT entry `0x5c440`; both copies use `0x5c490`. There is no
extra allocation or extra pass over the 1MiB content in the observed unmetered
path. Failure branches differ: std enters allocation-error handling while jzap
constructs an error. Those branches were not taken in this successful workload.
Register allocation, stack temporaries, and index calculations are not identical.
Their existence does not establish causation for the timing difference.

The metered path is separate. At `0x171fd` it loads the budget pointer, at `0x17206`
the available bytes, then uses `sub`/`jb` to check refusal. At `0x17216` it stores
the reserved availability; after allocation, `0x1719b` updates occupied bytes and
`0x171d4` stores the budget pointer in the new handle. These instructions are absent
from the unmetered case that produced the original 6.4% estimate.

## Evidence and reproduction

The retained [original summary](original-summary.json),
[repeat summary](repeat-summary.json), and [metadata](metadata.json) identify the
measured conditions and executable. Maintained benchmark tools generate
new samples and disassembly for the current checkout.

Follow the [benchmark prerequisites](../../benchmarks.md) and use
`compare_box_dynamic.py` for a new timed run. To diagnose a newly built executable,
the reusable `tools/box-analysis/diagnostics/trace_memcpy.c` helper can trace copy
call sites in a separate run. Recompute addresses from that executable: addresses
above apply only to the recorded hash. Instrumented times must remain separate
from performance results. This case supports the
[provisional checkpoint](../box-completion/README.md), not universal parity.
