# Instruction-level review for local timing

Date: 2026-11-08. Branch `work/staging-perf-687db7d`; product sources as of
`506d2eb`, tools as of `225c01d`. Compiler: Rust 1.95.0 (LLVM 22.1.2), release
profile of `tools/throughput-analysis` (`opt-level=3`, `codegen-units=1`,
`panic=abort`).

This report records executed-instruction counts and assembly observations made
in a shared cloud VM where wall-clock comparisons were not decisive. It selects
cases for timing on a dedicated machine. It is not a timing result, a
performance pass, or a library-completion claim.

## Measurement environment and limits

| Item | Observation |
| --- | --- |
| Host | Firecracker microVM on KVM; 4 vCPUs, 1 thread per core; 16 GB |
| CPU | Reported as `Intel(R) Xeon(R) Processor @ 2.10GHz`; L1d 48 KiB and L2 2 MiB per vCPU; one 260 MiB L3 shared with the host |
| Hardware counters | Not exposed: no `cpu` PMU device; `perf_event_open` for cycles fails with ENOENT |
| Steal | About 1% while one vCPU was busy for 5 s |
| CPU frequency | Not observable inside the VM |
| Same-code timing spread | Arithmetic: 130 identical-code pairs, median 2.9%, maximum 19.2%. Box: std control within ±9.4%. Vec `grow`: std control +38.2% in one seed |
| Pinning | `compare.py` pins one vCPU. On vCPU 0 and vCPU 3 the same `grow` processes still split into a fast and a slow group |

Timing differences below roughly 20% were not distinguishable in this VM.

## Method

`tools/throughput-analysis/instruction_counts.py` runs every task of the 11
benchmark executables for each implementation label (the `compare.py` labels
without the duplicate std control). Each case runs under callgrind with one and
with five measured epochs, seed 617, address randomization disabled
(`setarch -R`). The per-function difference removes start-up, corpus
generation, warmup and teardown; dividing by the difference of the reported
operation counts gives instructions per benchmark call. The hash benchmarks use
their documented `hash-map` / `hash-set` feature builds.

- 1,075 cases ran; none failed.
- Repeated runs without ASLR give identical totals. Identical-code labels can
  still differ by about 0.2 instructions per call (for example u64 Box
  `clone_from`: std 16.3, unmetered 16.1); the source of this residue was not
  identified. Treat differences of that size as resolution.
- Instructions are not cycles. Cache misses, branch mispredictions and code
  placement are absent.
- libc `memcpy`/`memset` use `rep movsb/stosb` for large sizes. Callgrind counts
  each repetition, so copy instruction counts track bytes, while an inlined
  copy loop counts its real instructions. `classify_instruction_counts.py`
  therefore compares code plus allocator instructions only, and classifies:
  `no-copy` (copy at most 5% of either total), `same-copy` (both copy counts
  within 5%) and `copy-differs` (otherwise; no ratio is meaningful).

| Class | Cases |
| --- | ---: |
| no-copy | 358 |
| same-copy | 273 |
| copy-differs | 176 |

Of 631 comparable cases, 310 have at most std's instruction count, 439 are
within 1.05, 542 within 1.15 and 89 above 1.15. The compact per-case data is in
[instruction-counts-classified.csv](instruction-counts-classified.csv).

### Summary by benchmark and implementation

Ratios compare code plus allocator instructions per call with std.

