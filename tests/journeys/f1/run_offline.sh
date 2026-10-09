#!/usr/bin/env bash
# F1 journey offline wrapper: runs a f1_journey.py subcommand inside a fresh
# user+network namespace so NO sockets exist (no IPv4, no IPv6, no loopback).
# The worker's UDS control/telemetry sockets are filesystem paths and keep
# working; --offline-check makes the runner prove TCP/loopback are denied.
#
# Usage: tests/journeys/f1/run_offline.sh <repo-root> <mode> [args...]
#   e.g. run_offline.sh "$PWD" t44 <engine> <fixture> <workdir>
set -euo pipefail
ROOT="$(cd "$1" && pwd)"; shift
MODE="$1"; shift

# Host-loopback leak probe: a listener bound to 127.0.0.1 on the HOST
# namespace. If the sandboxed runner can still connect to it, the two
# share a loopback (isolation broken); refusal proves isolation.
PROBE_PORT=39187
python3 -c "
import socket, sys, time
s = socket.socket()
try:
    s.bind(('127.0.0.1', $PROBE_PORT))
except OSError:
    sys.exit(0)
s.listen(4)
time.sleep(600)
" &
PROBE_PID=$!
trap 'kill "$PROBE_PID" 2>/dev/null || true' EXIT

exec unshare -Urn env -i \
    HOME="$HOME" \
    PATH="/usr/local/bin:/usr/bin:/bin" \
    PYTHONPATH="$ROOT/third_party/flatbuffers/python" \
    OFFLINE_PROBE_PORT="$PROBE_PORT" \
    python3 "$ROOT/tests/journeys/f1/f1_journey.py" "$MODE" "$@"
