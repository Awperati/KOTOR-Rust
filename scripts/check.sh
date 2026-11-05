#!/bin/sh
# CI-style gate: formatting, lints, tests.
set -e
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# The GPU package is standalone; once its crates are fetched (gpu/Cargo.lock exists) it must build and lint too.
if [ -f gpu/Cargo.lock ]; then
  cargo clippy --manifest-path gpu/Cargo.toml --offline -- -D warnings
fi
