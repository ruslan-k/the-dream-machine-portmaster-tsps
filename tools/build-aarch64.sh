#!/bin/bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
dpkg --add-architecture arm64
cp /work/bullseye-archive.list /etc/apt/sources.list
apt-get -o Acquire::Check-Valid-Until=false update -qq
apt-get install -y -qq gcc-aarch64-linux-gnu g++-aarch64-linux-gnu pkg-config libsdl2-dev:arm64 ca-certificates git default-jre-headless
rustup target add aarch64-unknown-linux-gnu
cd /work/source/runtime
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
export PKG_CONFIG_ALLOW_CROSS=1
export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig
export PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig
export CARGO_BUILD_JOBS=6
export RUSTFLAGS=""
cargo fetch --locked --target aarch64-unknown-linux-gnu
core=/usr/local/cargo/git/checkouts/ruffle-b1b568dcb5621339/563854a/core/src/backend/audio.rs
if ! cmp -s /work/source/core/src/backend/audio.rs "$core"; then
  cp /work/source/core/src/backend/audio.rs "$core"
  cargo clean --release -p ruffle_core --target aarch64-unknown-linux-gnu
fi
mkdir -p /work/test-tmp
TMPDIR=/work/test-tmp cargo test --manifest-path /work/tests/cache/Cargo.toml --target x86_64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
cp target/aarch64-unknown-linux-gnu/release/ruffle4consoles /work/port/dreammachine/ruffle-native-multifile.aarch64.lazy
aarch64-linux-gnu-strip /work/port/dreammachine/ruffle-native-multifile.aarch64.lazy
