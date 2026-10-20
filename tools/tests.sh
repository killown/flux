#!/bin/bash

set -e

echo ">>> Running Rust unit and integration tests..."

cargo fmt --check           # Fail if code isn't formatted
cargo clippy -- -D warnings # Fail if there are lints

# Capture test list output to count the tests
TEST_COUNT=$(cargo test --tests -- --list | grep ': test$' | wc -l | tr -d ' ')

RUST_BACKTRACE=1 cargo test --verbose --tests -- --test-threads=1 # Fail if logic is broken

echo ">>> All $TEST_COUNT tests passed successfully!"
