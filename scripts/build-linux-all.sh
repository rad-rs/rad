#!/usr/bin/env bash
# Builds rad-demo-linux (GTK 3) for x86_64, i686 and aarch64 (gnu/glibc).
# Linker and pkg-config settings are passed as environment variables so no
# .cargo/config.toml is needed and other targets are unaffected.
# Needs (Debian/Ubuntu, with `dpkg --add-architecture i386 arm64`):
#   libgtk-3-dev, libgtk-3-dev:i386, libgtk-3-dev:arm64,
#   gcc-i686-linux-gnu, gcc-aarch64-linux-gnu
set -euo pipefail
cd "$(dirname "$0")/.."

TARGETS=("${@:-x86_64-unknown-linux-gnu i686-unknown-linux-gnu aarch64-unknown-linux-gnu}")
read -r -a TARGETS <<< "${TARGETS[*]}"

rustup target add "${TARGETS[@]}" >/dev/null

for t in "${TARGETS[@]}"; do
  case "$t" in
    x86_64-unknown-linux-gnu) ;;
    i686-unknown-linux-gnu)
      export CARGO_TARGET_I686_UNKNOWN_LINUX_GNU_LINKER=i686-linux-gnu-gcc ;;
    aarch64-unknown-linux-gnu)
      export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc ;;
    *) echo "unsupported target $t" >&2; exit 1 ;;
  esac
  echo "== $t"
  cargo build --release -p rad-demo-linux --target "$t"
  file "${CARGO_TARGET_DIR:-target}/$t/release/rad-demo-linux"
done
