#!/bin/sh
# T65 offline gate: runs the capability + manual-ops suite with EVERY
# socket denied — loopback included — via an unprivileged user+net
# namespace (`unshare -Urn`), plus `env -i` so no PATH to network tools.
#
#   tests/journeys/ai/offline_gate.sh          # builds test bins, then gates
#
# Requirements: unshare(1) with unprivileged userns, python3, node.
# Fails loudly (exit != 0) if sockets are reachable inside the gate or
# if any gated test fails.
set -eu

ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
TAIDIR="$ROOT/tests/journeys/ai"
cd "$TAIDIR"

echo "== build test binaries (outside the gate) =="
cargo test --no-run --quiet 2>&1 | tail -3

BIN_DIR="$TAIDIR/target/debug/deps"
T65_BIN=""
for f in "$BIN_DIR"/t65_capability-*; do
  [ -e "$f" ] || continue
  case "$f" in *.d) ;; *) [ -x "$f" ] && T65_BIN="$f" ;; esac
done
[ -n "$T65_BIN" ] || { echo "t65_capability test binary not found"; exit 2; }
echo "test binary: $T65_BIN"

echo "== enter denied network namespace (unshare -Urn env -i) =="
unshare -Urn env -i \
  HOME="$HOME" \
  PATH="/usr/local/bin:/usr/bin:/bin" \
  VOID_OFFLINE_GATE=1 \
  VOID_REPO="$ROOT" \
  T65_BIN="$T65_BIN" \
  sh "$TAIDIR/offline_gate_inner.sh"
echo "OFFLINE GATE PASS"