| Benchmark | Implementation | Comparable | ≤ 1.00 | ≤ 1.05 | ≤ 1.15 | > 1.15 | Copy differs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Box / Vec | limited | 20 | 7 | 16 | 20 | 0 | 3 |
| Box / Vec | limited_metered | 20 | 6 | 9 | 11 | 9 | 3 |
| Box / Vec | metered | 29 | 8 | 12 | 17 | 12 | 5 |
| Box / Vec | unlimited | 31 | 17 | 27 | 31 | 0 | 3 |
| String | limited | 11 | 6 | 9 | 10 | 1 | 0 |
| String | limited_metered | 11 | 4 | 6 | 10 | 1 | 0 |
| String | metered | 11 | 5 | 7 | 10 | 1 | 0 |
| String | unlimited | 11 | 8 | 9 | 11 | 0 | 0 |
| Rc / Arc | concurrent | 30 | 9 | 10 | 20 | 10 | 0 |
| Rc / Arc | local | 30 | 8 | 13 | 21 | 9 | 0 |
| Rc / Arc | unlimited | 30 | 20 | 22 | 23 | 7 | 0 |
| Arc scenarios | borrowed | 17 | 4 | 11 | 16 | 1 | 2 |
| Arc scenarios | owned | 17 | 5 | 11 | 15 | 2 | 2 |
| Arc scenarios | unlimited | 19 | 11 | 19 | 19 | 0 | 0 |
| BTreeMap | concurrent | 11 | 10 | 11 | 11 | 0 | 5 |
| BTreeMap | local | 11 | 10 | 11 | 11 | 0 | 5 |
| BTreeMap | unlimited | 11 | 11 | 11 | 11 | 0 | 5 |
| BTreeSet | concurrent | 24 | 18 | 21 | 22 | 2 | 3 |
| BTreeSet | local | 24 | 19 | 21 | 23 | 1 | 3 |
| BTreeSet | unlimited | 24 | 19 | 21 | 22 | 2 | 3 |
| HashMap | concurrent | 13 | 8 | 8 | 12 | 1 | 5 |
| HashMap | local | 13 | 9 | 9 | 13 | 0 | 5 |
| HashMap | unlimited | 12 | 7 | 8 | 12 | 0 | 6 |
| HashSet | concurrent | 26 | 9 | 14 | 20 | 6 | 1 |
| HashSet | local | 26 | 7 | 13 | 19 | 7 | 1 |
| HashSet | unlimited | 26 | 7 | 13 | 18 | 8 | 1 |
| BinaryHeap | concurrent | 20 | 7 | 17 | 19 | 1 | 10 |
| BinaryHeap | local | 21 | 7 | 17 | 20 | 1 | 9 |
| BinaryHeap | unlimited | 21 | 10 | 19 | 20 | 1 | 9 |
| VecDeque | concurrent | 9 | 7 | 8 | 9 | 0 | 27 |
| VecDeque | local | 9 | 6 | 7 | 9 | 0 | 27 |
| VecDeque | unlimited | 9 | 6 | 8 | 9 | 0 | 27 |
| Scratch / slice | concurrent | 8 | 6 | 6 | 6 | 2 | 2 |
| Scratch / slice | local | 8 | 6 | 6 | 6 | 2 | 2 |
| Scratch / slice | unlimited | 18 | 6 | 9 | 16 | 2 | 2 |

### Comparable cases above 1.15

