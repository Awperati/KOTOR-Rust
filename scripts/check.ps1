# CI-style gate: formatting, lints, tests. Run before every commit.
# Tests that read the game skip themselves when the install is missing; set KOTOR_DIR to override the path.
$ErrorActionPreference = 'Stop'
cargo fmt --all -- --check
if ($LASTEXITCODE) { exit 1 }
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE) { exit 1 }
cargo test --workspace
if ($LASTEXITCODE) { exit 1 }

# The GPU package is standalone; once its crates have been fetched (gpu/Cargo.lock exists) it must build and lint too.
if (Test-Path gpu\Cargo.lock) {
    cargo clippy --manifest-path gpu\Cargo.toml --offline -- -D warnings
    if ($LASTEXITCODE) { exit 1 }
}
