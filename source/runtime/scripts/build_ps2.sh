#!/usr/bin/env bash
set -euo pipefail

# Requires ps2dev toolchain: https://github.com/ps2dev/ps2toolchain
: "${PS2DEV:=/usr/local/ps2dev}"
: "${PS2SDK:=${PS2DEV}/ps2sdk}"
export PATH="${PS2DEV}/ee/bin:${PS2DEV}/tools/bin:${PATH}"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"

cargo build \
  -Z build-std=core,alloc,std,panic_abort \
  --target targets/mipsel-sony-ps2.json \
  --profile=psp \
  "$@"

ELF=target/mipsel-sony-ps2/psp/ruffle4consoles.elf

echo "Output: ${ELF}"
echo ""
echo "Deploy to mc0:/ruffle/ruffle4consoles.elf"
echo "Launch via uLaunchELF or wLE."
