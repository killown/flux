#!/bin/bash

set -e

echo ">>> Running Rust unit and integration tests..."

cargo fmt --check           # Fail if code isn't formatted
cargo clippy -- -D warnings # Fail if there are lints
# Use --test-threads=1 to prevent race conditions when mocking environment variables
RUST_BACKTRACE=1 cargo test --verbose -- --test-threads=1 # Fail if logic is broken

echo ">>> All tests passed successfully!"
