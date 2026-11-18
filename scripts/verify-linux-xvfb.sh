#!/usr/bin/env bash
# Runs the cross-built rad-demo-linux for i686 or aarch64 (qemu-user) under Xvfb
# and takes screenshots: initial, ticking (timer), and after a Greet click.
# usage: scripts/verify-linux-xvfb.sh <i686|aarch64> <outdir> [prefix]
# Needs: xvfb xdotool imagemagick, qemu-user-static (aarch64), the target's GTK libs.
set -euo pipefail
a=$1; out=$2; pre=${3:-$a}
cd "$(dirname "$0")/.."
bin=${CARGO_TARGET_DIR:-target}/$a-unknown-linux-gnu/release/rad-demo-linux
D=:$((90 + RANDOM % 100))
Xvfb $D -screen 0 1024x768x24 >/dev/null 2>&1 & X=$!
trap 'kill $P $X 2>/dev/null || true' EXIT
sleep 2
export DISPLAY=$D GDK_BACKEND=x11 NO_AT_BRIDGE=1 GSETTINGS_BACKEND=memory
if [ "$a" = aarch64 ]; then
  export QEMU_LD_PREFIX=/usr/aarch64-linux-gnu LD_LIBRARY_PATH=/usr/lib/aarch64-linux-gnu
  qemu-aarch64-static "$bin" & P=$!
else
  "$bin" & P=$!
fi
until xdotool search --name . >/dev/null 2>&1; do sleep 2; done
sleep 8
import -window root "$out/$pre-initial.png"
sleep 6; import -window root "$out/$pre-ticking.png"
xdotool mousemove 70 61 click 1; sleep 4
import -window root "$out/$pre-greet.png"
