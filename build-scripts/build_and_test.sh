#!/bin/bash

set -x
set -e

cargo fmt --check --all
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-features --all-targets --locked
cargo test --all-features --all-targets --locked
