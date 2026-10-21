#!/usr/bin/env bash
set -euo pipefail
cargo build --profile local "$@"
cargo run --profile local --bin gen-docs
make install BIN_PATH=target/local/flux-fm
