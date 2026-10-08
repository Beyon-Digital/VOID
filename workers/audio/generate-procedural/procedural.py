#!/usr/bin/env python3
"""void-audio-generate-procedural — deterministic chord→WAV synth.

Not a model: a real additive-synthesis renderer (harmonic partials,
ADSR, vibrato, Schroeder reverb, soft clip) driven by spec parameters.
Deterministic — the same spec renders byte-identical WAV on the same
build, so the test harness asserts a stable output sha256.

Registered with ``kind: procedural`` in its model descriptor — an honest
label, never presented as learned generation. The qualified ML
candidate lives in ``workers/audio/generate`` (musicgen).

spec.parameters (all optional):
    seed             — default 0
    bpm              — default 96 (info field; chord slots are wall-clock)
    secondsPerChord  — default 1.0 (0.25..8)
    progression      — array of chords, each an array of MIDI notes
                       (default Cmaj7 Am7 Fmaj7 G7)
    sampleRate       — default 44100 (8000..96000)
    timbre           — 'pad' (default) | 'bright' | 'sub'
    gainDb           — default -6.0 (-60..0)
Output: ``generated.wav`` (16-bit stereo) + ``notes.json`` + ``metrics.json``.
"""

from __future__ import annotations

import json
import struct
import sys
import wave
from pathlib import Path

sys.dont_write_bytecode = True
_HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE.parent / "pycommon"))


def _freq(midi: float) -> float:
    return 440.0 * 2.0 ** ((midi - 69.0) / 12.0)


def _note(freq: float, t0: float, dur: float, sr: int, timbre: str, rng) -> "object":
    import numpy as np

    n = int(dur * sr)
    t = np.arange(n) / sr
    cents = rng.uniform(-4.0, 4.0)
    f = freq * 2.0 ** (cents / 1200.0)
    vib = 1.0 + 0.0012 * np.sin(2 * np.pi * 5.0 * t)
    f = f * vib
    phases = 2 * np.pi * np.cumsum(f) / sr
    if timbre == "bright":
        amps = [1.0 / k for k in range(1, 10)]
    elif timbre == "sub":
        amps = [1.0, 0.15, 0.05]
    else:
        amps = [1.0 / (k ** 0.8) for k in range(1, 9)]
    x = np.zeros(n)
    for k, a in enumerate(amps, 1):
        x += a * np.sin(k * phases + 0.35 * k)
    x /= sum(amps)
    a = max(1, int(0.015 * sr))
    d = max(1, int(0.08 * sr))
    r = max(1, int(min(0.18, dur * 0.4) * sr))
    env = np.ones(n)
    env[:a] = t[:a] / (a / sr)
    env[a : a + d] = 1.0 - 0.3 * np.linspace(0, 1, min(d, n - a))
    env[-r:] *= np.linspace(1, 0, r)
    return t0, x * env


def _schroeder(x: "object", sr: int, wet: float = 0.22) -> "object":
    import numpy as np

    def comb(sig, delay_s, fb):
        d = max(1, int(delay_s * sr))
        y = sig.copy()
        y[d:] += sig[:-d] * fb
        return y

    def allpass(sig, delay_s, g=0.5):
        d = max(1, int(delay_s * sr))
        y = np.zeros_like(sig)
        y[d:] = sig[:-d] * -g
        buf = np.zeros_like(sig)
        buf[d:] = sig[:-d] + g * y[:-d] if d < len(sig) else sig[:-d]
        return sig + buf

    wet_sig = np.zeros_like(x)
    for dly, fb in ((0.0297, 0.72), (0.0311, 0.70), (0.0351, 0.66), (0.0399, 0.62)):
        wet_sig += comb(x, dly, fb)
    wet_sig = allpass(wet_sig, 0.005, 0.5)
    wet_sig = allpass(wet_sig, 0.0017, 0.5)
    return x * (1 - wet) + wet_sig * wet


