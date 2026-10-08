#!/bin/sh
# Provision the void-audio-generate-procedural worker bundle: venv + pinned deps.
# Idempotent; safe to re-run. Weights/venv stay gitignored —
# manifest.json pins the exact artifact sha256s installed.
set -eu
D=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PY=${PYTHON:-python3}
"$PY" -m venv "$D/.venv"
"$D/.venv/bin/pip" install --upgrade pip -q
"$D/.venv/bin/pip" install -r "$D/requirements.txt"
# no extra provisioning (weights bundled in wheel / code-only)
echo "void-audio-generate-procedural provisioned"
