#!/bin/sh
# Provision the void-audio-generate worker bundle: venv + pinned deps.
# Idempotent; safe to re-run. Weights/venv stay gitignored —
# manifest.json pins the exact artifact sha256s installed.
set -eu
D=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PY=${PYTHON:-python3}
"$PY" -m venv "$D/.venv"
"$D/.venv/bin/pip" install --upgrade pip -q
"$D/.venv/bin/pip" install -r "$D/requirements.txt"
# audiocraft 1.3.0 must come AFTER the pinned deps: its declared pins
# (torch==2.1.0, xformers<0.0.23, av build) break a CPU-only resolve.
# Verified working on torch 2.14.1+cpu --no-deps.
"$D/.venv/bin/pip" install audiocraft==1.3.0 --no-deps
# Vendor musicgen-small weights into the bundle (HF cache layout).
HF_HOME="$D/models/hf" "$D/.venv/bin/python" -c \
  "from huggingface_hub import snapshot_download; snapshot_download('facebook/musicgen-small'); snapshot_download('t5-base')"
echo "void-audio-generate provisioned"
