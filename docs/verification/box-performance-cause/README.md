# Historical diagnosis: benchmark barriers and handle size

This experiment identified a measurement artifact affecting 16-byte handles.
It precedes the [current Box checkpoint](../box-completion/README.md). Its historical assembly
and samples identify that diagnosis, not a claim of performance across
machines or allocation conditions.

## Controlled change

With `source.push(black_box(new(black_box(i))))`, a 16-byte returned handle was
written using two 8-byte stack stores and reloaded with a 16-byte `movdqa`. The
8-byte std handle did not require the same sequence. A 16-byte std wrapper with
no budget reproduced the difference, separating handle materialization from
budget bookkeeping. CPU stall events were not measured.

Moving the result barrier to the completed source/target vector, before stopping
the timer, removed the per-item temporary. Input barriers, allocation work, and
budget checks remained. The experiment used CPU affinity, two warmup cycles,
30 retained cycles, and 15 independent processes.

| Creation of 10,000 values | Per-item barrier, ns/item | Phase barrier, ns/item |
| --- | ---: | ---: |
| std Box | 10.071 | 10.071 |
| std 16-byte wrapper | 12.542 | 10.373 |
| Metered jzap Box | 12.531 | 10.659 |

Workload checks at counts 0, 1, 37, and 10,000 covered std, its 16-byte wrapper,
unmetered/metered jzap, and Varlit. They retained allocation counts and peaks,
with zero live requested bytes after cleanup. For positive counts, the complete
create/clone lifecycle made `2n + 2` allocations.

## Recorded post-change ratios

| Count | Operation | Metered / std median ratio | 95% interval |
| ---: | --- | ---: | --- |
| 10,000 | Create | 1.0440 | 1.0302–1.0576 |
| 10,000 | Clone | 1.0440 | 1.0360–1.0656 |
| 10,000 | Drop | 1.0200 | 1.0025–1.0466 |
| 100,000 | Create | 0.9966 | 0.9639–1.0128 |
| 100,000 | Clone | 1.0068 | 0.9668–1.0448 |
| 100,000 | Drop | 1.0123 | 0.9976–1.0521 |

The observed artifact justified correcting the harness, not waiving genuine
budget costs or the independent memory criterion. The accepted extra budget
pointer remains explicit in memory comparisons.

## Evidence

These observations summarize the original timings, disassembly and workload
tests. Maintained benchmarks produce new timing and assembly outputs locally.
The [benchmark methodology](../../benchmarks.md) describes current datasets and
reproduction prerequisites.
