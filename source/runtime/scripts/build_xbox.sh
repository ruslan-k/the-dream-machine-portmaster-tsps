#!/usr/bin/env bash
set -euo pipefail

# Requires nxdk: https://github.com/XboxDev/nxdk
: "${NXDK_DIR:=/opt/nxdk}"
export PATH="${NXDK_DIR}/bin:${PATH}"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"

# nxdk uses an i686 MSVC-style environment via clang-cl
export CC="${NXDK_DIR}/bin/clang-cl"
export CXX="${NXDK_DIR}/bin/clang-cl"

cargo build \
  -Z build-std=core,alloc,std,panic_abort \
  --target targets/i686-microsoft-xbox.json \
  --profile=release \
  "$@"

EXE=target/i686-microsoft-xbox/release/ruffle4consoles.exe
XBE=target/i686-microsoft-xbox/release/default.xbe

if command -v cxbe &>/dev/null; then
  cxbe \
    /TITLE:"Ruffle" \
    /TITLEID:"RU-001" \
    /XBEFILENAME:"${XBE}" \
    "${EXE}"
  echo "Output: ${XBE}"
else
  echo "Warning: cxbe not found. Raw EXE at: ${EXE}"
fi
