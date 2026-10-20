#!/usr/bin/env bash
set -euo pipefail
cargo build --profile local "$@"
make install BIN_PATH=target/local/flux-fm
