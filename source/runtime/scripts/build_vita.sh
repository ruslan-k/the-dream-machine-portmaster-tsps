#!/usr/bin/env bash
set -euo pipefail
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"
cargo vita build vpk --profile=vita
echo "Output: target/armv7-sony-vita-newlibeabihf/vita/ruffle4consoles.vpk"
