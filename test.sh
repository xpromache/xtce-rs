#!/bin/sh
export RUST_LOG=debug
cargo test binary_leading_size -- --nocapture
