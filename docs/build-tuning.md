# Optional loop alignment

Applications can choose a loop-alignment setting for their own compiler, CPU and
workload. The default build uses the compiler defaults. No alignment setting is
automatically enabled, and choosing one does not change collection APIs or failure
contracts.

| Setting | Command from this checkout |
| --- | --- |
| Compiler default | `cargo build --release` |
| Request 32-byte loop alignment | `cargo --config .cargo/loop-align32.toml build --release` |
| Request 64-byte loop alignment | `cargo --config .cargo/loop-align64.toml build --release` |

The named TOML files are opt-in configuration fragments. Cargo does not load them
automatically: there is no `.cargo/config.toml` enabling these options. Select one
fragment at a time. Omit `--config` to return to the compiler default.

## Consumer projects

Cargo does not inherit this repository's configuration files when jzap is used as
a dependency. Apply the option at the consuming application's workspace root.
Copy the desired fragment into that workspace and use its path with `--config`,
or pass the equivalent flag for a single build:

```sh
RUSTFLAGS='-C llvm-args=-align-loops=32' cargo build --release
# Alternative:
RUSTFLAGS='-C llvm-args=-align-loops=64' cargo build --release
```

If the application already uses RUSTFLAGS, add the selected option to its existing
flags. RUSTFLAGS or CARGO_ENCODED_RUSTFLAGS take precedence over build.rustflags;
setting either can prevent a selected config fragment from taking effect.
Other Cargo configuration files may also contribute flags. Inspect `cargo build -vv` when
checking the effective rustc command.

This is a compiler build setting rather than a Cargo feature. The application
chooses the code generation of its generic collection operations, including instantiated
std collection code. These flags do not rebuild the precompiled standard-library
runtime automatically.

## Scope and tradeoffs

The options were accepted by Rust 1.95 / LLVM 22.1.2 and examined on Linux x86-64
with an AMD EPYC 9V74 guest CPU. They are LLVM backend options, so check support
and generated code when changing toolchain or target. Alignment steers loop
placement; it does not guarantee the fastest execution for every call context.

In the recorded comparison, 32/64-byte loop alignment removed one large retain
regression, while increasing the executable's text section by approximately
1.5%/4.3%. Other comparisons remained inconclusive. The application should choose
based on its workloads; neither setting is declared a universal winner.
See the [source-form and alignment controls](verification/vec/retain-source-controls.md)
for those historical conditions and limitations. The
[current Vec checkpoint](verification/vec-optimization.md) records the renewed
source/guard certificate and the latest timing comparisons. Optional tuning does
not establish universal performance parity.
The [BinaryHeap report](binary-heap-design.md#final-source-follow-up-and-optional-alignment)
also records matched default/32-byte comparisons and the remaining small-extension costs.

The three selections were compiled with the maintained Vec analysis program.
Each binary matched its earlier compiler-default or explicitly flagged build
byte for byte; the configuration files select the intended options.
