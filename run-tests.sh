#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
cargo fmt -- --check
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
