#!/usr/bin/env python3
"""void-audio-generate — text→audio worker on Meta MusicGen (audiocraft).

Real adapter: ``facebook/musicgen-small`` (EnCodec + autoregressive LM)
on torch CPU. Weights are vendored into ``models/hf`` by ``setup.sh``;
``HF_HUB_OFFLINE=1`` pins the worker offline — missing weights fail the
job rather than downloading at job time.

spec.parameters:
    prompt            — text conditioning (required, ≤2000 chars)
    durationS         — seconds to render (default 4.0, cap 30.0)
    seed              — RNG seed (default 42; recorded — sampling is not
                        guaranteed byte-deterministic across runtimes)
    temperature/topK/topP/cfgCoef — sampler knobs (audiocraft defaults)
Output: ``generated.wav`` @ 32 kHz mono + ``generation.json`` + ``metrics.json``.

Licence note: musicgen-small weights are CC-BY-NC-4.0 (non-commercial).
Qualified for development evaluation only — release/distribution rights
stay open in docs/ai-jobs/QUALIFICATION.md.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.dont_write_bytecode = True
_HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE.parent / "pycommon"))


def _run(spec: dict, staging: Path, started_ns: int):
    import void_worker as vw  # noqa: E402

    os.environ["HF_HOME"] = str(_HERE / "models" / "hf")
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["CUDA_VISIBLE_DEVICES"] = ""
    os.environ.setdefault("XDG_CACHE_HOME", str(staging / ".cache"))
    os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")

    params = spec.get("parameters") or {}
    prompt = str(params.get("prompt", "")).strip()
    if not prompt or len(prompt) > 2000:
        raise ValueError("parameters.prompt must be 1..2000 chars")
    duration = float(params.get("durationS", 4.0))
    if not (0.25 <= duration <= 30.0):
        raise ValueError("parameters.durationS out of range 0.25..30")
    seed = int(params.get("seed", 42))
    threads = int(spec.get("reservations", {}).get("cpuThreads", 0) or 0)

    vw.progress(5, "loading musicgen-small")
    import numpy as np  # noqa: E402
    import soundfile as sf  # noqa: E402
    import torch  # noqa: E402
    from audiocraft.models import MusicGen  # noqa: E402

    if threads > 0:
        torch.set_num_threads(threads)
    torch.manual_seed(seed)

    mg = MusicGen.get_pretrained("facebook/musicgen-small", device="cpu")
    gen_kwargs = {"duration": duration}
    for k_src, k_dst in (
        ("temperature", "temperature"),
        ("topK", "top_k"),
        ("topP", "top_p"),
        ("cfgCoef", "cfg_coef"),
    ):
        if k_src in params:
            gen_kwargs[k_dst] = params[k_src]
    mg.set_generation_params(**gen_kwargs)

    vw.progress(25, "generating")
    t0 = __import__("time").monotonic_ns()
    wav = mg.generate([prompt], progress=False)
    infer_ms = (int(__import__("time").monotonic_ns()) - t0) // 1_000_000
    vw.progress(85, "writing wav")

    audio = wav[0].detach().cpu().numpy().T.astype(np.float32)
    audio = np.clip(audio, -1.0, 1.0)
    out = staging / "generated.wav"
    sf.write(str(out), audio, mg.sample_rate, subtype="PCM_16")

    prov = {
        "v": 1,
        "worker": "void-audio-generate",
        "model": {"name": "facebook/musicgen-small", "kind": "text-to-music LM+codec", "weightsLicense": "CC-BY-NC-4.0"},
        "prompt": prompt,
        "params": {"durationS": duration, "seed": seed, **{k: params[k] for k in ("temperature", "topK", "topP", "cfgCoef") if k in params}},
        "output": {"sampleRate": mg.sample_rate, "frames": int(audio.shape[0]), "seconds": float(audio.shape[0] / mg.sample_rate)},
        "deterministic": False,
    }
    (staging / "generation.json").write_text(json.dumps(prov, indent=2) + "\n")

    artifacts = ["generated.wav", "generation.json"]
    artifacts.append(
        vw.write_metrics(
            staging,
            started_ns,
            {
                "worker": "void-audio-generate",
                "model": {"name": "facebook/musicgen-small"},
                "inferenceMs": infer_ms,
                "seed": seed,
                "outWavSha256": vw.sha256_file(out),
            },
        )
    )
    warnings = [
        f"{prompt[:60]!r} -> {audio.shape[0] / mg.sample_rate:.2f}s @ {mg.sample_rate}Hz in {infer_ms}ms",
        "musicgen-small weights are CC-BY-NC-4.0 — development evaluation only",
    ]
    return artifacts, warnings


if __name__ == "__main__":
    import void_worker as _vw  # noqa: E402

    _vw.run_worker(_run)
