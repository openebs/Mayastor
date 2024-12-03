#!/usr/bin/env bash

source ${BASH_SOURCE%/*}/../spdk-rs/scripts/rust-linter-env.sh
$CARGO clippy --all --all-targets --features=io-engine-testing -- -D warnings \
    -A clippy::result-large-err
