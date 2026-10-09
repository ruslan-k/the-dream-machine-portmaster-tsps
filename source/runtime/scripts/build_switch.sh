#!/usr/bin/env bash
set -euo pipefail

export PKG_CONFIG_DIR=
export PKG_CONFIG_PATH=
export PKG_CONFIG_SYSROOT_DIR=
export PKG_CONFIG_LIBDIR="${DEVKITPRO}/portlibs/switch/lib/pkgconfig"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"

cargo build \
  -Z build-std=core,alloc,std,panic_abort \
  --target targets/aarch64-nintendo-switch.json \
  --profile=switch \
  "$@"

ELF=target/aarch64-nintendo-switch/switch/ruffle4consoles.elf
NACP=target/aarch64-nintendo-switch/switch/ruffle4consoles.nacp
NRO=target/aarch64-nintendo-switch/switch/ruffle4consoles.nro

nacptool --create 'Ruffle' 'ruffle4consoles contributors' '0.1.0' "$NACP"
elf2nro "$ELF" "$NRO" --icon=icon.jpg --nacp="$NACP"

echo "Output: $NRO"