def _render(progression, seconds_per_chord, sr, timbre, gain_db, seed):
    import numpy as np

    rng = np.random.RandomState(seed & 0x7FFFFFFF)
    slot = float(seconds_per_chord)
    tail = 0.35
    total_s = slot * len(progression) + tail
    n = int(total_s * sr)
    mix = np.zeros(n)
    notes = []
    for ci, chord in enumerate(progression):
        for midi in chord:
            freq = _freq(float(midi))
            t0 = ci * slot
            dur = slot + 0.22
            i0 = int(t0 * sr)
            _, sig = _note(freq, t0, min(dur, total_s - t0), sr, timbre, rng)
            m = min(n - i0, len(sig))
            if m > 0:
                mix[i0 : i0 + m] += sig[:m] * 0.5
            notes.append(
                {
                    "midi": int(midi),
                    "freqHz": round(freq, 3),
                    "onsetS": round(t0, 4),
                    "durS": round(min(dur, total_s - t0), 4),
                    "chord": ci,
                }
            )
    mix = _schroeder(mix, sr)
    g = 10.0 ** (gain_db / 20.0)
    mix = np.tanh(mix * g)
    peak = float(np.max(np.abs(mix))) or 1.0
    mix *= 0.891 / peak
    delay = int(0.0075 * sr)
    left = mix
    right = np.roll(mix, delay)
    right[:delay] = 0
    stereo = np.stack([left, right], axis=-1)
    return stereo, notes, total_s


def _write_wav(path: Path, stereo, sr: int) -> None:
    import numpy as np

    pcm = (np.clip(stereo, -1, 1) * 32767).astype("<i2")
    with wave.open(str(path), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(sr)
        w.writeframes(pcm.tobytes())


def _run(spec: dict, staging: Path, started_ns: int):
    import void_worker as vw  # noqa: E402

    params = spec.get("parameters") or {}
    seed = int(params.get("seed", 0))
    bpm = float(params.get("bpm", 96))
    spc = float(params.get("secondsPerChord", 1.0))
    sr = int(params.get("sampleRate", 44100))
    timbre = str(params.get("timbre", "pad"))
    gain_db = float(params.get("gainDb", -6.0))
    progression = params.get("progression") or [
        [60, 64, 67, 71],
        [57, 60, 64, 67],
        [53, 57, 60, 64],
        [55, 59, 62, 65],
    ]
    if timbre not in ("pad", "bright", "sub"):
        raise ValueError("timbre must be pad|bright|sub")
    if not (0.25 <= spc <= 8.0):
        raise ValueError("secondsPerChord out of range")
    if not (8000 <= sr <= 96000):
        raise ValueError("sampleRate out of range")
    if not isinstance(progression, list) or not (1 <= len(progression) <= 64):
        raise ValueError("progression must be 1..64 chords")
    for chord in progression:
        if not isinstance(chord, list) or not (1 <= len(chord) <= 16):
            raise ValueError("each chord must hold 1..16 midi notes")
        for m in chord:
            if not (0 <= int(m) <= 127):
                raise ValueError("midi note out of range")

    vw.progress(20, "rendering")
    stereo, notes, total_s = _render(progression, spc, sr, timbre, gain_db, seed)
    out = staging / "generated.wav"
    _write_wav(out, stereo, sr)
    vw.progress(85, "wav written")

    prov = {
        "v": 1,
        "worker": "void-audio-generate-procedural",
        "model": {"name": "void.procedural-synth", "kind": "procedural"},
        "params": {"seed": seed, "bpm": bpm, "secondsPerChord": spc, "sampleRate": sr, "timbre": timbre, "gainDb": gain_db},
        "output": {"frames": int(stereo.shape[0]), "seconds": total_s, "channels": 2},
        "notes": notes,
        "deterministic": True,
    }
    (staging / "notes.json").write_text(json.dumps(prov, indent=2) + "\n")

    artifacts = ["generated.wav", "notes.json"]
    artifacts.append(
        vw.write_metrics(
            staging,
            started_ns,
            {
                "worker": "void-audio-generate-procedural",
                "outWavSha256": vw.sha256_file(out),
                "noteCount": len(notes),
            },
        )
    )
    return artifacts, [f"{len(notes)} notes, {total_s:.2f}s @ {sr}Hz (kind: procedural)"]


if __name__ == "__main__":
    import void_worker as _vw  # noqa: E402

    _vw.run_worker(_run)
