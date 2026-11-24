---
name: hot-path-review
description: Check a jzap hot path for the performance problems this repository keeps meeting, before writing or changing it and before claiming a speedup. Use when adding or optimizing lookup, insertion, growth, comparison or iteration code, when a benchmark regresses, when an enum or result type used on a hot path changes, or when a Kani harness fails or runs out of memory after a code-shape change.
---

# Hot-path review

Most performance problems in this repository had one of six physical causes,
found from machine code and counters rather than from the source. Check each
of them while designing a change, then confirm with the tools below. Never
explain a timing difference with a cause that was not observed in the
executables that were measured (`AGENTS.md`).

## Rules that always apply

- Benchmark a change before keeping it, against the code it replaces.
  Reject it if any measured condition is slower and the difference is
  confirmed, unless the user decides otherwise.
- Do not compile, prove or run another benchmark while a timed interval
  runs. Parsing finished profiles is fine.
- Every timed interval lasts at least one second; use several seeds,
  alternate the build order by seed, and keep unchanged methods as controls.
- Use the pinned toolchain: `RUSTUP_TOOLCHAIN=1.95.0`.
- Pin the call counts. Benchmarks that size their batches from a pilot run
  choose different counts per build; set the batch counts explicitly (for
  `bplus_partial`, `BPLUS_PARTIAL_BATCHES_<family>_<size>`) so both builds
  run the same calls in each seed. `paired_ab.py` refuses unequal counts.
- Raw outputs go under `target/verification/`.

## The six recurring causes

| # | Cause | What it looked like here | How to check |
| --- | --- | --- | --- |
| 1 | A value that belongs in a register goes through memory | `&mut self` passed to an out-of-line growth kept `len` on the stack each push; a `Result` from an out-of-line `storage::reallocate` and the `FixedTree` routing result made stack round trips; `retain` reloaded its threshold through a `&mut` closure capture | `hot_instructions.py`: instructions with a stack operand per operation; `codegen_diff.py --show` for new `(%rsp)` loads and stores |
| 2 | The same work is done twice | zero-length `memmove` calls; the paired `dedup` sweep's extra branches; one more sequential write in `FixedTree::try_new` (+9.3% instructions); the partial-key pass comparing the inline prefix and then the full key of the same pair | `hot_instructions.py`: executed `call` counts (e.g. `memcmp` per operation) and loop-body execution counts against the expected work |
| 3 | A type's layout changes unrelated code | adding variants to `LookupStep` narrowed the tag store (`movq` to `movb`) and changed register allocation of every complete-key lookup (+5.3% / +7.3% instructions); a returned tuple made a Kani harness exceed 57 GB | `codegen_diff.py` on every benchmark executable that uses the type; prefer a separate type over growing a shared hot-path enum |
| 4 | Placement, not instructions | stack alignment sharing a 64-byte line with a benchmark temporary (22%); a hot path spread over distant cache lines (uop-cache misses); identical-code controls moving ±30% between executables | identical `codegen_diff.py` output plus a timing difference; `compare.py --stack-offsets`; `prepare_stack_layout.py`; loop alignment in `docs/build-tuning.md` |
| 5 | Branches against conditional selection | width-eight branch search took two data-dependent branches per level; conditional selection was faster on Intel but the result depends on the CPU | executed branch instructions in `hot_instructions.py`; time on each CPU class the claim covers |
| 6 | Reading memory the decision does not need | comparing attached full keys first touched out-of-line key bytes that the inline prefix already decided: L1 data misses 86 to 211 per lookup, distinct prefixes up to 2.29x slower | `hot_instructions.py` with `--cache-sim=yes`: D1/LL misses and data reads per operation |

A change that fixes one cause often introduces another. Replacing the
prefix `memcmp` with word comparisons removed 193 calls per lookup, but the
larger `PrefixKey::cmp_stored` was then called out of line about 100 times
per lookup and doubled the stores (cause 1); `#[inline(always)]` on the
comparison and a fixed-size path for whole prefixes recovered it. Re-check
the whole table after every step.

## Workflow

