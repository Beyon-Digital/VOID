#!/bin/sh
# Runs INSIDE `unshare -Urn env -i` (no sockets at all — not even
# loopback). Asserts the deny, then runs the manual-ops vitest subset,
# the save-path dry run, a standalone worker invocation, and the
# capability evidence suite.
set -eu

echo "== socket deny proof =="
python3 - <<'PY'
import socket, sys
fails = 0
for host, port in [("127.0.0.1", 9), ("127.0.0.1", 1), ("1.1.1.1", 443)]:
    try:
        s = socket.create_connection((host, port), timeout=1)
        s.close()
        print(f"FAIL: connected to {host}:{port} inside denied namespace")
        fails += 1
    except OSError as e:
        print(f"denied {host}:{port}: {e}")
try:
    u = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    u.sendto(b"x", ("192.0.2.1", 53))
    u.close()
    print("FAIL: UDP send succeeded inside denied namespace")
    fails += 1
except OSError as e:
    print(f"denied udp: {e}")
sys.exit(1 if fails else 0)
PY

echo "== T65 rust-side gated tests (offline posture + save dry-run) =="
"$T65_BIN" --nocapture t65_offline_posture_socket_deny t65_manual_save_path_dry_run

echo "== manual studio ops (vitest subset) under socket deny =="
cd "$VOID_REPO/packages/void-studio"
node_modules/.bin/vitest run src/gestures src/patterns src/harmony src/learn src/proposals
cd "$VOID_REPO/tests/journeys/ai"

echo "== capability evidence under socket deny =="
# Each capability runs one at a time — real binaries, real artifacts.
# The symbolic + procedural capabilities need no model weights;
# the network-dependent ones (transcribe/separate/musicgen) prove the
# vendored-weights path works with zero sockets.
"$T65_BIN" --nocapture t65_capability_symbolic t65_capability_generate_procedural \
    t65_capability_transcribe t65_capability_separate t65_capability_generate_musicgen

echo "INNER GATE PASS"
