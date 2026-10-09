#!/usr/bin/env bash
set -euo pipefail

# Requires devkitPro + cargo-3ds: cargo install cargo-3ds
: "${DEVKITPRO:=/opt/devkitpro}"
export PATH="${DEVKITPRO}/tools/bin:${DEVKITPRO}/devkitARM/bin:${PATH}"
export RUSTFLAGS="${RUSTFLAGS:-} -Zthreads=16"

cargo 3ds build --release "$@"

CIA=target/armv6k-nintendo-3ds/release/ruffle4consoles.3dsx
echo "Output: ${CIA}"
echo ""
echo "Deploy to sdmc:/3ds/ruffle/ with:"
echo "  ruffle4consoles.3dsx"
echo "  movie.swf"
