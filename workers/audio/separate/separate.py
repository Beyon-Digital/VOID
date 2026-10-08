#!/usr/bin/env python3
"""void-audio-separate — stem separation worker on Meta demucs.

Real adapter: ``htdemucs_6s`` (hybrid-transformer, 6 sources: bass,
drums, guitar, other, piano, vocals) via torch CPU. Weights are vendored
into ``models/hf`` by ``setup.sh``; ``HF_HUB_OFFLINE=1`` makes any
job-time download structurally impossible — missing weights fail the
job rather than fetching.

spec.parameters:
    inputFile | input | inputPath   — scoped input asset
    model                           — demucs bag name (default htdemucs_6s)
    shifts                          — default 0 (deterministic; >0 adds
                                      random-shift averaging → outputs vary)
    overlap                         — default 0.25
    jobs                            — demucs worker procs (default 0)
    clip                            — 'clamp' (default) | 'none' | 'rescale'
    bitsPerSample                   — 16 (default) | 24 | 32 | float
Output: ``stems/<name>.wav`` + ``metrics.json``.
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

    # Weights are vendored inside this bundle — point HF at them and pin
    # offline BEFORE huggingface_hub/demucs import so a miss fails loudly.
    os.environ["HF_HOME"] = str(_HERE / "models" / "hf")
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ.setdefault("XDG_CACHE_HOME", str(staging / ".cache"))

    audio = vw.resolve_input(spec, staging)
    vw.progress(5, f"input {audio.name} resolved")

    params = spec.get("parameters") or {}
    model_name = str(params.get("model", "htdemucs_6s"))
    threads = int(spec.get("reservations", {}).get("cpuThreads", 0) or 0)

    import torch  # noqa: E402
    from demucs.api import Separator  # noqa: E402
    from demucs.audio import save_audio  # noqa: E402

    if threads > 0:
        torch.set_num_threads(threads)

    last_pct = [5]

    def cb(state: dict) -> None:
        if state.get("state") == "end":
            total = max(1, int(state.get("audio_length", 1)))
            done = int(state.get("segment_offset", 0))
            pct = 10 + int(80 * min(1.0, done / total))
            if pct > last_pct[0]:
                last_pct[0] = pct
                vw.progress(pct, "separating")

    vw.progress(8, f"loading model {model_name}")
    sep = Separator(
        model=model_name,
        device="cpu",
        shifts=int(params.get("shifts", 0)),
        overlap=float(params.get("overlap", 0.25)),
        split=True,
        jobs=int(params.get("jobs", 0)),
        progress=False,
        callback=cb,
    )

    t0 = __import__("time").monotonic_ns()
    origin, stems = sep.separate_audio_file(str(audio))
    infer_ms = (int(__import__("time").monotonic_ns()) - t0) // 1_000_000
    vw.progress(92, "writing stems")

    outdir = staging / "stems"
    outdir.mkdir(exist_ok=True)
    names = list(stems.keys())
    artifacts = []
    for name in names:
        rel = f"stems/{name}.wav"
        save_audio(
            stems[name],
            str(staging / rel),
            sep.samplerate,
            clip=str(params.get("clip", "clamp")),
            bits_per_sample=int(params.get("bitsPerSample", 16)),
        )
        artifacts.append(rel)

    weights_note = f"{model_name} HF adefossez/{model_name.replace('htdemucs_6s','HTDemucs-6s')} vendored"
    artifacts.append(
        vw.write_metrics(
            staging,
            started_ns,
            {
                "worker": "void-audio-separate",
                "model": {"name": model_name, "source": weights_note},
                "input": {"name": audio.name, "sha256": vw.sha256_file(audio)},
                "inferenceMs": infer_ms,
                "stemCount": len(names),
                "stems": names,
                "samplerate": sep.samplerate,
            },
        )
    )
    warnings = [f"{len(names)} stems ({', '.join(names)}) in {infer_ms}ms"]
    return artifacts, warnings


if __name__ == "__main__":
    import void_worker as _vw  # noqa: E402

    _vw.run_worker(_run)
