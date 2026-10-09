#!/usr/bin/env python3
"""void-audio-transcribe — audio→MIDI worker on Spotify basic-pitch.

Real adapter: runs the bundled ICASSP-2022 CNN+timbre model
(``basic_pitch.saved_models.icassp_2022``) via tflite-runtime on CPU —
the wheel ships the weights, no network access at job time.

spec.parameters (all optional except the input):
    inputFile | input | inputPath   — scoped input asset (see resolve_input)
    onsetThreshold                  — default 0.5
    frameThreshold                  — default 0.3
    minNoteLenMs                    — default 127.70
    minFreqHz / maxFreqHz           — pitch range filters
Output: ``transcription.mid`` (SMF) + ``metrics.json``.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

sys.dont_write_bytecode = True
_HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE.parent / "pycommon"))


def _run(spec: dict, staging: Path, started_ns: int):
    import void_worker as vw  # noqa: E402

    # Redirect JIT/OS caches inside the staging write scope before any
    # heavy imports — env_clear means nothing outside granted env is set.
    os.environ.setdefault("NUMBA_CACHE_DIR", str(staging / ".numba_cache"))
    os.environ.setdefault("XDG_CACHE_HOME", str(staging / ".cache"))

    audio = vw.resolve_input(spec, staging)
    vw.progress(8, f"input {audio.name} resolved")

    params = spec.get("parameters") or {}

    import numpy  # noqa: F401,E402 - pins the numpy<2 wheel for tflite
    from basic_pitch.inference import predict  # noqa: E402

    t0 = __import__("time").monotonic_ns()
    model_output, midi_data, note_events = predict(
        str(audio),
        onset_threshold=float(params.get("onsetThreshold", 0.5)),
        frame_threshold=float(params.get("frameThreshold", 0.3)),
        minimum_note_length=float(params.get("minNoteLenMs", 127.70)),
        minimum_frequency=params.get("minFreqHz"),
        maximum_frequency=params.get("maxFreqHz"),
    )
    infer_ms = (int(__import__("time").monotonic_ns()) - t0) // 1_000_000
    vw.progress(85, "inference complete")

    out = staging / "transcription.mid"
    midi_data.write(str(out))

    notes = [n for inst in midi_data.instruments for n in inst.notes]
    # Hash the actual weights that ran — provenance, not a claim.
    import basic_pitch  # noqa: E402

    weights = Path(basic_pitch.__file__).parent / "saved_models" / "icassp_2022" / "nmp.tflite"
    weights_sha = vw.sha256_file(weights) if weights.is_file() else None
    artifacts = ["transcription.mid"]
    artifacts.append(
        vw.write_metrics(
            staging,
            started_ns,
            {
                "worker": "void-audio-transcribe",
                "model": {"name": "basic-pitch", "weightsSha256": weights_sha},
                "input": {"name": audio.name, "sha256": vw.sha256_file(audio)},
                "inferenceMs": infer_ms,
                "noteCount": len(notes),
            },
        )
    )
    warnings = [f"detected {len(notes)} notes in {infer_ms}ms inference"]
    return artifacts, warnings


if __name__ == "__main__":
    _sys_path_ok = True
    import void_worker as _vw  # noqa: E402

    _vw.run_worker(_run)
