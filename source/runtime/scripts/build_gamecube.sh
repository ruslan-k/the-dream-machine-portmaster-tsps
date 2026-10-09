#!/usr/bin/env bash
set -euo pipefail

# Requires devkitPro with devkitPPC + libogc
: "${DEVKITPRO:=/opt/devkitpro}"
export PATH="${DEVKITPRO}/tools/bin:${DEVKITPRO}/devkitPPC/bin:${PATH}"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"

cargo build \
  -Z build-std=core,alloc,std,panic_abort \
  --target targets/powerpc-nintendo-gamecube.json \
  --profile=wii \
  "$@"

ELF=target/powerpc-nintendo-gamecube/wii/ruffle4consoles.elf
DOL=target/powerpc-nintendo-gamecube/wii/ruffle4consoles.dol

elf2dol "${ELF}" "${DOL}"
echo "Output: ${DOL}"
echo ""
echo "Deploy as a GCI homebrew via SD Gecko, or run with Dolphin."
