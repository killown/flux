#!/bin/bash

set -e

echo ">>> Running Rust unit and integration tests..."

cargo fmt --check                     # Fail if code isn't formatted
cargo clippy -- -D warnings           # Fail if there are lints
RUST_BACKTRACE=1 cargo test --verbose # Fail if logic is broken

echo ">>> All tests passed successfully!"
