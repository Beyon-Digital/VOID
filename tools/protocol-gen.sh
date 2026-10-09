#!/usr/bin/env bash
# Regenerate FlatBuffers bindings for protocol/void_control.fbs.
# Pinned generator: flatc 25.9.23 (see docs/dependencies/PINS.md).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUIRED_FLATC="25.9.23"
FLATC="${FLATC:-flatc}"

version="$("$FLATC" --version | awk '{print $3}')"
if [[ "$version" != "$REQUIRED_FLATC" ]]; then
  echo "protocol-gen: flatc $REQUIRED_FLATC required (found $version)." >&2
  echo "Build it: tools/bootstrap/flatc.sh, or set FLATC=/path/to/flatc." >&2
  exit 1
fi

OUT="$ROOT/protocol/generated"
rm -rf "$OUT"
mkdir -p "$OUT/rust" "$OUT/cpp"

"$FLATC" --rust -o "$OUT/rust" "$ROOT/protocol/void_control.fbs"
"$FLATC" --cpp -o "$OUT/cpp" "$ROOT/protocol/void_control.fbs"

echo "generated rust -> protocol/generated/rust"
echo "generated cpp  -> protocol/generated/cpp"
