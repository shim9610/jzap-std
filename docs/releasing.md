# Publishing a beta

The Cargo package is `jzap-std`, the Rust import is `jzap`, and the release
candidate is `0.1.0-beta.1`. The package includes source, guides, licenses,
examples, and tests with their fuzz replay inputs. Run the formal verification
and benchmark tooling from the corresponding Git checkout.
[Beta support status](support.md)

## Check the release candidate

Run these commands with Rust 1.95.0 from a clean checkout of the release
candidate. The dry run builds the packaged sources.

```sh
cargo +1.95.0 package --locked --list
cargo +1.95.0 publish --dry-run --locked
cargo +1.95.0 test --manifest-path target/package/jzap-std-0.1.0-beta.1/Cargo.toml --locked --all-features
cargo +1.95.0 check --manifest-path target/package/jzap-std-0.1.0-beta.1/Cargo.toml --locked --no-default-features
```

Record the candidate commit, package checksum, CI results including
`Full certificate`, and the status of the macOS benchmark campaign. When
writing the release notes, use the measured workload and platform scope.
[macOS benchmark prompt](macos-benchmark-prompt.md)

## Publish the approved candidate

Publish from the reviewed commit after confirming the crate name and version in
the registry and authenticating with an account authorized to publish the crate.
Run `cargo +1.95.0 publish --locked` when the candidate is approved. Verify
the published version from a fresh consumer project with
`jzap-std = "=0.1.0-beta.1"`, then tag the published commit
`v0.1.0-beta.1`. Use a new version for any subsequent package revision.