| Benchmark | Task | Profile | Class | std instructions/call | Implementations above 1.15 (ratio) |
| --- | --- | --- | --- | ---: | --- |
| HashMap | `hash_map_default_new` | mixed | no-copy | 13.2 | concurrent 2.04 |
| Box / Vec | `box_slice_clone_from` | mixed | same-copy | 20.8 | metered 1.70 |
| Rc / Arc | `arc_weak_clone` | mixed | no-copy | 19.2 | concurrent 1.68, unlimited 1.68, local 1.67 |
| Box / Vec | `clone` | small | same-copy | 167.4 | limited_metered 1.68, metered 1.60 |
| Scratch / slice | `scratch_extend_8` | mixed | same-copy | 42.8 | unlimited 1.59, concurrent 1.40, local 1.31 |
| Arc scenarios | `arc_app_domain64_t1` | mixed | no-copy | 498.5 | owned 1.54 |
| Arc scenarios | `arc_app_domain64_t4` | mixed | no-copy | 505.6 | owned 1.52 |
| BinaryHeap | `binary_heap_iter_256` | mixed | no-copy | 604.4 | unlimited 1.43, concurrent 1.43, local 1.43 |
| Scratch / slice | `scratch_extend_256` | mixed | same-copy | 46.3 | unlimited 1.42, concurrent 1.38, local 1.31 |
| BTreeSet | `btree_set_intersection_disjoint` | mixed | no-copy | 98.0 | unlimited 1.41, concurrent 1.41, local 1.41 |
| Box / Vec | `pop` | small | same-copy | 301.2 | limited_metered 1.39, metered 1.35 |
| Box / Vec | `clone` | mixed | same-copy | 300.3 | limited_metered 1.38, metered 1.34 |
| Box / Vec | `drain` | mixed | same-copy | 302.0 | limited_metered 1.37, metered 1.33 |
| Box / Vec | `split_off` | mixed | same-copy | 584.2 | limited_metered 1.35, metered 1.27 |
| String | `string_reserved` | mixed | same-copy | 24.2 | limited_metered 1.33, limited 1.16 |
| Box / Vec | `insert_remove` | mixed | same-copy | 668.3 | limited_metered 1.31, metered 1.27 |
| Box / Vec | `extract_all` | small | same-copy | 336.4 | limited_metered 1.30, metered 1.26 |
| Rc / Arc | `rc_clone` | mixed | no-copy | 10.2 | concurrent 1.30, unlimited 1.30, local 1.29 |
| Rc / Arc | `arc_cow_unique` | mixed | no-copy | 17.2 | local 1.29, concurrent 1.29, unlimited 1.23 |
| HashSet | `hash_set_shrink` | mixed | same-copy | 92,896.1 | concurrent 1.29, local 1.29, unlimited 1.26 |
| HashSet | `hash_set_reserve` | mixed | same-copy | 46,637.6 | concurrent 1.28, local 1.28, unlimited 1.26 |
| HashSet | `hash_set_owned_symmetric_difference` | mixed | no-copy | 235,396.9 | local 1.27, concurrent 1.27, unlimited 1.26 |
| Rc / Arc | `arc_cyclic` | mixed | no-copy | 197.2 | concurrent 1.26, local 1.24 |
| Rc / Arc | `rc_downgrade` | mixed | no-copy | 12.2 | concurrent 1.26, unlimited 1.25, local 1.25 |
| Rc / Arc | `arc_downgrade` | mixed | no-copy | 20.2 | unlimited 1.25, concurrent 1.25, local 1.24 |
| Box / Vec | `append` | mixed | same-copy | 1,206.6 | limited_metered 1.25, metered 1.22 |
| Box / Vec | `shrink` | mixed | same-copy | 620.7 | limited_metered 1.25, metered 1.23 |
| HashSet | `hash_set_owned_difference` | mixed | no-copy | 118,058.9 | concurrent 1.23, local 1.23, unlimited 1.23 |
| HashSet | `hash_set_clone` | mixed | same-copy | 343.7 | local 1.23 |
| Box / Vec | `box_u64_clone_drop` | mixed | no-copy | 161.1 | metered 1.22 |
| Rc / Arc | `arc_cyclic_infallible` | mixed | no-copy | 205.7 | concurrent 1.21, local 1.19 |
| Rc / Arc | `rc_weak_clone` | mixed | no-copy | 19.2 | local 1.21, concurrent 1.21, unlimited 1.20 |
| Arc scenarios | `arc_app_mutex_t1` | mixed | no-copy | 37.2 | borrowed 1.20 |
| HashSet | `hash_set_build` | mixed | no-copy | 143,083.7 | local 1.18, concurrent 1.18, unlimited 1.17 |
| HashSet | `hash_set_retain` | mixed | same-copy | 11,531.9 | unlimited 1.18 |
| Rc / Arc | `arc_upgrade` | mixed | no-copy | 23.2 | unlimited 1.17, local 1.17, concurrent 1.17 |
| HashSet | `hash_set_owned_union` | mixed | no-copy | 160,769.3 | local 1.17, concurrent 1.17, unlimited 1.17 |
| HashSet | `hash_set_is_subset` | mixed | no-copy | 237.1 | unlimited 1.17 |
| String | `string_utf16` | mixed | no-copy | 105,143.9 | metered 1.17 |
| Box / Vec | `box_u64_create_drop` | mixed | no-copy | 160.2 | metered 1.16 |
| BTreeSet | `btree_set_append` | mixed | no-copy | 90,509.6 | unlimited 1.16, concurrent 1.15 |
| Rc / Arc | `arc_unwrap` | mixed | no-copy | 210.2 | concurrent 1.15 |

### Cases whose copy mechanism differs from std

Instruction ratios are not meaningful here. One side calls libc `memcpy` or
`memset` and the other copies with an inlined loop, or the copy volume
differs. These are the first candidates for timing.

