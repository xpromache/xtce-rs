#!/bin/sh
export RUST_LOG=debug
cargo test test_bogus2 -- --nocapture
