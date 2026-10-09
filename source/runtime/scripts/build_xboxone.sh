#!/usr/bin/env bash
set -euo pipefail

# Xbox One uses the standard x86_64-pc-windows-msvc target (UWP profile) or
# the GDK (Game Development Kit). Build as a regular Windows x64 binary
# targeting the UWP API surface, then package with makeappx.
#
# Requires: MSVC toolchain (Windows) or cross-compilation via Wine + MSVC headers.

export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"

cargo build \
  --target x86_64-pc-windows-msvc \
  --profile=ps4 \
  "$@"

EXE=target/x86_64-pc-windows-msvc/ps4/ruffle4consoles.exe
echo "Output: ${EXE}"
echo ""
echo "Package with:"
echo "  makeappx pack /d platform/xboxone/appx /p ruffle4consoles.msix"
echo "Then sideload via Dev Mode on Xbox One/Series."
