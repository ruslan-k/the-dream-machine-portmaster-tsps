#!/usr/bin/env bash
set -euo pipefail

# Requires PS5 homebrew / Prospero SDK or OpenOrbis PS5 fork.
: "${PROSPERO_SDK:=/opt/prospero-sdk}"

export PKG_CONFIG_PATH="${PROSPERO_SDK}/target/lib/pkgconfig"
export CC="${PROSPERO_SDK}/bin/prospero-clang"
export CXX="${PROSPERO_SDK}/bin/prospero-clang++"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16 --cfg getrandom_backend=\"custom\""

cargo build \
  -Z build-std=core,alloc,std,panic_abort \
  --target targets/x86_64-sie-ps5.json \
  --profile=ps4 \
  "$@"

ELF=target/x86_64-sie-ps5/ps4/ruffle4consoles.elf
echo "Output ELF: ${ELF}"
echo "Package with create-fself / prospero pkg tools."