| Benchmark | Task | Profile | Implementations | std copy / product copy instructions per call |
| --- | --- | --- | --- | --- |
| Box / Vec | `box_collect_create_drop` | mixed | metered | 56,264.3 / -0.0 (metered) |
| Box / Vec | `box_slice_clone_drop` | mixed | metered | 90,099.5 / 0.0 (metered) |
| Box / Vec | `splice_no_hint` | mixed | unlimited, limited, metered, limited_metered | 238,444.2 / 263,370.9 (unlimited) |
| Box / Vec | `splice_unknown` | mixed | unlimited, limited, metered, limited_metered | 154,649.0 / 177,674.5 (unlimited) |
| Box / Vec | `within` | mixed | unlimited, limited, metered, limited_metered | 101,858.3 / 73,522.6 (unlimited) |
| Arc scenarios | `arc_app_request4096_t1` | mixed | borrowed, owned | 413.0 / 826.0 (borrowed) |
| Arc scenarios | `arc_app_request4096_t4` | mixed | borrowed, owned | 413.0 / 826.0 (borrowed) |
| BTreeMap | `btree_build` | mixed | unlimited, local, concurrent | 11,791.5 / 13,733.6 (unlimited) |
| BTreeMap | `btree_extract` | mixed | unlimited, local, concurrent | 5,276.4 / 4,855.3 (unlimited) |
| BTreeMap | `btree_pop_all` | mixed | unlimited, local, concurrent | 20,605.9 / 18,113.3 (unlimited) |
| BTreeMap | `btree_remove` | mixed | unlimited, local, concurrent | 16,593.6 / 15,100.0 (unlimited) |
| BTreeMap | `btree_retain` | mixed | unlimited, local, concurrent | 5,276.4 / 4,855.3 (unlimited) |
| BTreeSet | `btree_set_build` | mixed | unlimited, local, concurrent | 6,198.0 / 7,236.8 (unlimited) |
| BTreeSet | `btree_set_extract` | mixed | unlimited, local, concurrent | 2,760.2 / 3,386.5 (unlimited) |
| BTreeSet | `btree_set_retain` | mixed | unlimited, local, concurrent | 2,760.2 / 3,386.5 (unlimited) |
| HashMap | `hash_map_clone` | mixed | unlimited, local, concurrent | 12,193.0 / 12,869.6 (unlimited) |
| HashMap | `hash_map_default_new` | mixed | unlimited | 0.0 / 0.0 (unlimited) |
| HashMap | `hash_map_extract` | mixed | unlimited, local, concurrent | 12,193.0 / 12,869.6 (unlimited) |
| HashMap | `hash_map_fixed_clone` | mixed | unlimited, local, concurrent | 12,193.0 / 12,869.6 (unlimited) |
| HashMap | `hash_map_into_iter` | mixed | unlimited, local, concurrent | 12,193.0 / 12,869.5 (unlimited) |
| HashMap | `hash_map_retain` | mixed | unlimited, local, concurrent | 12,193.0 / 12,869.6 (unlimited) |
| HashSet | `hash_set_default_new` | mixed | unlimited, local, concurrent | 0.0 / 0.0 (unlimited) |
| BinaryHeap | `binary_heap_append_large_256` | mixed | unlimited, local, concurrent | 70,924.9 / 22,906.1 (unlimited) |
| BinaryHeap | `binary_heap_append_small_256` | mixed | unlimited, local, concurrent | 70,835.6 / 23,127.4 (unlimited) |
| BinaryHeap | `binary_heap_clone_256` | mixed | unlimited, local, concurrent | 47,873.5 / 0.0 (unlimited) |
| BinaryHeap | `binary_heap_drain_256` | mixed | unlimited, local, concurrent | 47,873.5 / 0.0 (unlimited) |
| BinaryHeap | `binary_heap_into_iter_256` | mixed | unlimited, local, concurrent | 47,873.5 / 0.0 (unlimited) |
| BinaryHeap | `binary_heap_peek_mut_256` | mixed | unlimited, local, concurrent | 134,722.5 / 86,849.0 (unlimited) |
| BinaryHeap | `binary_heap_pop_all_256` | mixed | unlimited, local, concurrent | 158,648.5 / 123,287.3 (unlimited) |
| BinaryHeap | `binary_heap_push_reserved_256` | mixed | concurrent | 15,660.9 / 65,822.8 (concurrent) |
| BinaryHeap | `binary_heap_retain_256` | mixed | unlimited, local, concurrent | 55,493.5 / 7,620.0 (unlimited) |
| BinaryHeap | `binary_heap_sort_256` | mixed | unlimited, local, concurrent | 217,059.9 / 169,186.4 (unlimited) |
| VecDeque | `deque_append_256` | mixed | unlimited, local, concurrent | 95,756.1 / 188,976.5 (unlimited) |
| VecDeque | `deque_append_8` | mixed | unlimited, local, concurrent | 806.5 / 3,353.9 (unlimited) |
| VecDeque | `deque_clone_256` | mixed | unlimited, local, concurrent | 12,540.5 / 105,746.4 (unlimited) |
| VecDeque | `deque_clone_8` | mixed | unlimited, local, concurrent | 0.0 / 2,524.8 (unlimited) |
| VecDeque | `deque_clone_from_256` | mixed | unlimited, local, concurrent | 12,540.5 / 105,752.2 (unlimited) |
| VecDeque | `deque_clone_from_8` | mixed | unlimited, local, concurrent | 0.0 / 2,524.2 (unlimited) |
| VecDeque | `deque_contiguous_256` | mixed | unlimited, local, concurrent | 49,834.2 / 143,040.2 (unlimited) |
| VecDeque | `deque_contiguous_8` | mixed | unlimited, local, concurrent | 19.9 / 2,544.7 (unlimited) |
| VecDeque | `deque_drain_256` | mixed | unlimited, local, concurrent | 37,939.3 / 131,145.3 (unlimited) |
| VecDeque | `deque_drain_8` | mixed | unlimited, local, concurrent | 92.9 / 2,617.7 (unlimited) |
| VecDeque | `deque_extend_8` | mixed | unlimited, local, concurrent | 0.0 / 2,951.9 (unlimited) |
| VecDeque | `deque_fifo_256` | mixed | unlimited, local, concurrent | 50,161.8 / 143,367.8 (unlimited) |
| VecDeque | `deque_fifo_8` | mixed | unlimited, local, concurrent | -0.0 / 2,524.8 (unlimited) |
| VecDeque | `deque_insert_remove_256` | mixed | unlimited, local, concurrent | 105,331.5 / 198,508.2 (unlimited) |
| VecDeque | `deque_insert_remove_8` | mixed | unlimited, local, concurrent | 2,143.1 / 4,668.4 (unlimited) |
| VecDeque | `deque_pop_all_256` | mixed | unlimited, local, concurrent | 25,080.9 / 118,286.9 (unlimited) |
| VecDeque | `deque_pop_all_8` | mixed | unlimited, local, concurrent | 0.0 / 2,524.8 (unlimited) |
| VecDeque | `deque_resize_256` | mixed | unlimited, local, concurrent | 47,554.4 / 140,791.1 (unlimited) |
| VecDeque | `deque_resize_8` | mixed | unlimited, local, concurrent | 740.2 / 3,266.2 (unlimited) |
| VecDeque | `deque_retain_256` | mixed | unlimited, local, concurrent | 23,209.7 / 109,302.8 (unlimited) |
| VecDeque | `deque_retain_8` | mixed | unlimited, local, concurrent | 0.0 / 2,524.8 (unlimited) |
| VecDeque | `deque_rotate_256` | mixed | unlimited, local, concurrent | 12,540.5 / 105,746.4 (unlimited) |
| VecDeque | `deque_rotate_8` | mixed | unlimited, local, concurrent | 0.0 / 2,524.8 (unlimited) |
| VecDeque | `deque_shrink_256` | mixed | unlimited, local, concurrent | 42,395.1 / 138,962.8 (unlimited) |
| VecDeque | `deque_shrink_8` | mixed | unlimited, local, concurrent | 65.7 / 2,707.7 (unlimited) |
| VecDeque | `deque_split_256` | mixed | unlimited, local, concurrent | 64,892.9 / 158,100.2 (unlimited) |
| VecDeque | `deque_split_8` | mixed | unlimited, local, concurrent | 808.8 / 3,337.2 (unlimited) |
| Scratch / slice | `scratch_push_pop_256` | mixed | unlimited, local, concurrent | 25,080.9 / 12,540.5 (unlimited) |
| Scratch / slice | `scratch_resize_256` | mixed | unlimited, local, concurrent | 12,540.5 / 0.0 (unlimited) |

