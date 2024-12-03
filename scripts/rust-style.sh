#!/usr/bin/env bash

FMT_OPTS=${FMT_OPTS:-"--config imports_granularity=Crate"}

source ${BASH_SOURCE%/*}/../spdk-rs/scripts/rust-linter-env.sh
$CARGO fmt --all -- $FMT_OPTS
