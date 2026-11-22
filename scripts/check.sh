#!/usr/bin/env bash
# Every check the repo has: formatting, lints, and the full test suite.
# The pre-push hook runs this; run it yourself before handing work over.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
