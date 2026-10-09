#!/usr/bin/env bash
set -euo pipefail

# Requires devkitPro with devkitPPC + wut
: "${DEVKITPRO:=/opt/devkitpro}"
export PATH="${DEVKITPRO}/tools/bin:${DEVKITPRO}/devkitPPC/bin:${PATH}"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"

cargo build \
  -Z build-std=core,alloc,std,panic_abort \
  --target targets/powerpc-nintendo-wiiu.json \
  --profile=wiiu \
  "$@"

ELF=target/powerpc-nintendo-wiiu/wiiu/ruffle4consoles.elf
RPX=target/powerpc-nintendo-wiiu/wiiu/ruffle4consoles.rpx

# wut provides wut-tools for ELF → RPX conversion
if command -v elf2rpl &>/dev/null; then
  elf2rpl "${ELF}" "${RPX}"
  echo "Output: ${RPX}"
else
  echo "Warning: elf2rpl not found; raw ELF at ${ELF}"
fi

echo ""
echo "Deploy to sd:/wiiu/apps/ruffle/ as:"
echo "  ruffle4consoles.rpx"
echo "  meta.xml   (see platform/wiiu/meta.xml)"
echo "  icon.png   (see platform/wiiu/icon.png)"