## Assembly observations in this session

### Integer arithmetic

Non-inlined functions, 12 types × 15 operations, compared with core controls
returning `Option`:

| Operations | Pairs | Result |
| --- | ---: | --- |
| `wrap_add/sub/mul` | 36 | Identical |
| `try_add/sub/mul`, `Checked` `+ - *` | 72 | 60 differ only in condition polarity (`Result` Err=1 versus `Option` Some=1); unsigned add uses `add/setb/mov` versus `lea/cmp/setae` with equal count; `u128` subtraction is shorter in the product (14 versus 17) |
| division and remainder, 6 operations | 72 | Mostly longer in the product (`i16`/`i32` `try_wrap_div` are shorter): the error distinguishes zero divisor from `MIN / -1`, and with one 1-byte error the 64-bit `Result` returned through memory |

Inside inlined loops mapping errors to zero (6 types × 10 operations), 59 of 60
loops compile to code identical to the `checked_*` control; `i64` wrapping
remainder is shorter in the product. Commit `506d2eb` gives 64-bit integers a
64-bit `DivisionError64`; the measured non-inlined division table is in the
[arithmetic guide](../arithmetic.md). No division or remainder now returns
through memory where `Option` does not. A remaining 64-bit difference of two to
three instructions is not attributed.

### Vec splice excess staging

