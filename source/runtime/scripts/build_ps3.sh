#!/usr/bin/env bash
set -euo pipefail

# Requires ps3toolchain / PSL1GHT:
# https://github.com/ps3dev/ps3toolchain
: "${PSL1GHT:=/usr/local/ps3dev}"
export PATH="${PSL1GHT}/ppu/bin:${PATH}"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"

cargo build \
  -Z build-std=core,alloc,std,panic_abort \
  --target targets/powerpc64-sony-ps3.json \
  --profile=ps3 \
  "$@"

ELF=target/powerpc64-sony-ps3/ps3/ruffle4consoles.elf
SELF=target/powerpc64-sony-ps3/ps3/ruffle4consoles.self

# Sign as fSELF (requires make_self_npdrm / scetool)
if command -v make_self &>/dev/null; then
  make_self "${ELF}" "${SELF}"
  echo "Output: ${SELF}"
else
  echo "Warning: make_self not found. Raw ELF at: ${ELF}"
fi

echo ""
echo "Package as PS3 PKG and install to:"
echo "  /dev_hdd0/game/RUFL00001/USRDIR/"
