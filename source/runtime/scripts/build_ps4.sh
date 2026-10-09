#!/usr/bin/env bash
set -euo pipefail

# Requires OpenOrbis toolchain: https://github.com/OpenOrbis/OpenOrbis-PS4-Toolchain
: "${OPENORBIS:=/opt/openorbis}"

export PKG_CONFIG_DIR=
export PKG_CONFIG_PATH="${OPENORBIS}/target/lib/pkgconfig"
export PKG_CONFIG_SYSROOT_DIR="${OPENORBIS}"
export CC="${OPENORBIS}/bin/orbis-clang"
export CXX="${OPENORBIS}/bin/orbis-clang++"
export LD="${OPENORBIS}/bin/orbis-ld"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16 --cfg getrandom_backend=\"custom\""

cargo build \
  -Z build-std=core,alloc,std,panic_abort \
  --target targets/x86_64-scei-ps4.json \
  --profile=ps4 \
  "$@"

ELF=target/x86_64-scei-ps4/ps4/ruffle4consoles.elf
PKG_DIR=target/ps4_pkg

mkdir -p "${PKG_DIR}/sce_sys"
mkdir -p "${PKG_DIR}/sce_module"
cp "${ELF}" "${PKG_DIR}/eboot.bin"

# Package (requires orbis-pub-cmd or make_fself)
if command -v create-fself &>/dev/null; then
  create-fself -in "${PKG_DIR}/eboot.bin" -out "${PKG_DIR}/eboot.oelf" \
    --paid 0x3800000000000011 --ptype fake
  echo "Output: ${PKG_DIR}/"
else
  echo "Warning: create-fself not found. Raw ELF at: ${ELF}"
fi
