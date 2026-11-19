# Support status

This page describes the APIs and verification scope of `0.1.0-beta.1`. Public
APIs can change during the beta. Box and Vec have reached their implementation
checkpoints. The [remaining implementation](#remaining-implementation) and
[measurement and verification follow-up](#measurement-and-verification-follow-up)
tables record outstanding specification work and measured performance limits.

| Area | Implemented scope | Status |
| --- | --- | --- |
| Errors and layouts | Allocation-free errors, rejected input ownership, array layout checks | Foundation checks passed |
| `Checked<T>` / `TryArithmetic` / `WrappingArithmetic` | Fallible `+ - * / %` operators and explicit checked/wrapping methods for all 12 primitive integer types; zero divisors return errors | 11 full-input arithmetic Kani harnesses; 36 primitive and three generic Checked direct Verus add/sub/mul contracts; [API and ABI evidence](arithmetic.md). Internal use and remaining obligations: [checked metadata](checked-metadata.md) |
| Memory budgets | Borrowed and owning local/concurrent policies; reservation, commitment, cancellation, and release | Implemented; owning policies remove stored budget lifetimes using fallible Rc/Arc. High-rate shared-domain allocation exceeds the performance criterion; see [workflow results](verification/arc-scenarios.md), [guide](budget.md) and [verification](verification/owned-budgets.md) |
| Wrapping values at memory boundaries | External values are range checked; private metadata wrapping has a no-overflow obligation | Common modular calls and collection-specific numeric transitions are bound to contracts; native/32-bit and source-connected guard checks pass. Ownership/provenance checks use the agreed compiler/Miri/sanitizer boundary; see the [Scratch/slice source and link checkpoint](scratch-and-slice.md#fresh-sourcelink-composition) |
| `TryClone` / `TryCloneIn` | Scalars, references, Option/Result, tuples of arity 1–12, arrays, Reverse, Box, Vec, String, shared strong/weak handles, ordered/hashed maps and sets, BinaryHeap, VecDeque, ScratchBuf, shallow checked shared slices, and target-budget propagation | Implemented and tested; derive support is not implemented |
| `TryDefault` | Primitive defaults, empty borrows, Option, tuples, arrays, Box, String, shared owners, ordered/hashed containers, BinaryHeap, VecDeque and ScratchBuf | Implemented and tested |
| Fallible iteration | Open iterator/conversion traits, double-ended/exact-size/fused counterparts, Box/Vec propagation, consumer link checks | Initial trait foundation; full adapter catalogue is not implemented; see [iteration contracts](iteration.md) |
| `InitSlot` | Recorded initialization of individual values, arrays, and named fields | Implemented; partial cleanup and small-stack tests passed |
| Box | Sized and dynamically sized ownership, slices, strings, trait objects, Pin, collection, calls, and explicit standard-owner conversions | Provisionally closed; see the API matrix |
| OS strings and paths | `OsStr` / `Path` ownership under the `std` feature | Box and shared-owner integration tests run with all features on Linux, macOS, and Windows in [CI](verification/README.md#continuous-integration) |
| Performance | Repeated equal-work std comparisons and separate allocation measurements | ExtractIf meets the adjusted std +15% criterion in all 12 measured default-build conditions. Splice batch staging improves large zero-hint replacements. Three small-input Splice conditions exceed the criterion, and one is unresolved. See [ExtractIf](verification/vec-extract-inline.md) and [Splice](verification/vec-splice-batch.md). |
| Linked exit paths | Concrete Box/Vec/String/Rc/Arc/BTreeMap/BTreeSet/HashMap/HashSet/BinaryHeap/VecDeque/Scratch/checked-slice and owning-budget panic/exit checks with deliberate negative controls | Fresh release/LTO/overflow-checked/debug source-link composition has zero open conditions or unproved linked edges. Retained core guards are bound to their caller proofs; see [scope and reproduction](verification/vec-splice-batch.md#reproduction) |
| Rc / Arc / Weak | Stable Rust 1.95 functional counterparts, fallible counts/allocation/COW, dynamically sized values, conversions, pinning and byte budgets | Implemented; source/guard, memory and concurrency checks pass. Arc performance depends on contention and allocation; [workflow evaluation](verification/arc-scenarios.md) and [foundation results](verification/shared.md) |
| BTreeMap and borrowed slice views | Stable API counterparts, Entry operations, byte budgets and caller-supplied node lookup/traversal | Implemented; verification passes and reported std timing/allocation means meet the observed +15% criteria; [results and scope](verification/btree.md) |
| BTreeSet | Explicit counterparts for all 27 pinned stable inherent names, fallible clone/owned algebra, sorted borrowing/owning iteration and byte policies | Functionality, memory and source/guard checks complete. Of 32 unmetered timing conditions, 30 means are within +15%; two short intersection conditions add 3.1/5.8 ns. [Full results](btree-set-design.md) |
| HashMap | Stable map/Entry counterparts, explicit entropy/allocation failures, keyed hashing, cloning, iteration and all byte policies | Functionality, memory and source/guard checks pass. After removing redundant Copy-clone initialization, all eight focused unmetered timing means are within +15% of std; small clone adds 0.16 ns. The guide also retains the broader baseline campaign. [Guide, conditions and results](hash-map-design.md) |
| HashSet | HashMap-backed set, explicit errors/policies, borrowed and owned algebra, replacement and iteration | Functionality, memory and fresh four-profile source/guard checks pass. All 33 unmetered timing means are within the observed std +15% criterion, retaining the initial and fixed supplemental intersection samples. Requested held/peak bytes match std. [Contracts, results and scope](hash-set-design.md) |
| Independent Vec | All 46 Rust 1.95 stable inherent methods have explicit counterparts; trait adapters, element limits, byte budgets, and fallible range editing | Implementation provisionally closed; current safety/guard checks pass within documented boundaries. Performance remains conditional; see the [checkpoint](verification/vec-optimization.md) and API mapping |
| Independent String | All 38 Rust 1.95 stable inherent methods have explicit counterparts; byte limits, budgets, UTF-8 editing, encoding conversions and formatting | Implemented; verification and performance results are recorded in the [String report](verification/string.md) |
| BinaryHeap | Vec-backed max heap; 23 distinct stable inherent names, fallible cloning/growth, mutable peek, allocation policies and iteration | Functionality, allocation and fresh four-profile source/link checks pass. Same-ledger append preserves policy handles and admission. Unmetered observed +15% coverage is 53/60 default and 59/60 aligned; small extend retains 8–9 ns. [Contracts, conditions and results](binary-heap-design.md) |
| VecDeque | Independent ring storage; 54 pinned stable inherent names, failure-returning edits, budgets/limits, clone and iteration | Stable API and allocation checks pass. All 39 timing conditions completed; unmetered means are at most std +15% in 38/39. Iteration placement is reproduced and removed in the matched 32-byte-option comparison. Fresh four-profile source/link composition passed with zero open conditions and zero unproved linked paths. [Contracts and verification](vec-deque-design.md) |
| Fixed scratch | Vec-backed `ScratchBuf`, borrowed typed/initialized-byte storage, fixed capacity, slot initialization and rollback | Domain and allocation checks pass. All 20 two-layout timing means are within std +7.2%; one 256-byte push/pop layout has +54.8% and its alternate −30.7%. Fresh four-profile source/link composition passes with zero open conditions and zero unproved linked paths. [Contracts and results](scratch-and-slice.md) |
| Checked slices | Shared/mutable access, ranges, disjoint splitting, matching-length copy/swap, overlap copy and fallible iteration | Core-only, explicit raw-slice interoperability; direct numeric/actual Rust, compiler and memory checks pass. [Guide](scratch-and-slice.md) |

The [API and verification matrix](verification/box-completion/README.md) gives
per-family status and evidence. For Box on the measured 64-bit target, sized
handles are 8 bytes without a budget and 16 bytes with a budget; slice/str handles are 16 and
24 bytes. The additional budget pointer is a documented memory cost, not a speed
exemption.

Library-controlled allocation, budget, layout, bounds, and initialization failures
are returned explicitly. User callbacks, destructors, and allocators retain their
own execution contracts. The library cannot convert arbitrary process termination
or nontermination in external code into an error return.

Earlier enhanced Box Kani probes recorded unsupported instrumentation, compiler
errors and compilation timeouts. Their historical outcomes are listed in the
[proof boundaries](verification/memory-proof/README.md). Current source/link
composition and the separate memory suites are recorded in the
[Splice batch verification checkpoint](verification/vec-splice-batch.md#verification).

## Vec element limit

Vec construction optionally accepts a maximum element count. The limit is
separate from current allocation capacity and the byte budget. Selecting a limit
does not preallocate that many elements. Growth up to the limit is allowed;
requests exceeding it return a capacity error. Removing elements or clearing the
vector makes room for later insertions under the same limit.

Implemented growth paths enforce this contract, including reservation, insertion,
extension, resizing, and owner conversion. Geometric growth clamps spare capacity
to the limit instead of rejecting a valid request because doubling overshoots. Zero-sized elements still
count toward the limit. Single-element rejection preserves the vector and returns
the input; bulk operations must state preservation and iterator-consumption
semantics individually. Ordinary construction without a limit remains available.

See the [Vec guide](vec.md) for APIs and the stable standard-library mapping, and
the [Vec report](verification/vec/README.md) for verification and measurement scope.

## Remaining implementation

Inventory reviewed against implementation checkpoint
`04a247c` and the [specification](jzap_std_specification_final.md), sections 3 and 10.
The collection inventory now includes Vec, String, Box, Rc/Arc/Weak,
BTreeMap/BTreeSet, HashMap/HashSet, BinaryHeap, VecDeque, fixed scratch and checked
shared/mutable slice views. BTreeMap/BTreeSet
provide the ordered map/set implementation selected for the specification's
`SortedMap`/`SortedSet` requirement. Construction-time Vec/String limits and
fallible formatting are also implemented.

| Area | Existing foundation | Remaining implementation |
| --- | --- | --- |
| Fallible iterator adapters | Open iterator traits, nth/fold/for-each/take helpers, std/result bridges and collection consumers | Native fallible map/filter/zip and the rest of the adapter catalogue; [current contracts](iteration.md) |
| Clone derive | Manual `TryClone`/`TryCloneIn` implementations and field initialization | The separate `jzap-derive` procedural-macro package, including generics, renamed dependencies and target-budget propagation |
| Numeric operations and conversions | Checked/wrapping add, subtract, multiply, divide and remainder for 12 integer types | Checked absolute value/negation, lossless shifts, integer-width/signedness conversion, and integer/float conversion with explicit precision and special-value rules |
| System runtime | Rc/Arc/Weak, OS string/path ownership, and fallible in-memory formatting | Fallible thread creation, Mutex/RwLock, checked time/duration, general I/O and print/eprint; resource errors and partial-progress contracts |
| Consumer policy tooling | Library Clippy rules, arithmetic enforcement, source inventories, compile-fail checks and consumer link fixtures | Packaging and applying the specification's `jzap-policy` checks to consuming projects |

Scratch/slice reuse the allocation and initialization foundations in the basic
ownership stage. Consumer module migration is a separate later stage; this
inventory covers the library checkout. `RefCell`, Once/Lazy and channels are listed in section 11 as future
scope decisions.

The budget lifetime work and reallocation review are closed implementation items:
owning local/concurrent policies are available; the default reservation contract
retains old occupied bytes while reserving the full new allocation. See the
[budget guide](budget.md).

## Measurement and verification follow-up

These entries record existing measurements and verification scope. Box, Vec and
BinaryHeap retain their user-directed implementation checkpoints.

| Area | Recorded result / remaining work | Evidence |
| --- | --- | --- |
| Box measurements | Broader repeated measurements across conditions remain pending under its provisional close | [Box checkpoint](verification/box-completion/README.md#performance-checkpoint) |
| Vec code generation, November 4 | Two of 28 targeted default-build means exceed std +15%; the recorded 32-byte-option comparison puts all 28 within that observed range | [Vec comparisons](verification/vec-optimization.md#policy-and-alignment-comparisons) |
| Vec range edits, November 18 | ExtractIf meets the adjusted +15% criterion in all 12 measured conditions. For small Splice inputs, deletion, shorter replacement and exact-length replacement exceed +15%. Longer replacement with a known hint has an unresolved margin. | [ExtractIf](verification/vec-extract-inline.md), [Splice](verification/vec-splice-batch.md#repeated-timing), issue #4 |
| BTreeSet short intersections | Two of 32 unmetered conditions exceed +15%, adding 5.784 and 3.085 ns/call | [BTreeSet results](btree-set-design.md#performance-and-allocation) |
| BinaryHeap code generation | Default means meet the +15% observation in 53/60 conditions; optional alignment in 59/60. Small extend retains about 8–9 ns | [Heap results](binary-heap-design.md) |
| VecDeque iteration placement | Default means meet the +15% observation in 38/39 conditions. The u64 iteration gap is +23.3%; its matched 32-byte-option comparison measures −0.3% | [Deque results](vec-deque-design.md#iteration-alignment-comparison) |
| Scratch large-value copy layout | The 256-byte borrowed push/pop layouts measure +54.8% and −30.7%, with +3.4% combined. A fixed-address copy diagnostic reproduces the temporary-address phase effect | [Scratch results](scratch-and-slice.md#final-timing-and-allocation-matrix) |
| Optional shared byte ledger | High-rate shared-domain allocation carries ledger operations absent from std; the cost is documented for policy selection | [Arc workflows](verification/arc-scenarios.md) |
| Direct-source proof migration | Original arithmetic, initialization, budget, state, layout, Vec numeric/control, String cursor, shared numeric and BTree kernels have direct source checks. ExtractIf and splice-fill control/content/caller checks and executed memory checks pass. Repeated std time and allocation comparisons are recorded; some measured time conditions remain slower. Remaining Vec/Buffer owner loops, pinned core and domain/global projections are in progress. | [Migration groups and partial reproduction](direct-source-migration.md) |
| Platform execution | Current CI targets Linux, macOS, and Windows. Wasm/thumb builds and i686 Miri cover the configurations identified in the individual reports | [CI and local verification targets](verification/README.md#continuous-integration) |

The four-profile source/link certificate for `42afdab` passed with zero open
source obligations and zero unproved link edges. The run includes direct
contracts and reviewed projections. See the
[batch verification and reproduction commands](verification/vec-splice-batch.md).

The [migration guide](direct-source-migration.md#contract-groups) tracks the
remaining projections. The [handoff](next-session.md) preserves earlier checkpoints.