Per staged element (zero-lower-hint excess beyond the first growth, u64):

| Version | Instructions per element | Notes |
| --- | ---: | --- |
| std (`collect` into a temporary) | 10 | count-down iterator end; capacity compared from a stack slot |
| `687db7d` (this branch) | 24 | per-element bounds check `cap < len`, iterator state loaded and stored through memory, one stack spill pair |
| `736d479` (verified checkpoint, not on this branch) | 26 | adds two returned-capacity checks and one more stack spill pair; drops the iterator reload |

`736d479`'s returned-capacity checks exist because its Verus controller does not
assume the callback postcondition that Kani proves for `stage_push`. The product
caller discards the controller's returned length and capacity. Allocation calls
are one more than std at lengths 8, 32, 512 and 4096; the product grows the owner
before staging, while std collects the remainder into one temporary. In one in-VM comparison std was 318.2 ns / 20.70 µs (small /
mixed); `687db7d` +8.8% / +32.7%, `736d479` +12.8% / +36.7%.

### Box u64 `clone_from`

Std and the unmetered product execute identical instructions; the metered loop
differs only in its source stride (0x50 versus 0x80 for the 16-byte handle). All
three executed 1,033,090 instructions in the same run. A one-seed VM timing
showed +28% and +26%; loop alignment did not predict the direction (std's inner
loop was the least aligned).

### Vec `grow`

Per pushed element std executes 10 instructions and every product policy 11:
std counts the input down and branches on the subtraction's flags; the product
adds a separate end comparison. The product compares capacity in a register,
std from a stack slot. Code plus allocator instructions are 1.10 times std.
VM timings ranged from +12.8% (vCPU 3) to +47.5% (vCPU 0) for the same binary.

## Local timing guide

Use a dedicated machine. Build first; do not compile while timing.

```sh
rustup toolchain install 1.95.0 --profile minimal
cd tools/throughput-analysis
RUSTUP_TOOLCHAIN=1.95.0 cargo build --locked --release
RUSTUP_TOOLCHAIN=1.95.0 cargo build --locked --release --features hash-map --bin hash_map --target-dir ../../target/verification/tp-hash-map
RUSTUP_TOOLCHAIN=1.95.0 cargo build --locked --release --features hash-set --bin hash_set --target-dir ../../target/verification/tp-hash-set
cd ../..
```

Reduce variation where the platform allows: fix the CPU frequency or disable
turbo, isolate a core and pass it with `--cpus`, and keep other load away.
`perf stat -e cycles,instructions` on the same command separates cycles from
instruction counts.

Time one executable at a time with the standard protocol (three seeds, every
interval at least one second):

```sh
python3 tools/throughput-analysis/compare.py --binary default=$PWD/tools/throughput-analysis/target/release/jzap-throughput-analysis --tasks grow,within,splice_unknown,splice_no_hint --small-tasks clone,extract_all,pop --small-variants default --output target/verification/local-vec
python3 tools/throughput-analysis/compare.py --binary default=$PWD/tools/throughput-analysis/target/release/deque --tasks deque_clone_from_256,deque_fifo_8 --small-tasks "" --output target/verification/local-deque
python3 tools/throughput-analysis/report.py --input target/verification/local-vec
```

Pass any task of an executable with `--tasks` (list them with `<executable> tasks`);
`--small-tasks ""` disables the Vec-only small profile for other executables.
Suggested order: the `copy-differs` table, then comparable cases above 1.15,
then cases whose VM timing disagreed with their instruction counts (Box u64
`clone_from`, Vec `grow`).

Repeat the instruction counts on the local machine for comparison:

```sh
python3 tools/throughput-analysis/instruction_counts.py --jobs 4 --output target/verification/instruction-counts
python3 tools/throughput-analysis/classify_instruction_counts.py target/verification/instruction-counts/instruction-counts.csv target/verification/instruction-counts/classified.csv
```

## Not done

- No timing in this report is a pass or fail; none used a dedicated machine.
- Copy-differs cases were not timed or attributed instruction by instruction.
- Instruction counts used one seed and one input distribution per task.
- The unexplained 0.2-instruction residue between identical-code labels, the
  bimodal process timing in the VM, and the 64-bit division instruction gap
  were not resolved.
