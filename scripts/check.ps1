# CI-style gate: formatting, lints, tests. Run before every commit.
# Tests that read the game skip themselves when the install is missing; set KOTOR_DIR to override the path.
$ErrorActionPreference = 'Stop'
cargo fmt --all -- --check
if ($LASTEXITCODE) { exit 1 }
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE) { exit 1 }
cargo test --workspace
if ($LASTEXITCODE) { exit 1 }
