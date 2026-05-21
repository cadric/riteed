#!/usr/bin/env bash
set -euo pipefail

cd app
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test proptest_ --target x86_64-unknown-linux-gnu -- --test-threads=1
