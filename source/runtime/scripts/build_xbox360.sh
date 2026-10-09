#!/usr/bin/env bash
set -euo pipefail

# Requires libxenon toolchain: https://github.com/Free60Project/libxenon
: "${LIBXENON_DIR:=/usr/local/xenon}"
export PATH="${LIBXENON_DIR}/bin:${PATH}"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"

cargo build \
  -Z build-std=core,alloc,std,panic_abort \
  --target targets/powerpc64-microsoft-xbox360.json \
  --profile=release \
  "$@"

ELF=target/powerpc64-microsoft-xbox360/release/ruffle4consoles.elf
ELF32=target/powerpc64-microsoft-xbox360/release/ruffle4consoles.elf32

# libxenon uses 32-bit ELF output from a PPC64 toolchain
if command -v xenon-objcopy &>/dev/null; then
  xenon-objcopy -O elf32-powerpc "${ELF}" "${ELF32}"
  echo "Output: ${ELF32}"
else
  echo "Output: ${ELF} (copy to JTAG/RGH HDD as Hdd:\\ruffle\\ruffle.elf)"
fi