1. **Design.** For the new or changed path, write down the expected work per
   operation (comparisons, calls, memory reads) and check the table above.
   Keep shared hot-path types unchanged where a separate type will do.
2. **Correctness first.** Tests, then the relevant Kani and Miri checks, so
   that a timing never measures a broken form.
3. **Machine code.** Build the previous and the new form with the same
   flags and v0 mangling, and compare every benchmark executable:

   ```sh
   export RUSTUP_TOOLCHAIN=1.95.0 RUSTFLAGS="-C symbol-mangling-version=v0"
   cargo build --release --manifest-path tools/throughput-analysis/Cargo.toml \
     --target-dir target/verification/review/new --bin BENCH
   python3 tools/perf-audit/codegen_diff.py BASE_BINARY NEW_BINARY --match 'jzap|BENCH'
   python3 tools/perf-audit/codegen_diff.py BASE_BINARY NEW_BINARY --show 'FUNCTION'
   ```

   Build the previous form from a `git archive` of its commit under
   `target/verification/`, not from a worktree.
4. **Executed instructions.** Count only the timed region, with the cache
   simulator when memory access may matter:

   ```sh
   valgrind --tool=callgrind --dump-instr=yes --cache-sim=yes \
     --toggle-collect='BENCH::measure' --callgrind-out-file=OUT BINARY SEED
   python3 tools/perf-audit/hot_instructions.py OUT BINARY 'FUNCTION_REGEX' \
     --ops TIMED_OPERATIONS --compare OUT_BASE BINARY_BASE
   ```

   Benchmarks that refuse short intervals need an explicit counting mode
   (for example `BPLUS_PARTIAL_INSTRUCTIONS=1` in `bplus_partial`); never mix
   counting runs into timing records. `instruction_counts.py` and
   `classify_instruction_counts.py` cover the collection benchmarks.
5. **Timing.** Paired runs, alternating order, several seeds, controls:

   ```sh
   python3 tools/perf-audit/paired_ab.py --base base-seed*.csv --new new-seed*.csv \
     --controls complete,std
   ```

   For collection workloads use `tools/throughput-analysis/compare.py` with
   `--stack-offsets 0,16,32,48` and `report.py`. If a control moves as much
   as the change, report the change as unresolved rather than as a gain.
6. **Record.** In the English guide: the conditions, the tables, the
   physical cause that was observed and how, and what was rejected.

## Verification pitfalls after a code-shape change

- **CBMC memory.** Split a harness into units that compose through the
  lookup state, fix sizes per harness so every loop has a concrete bound,
  and try `--solver kissat` (it kept the partial-key harnesses near 4 GB
  where CaDiCaL needed 13 GB). Running out of memory is a failure, never a
  pass.
- **Object bits.** "too many addressed objects" needs
  `--cbmc-args --object-bits 12`; it is a tool limit, not a property result.
- **False counterexamples.** Three kinds of Kani counterexample here
  contradicted their own paths: `get_unchecked` chains of width one,
  indexing an array with a symbolic index, and harnesses consuming a
  `Filter` iterator. Before
  calling a failure a defect or a false counterexample, replay it:

  ```sh
  python3 scripts/kani_playback.py --harness PATH::TO::HARNESS \
    --file src/path/to/file.rs --kani-arg=--solver=kissat \
    --kani-arg=-Z --kani-arg=unstable-options \
    --kani-arg=--cbmc-args --kani-arg=--object-bits --kani-arg=12
  ```

  A playback that passes natively shows the counterexample does not
  reproduce. Then select values with concrete loops instead of symbolic
  indexes or iterator consumption in the harness, keep a native exhaustive
  test of the same property, and record the finding in the guide.
- **Certificate bindings.** A changed product file needs its hash updated
  in the binding scripts (for example `scripts/bplus_bindings.py`), and a
  new certificate step must be listed wherever steps are named, including
  `scripts/certificate_artifacts.py`. Run `python3 scripts/test_verify.py`.

## Reporting

Report to the user in Korean, with tables, separating what was measured,
what was observed in machine code or counters, and what remains a
hypothesis. Repository documents, comments and commit messages stay in
English.
