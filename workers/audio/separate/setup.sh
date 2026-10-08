#!/bin/sh
# Provision the void-audio-separate worker bundle: venv + pinned deps.
# Idempotent; safe to re-run. Weights/venv stay gitignored —
# manifest.json pins the exact artifact sha256s installed.
set -eu
D=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PY=${PYTHON:-python3}
"$PY" -m venv "$D/.venv"
"$D/.venv/bin/pip" install --upgrade pip -q
"$D/.venv/bin/pip" install -r "$D/requirements.txt"
# Vendor htdemucs_6s weights into the bundle (HF cache layout).
HF_HOME="$D/models/hf" "$D/.venv/bin/python" -c \
  "from huggingface_hub import snapshot_download; snapshot_download('adefossez/HTDemucs-6s')"
echo "void-audio-separate provisioned"
