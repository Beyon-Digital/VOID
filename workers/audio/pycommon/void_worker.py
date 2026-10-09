"""void_worker — shared helpers for void-job-worker protocol v1.

Stdlib-only module imported by the Python argv workers under
``workers/audio/``. Implements the parts of ``workers/PROTOCOL.md``
that must not drift between adapters:

- spec JSON from stdin (one document, then EOF)
- ``{"v":1,"kind":"progress",...}`` lines, then exactly one terminal
  ``{"v":1,"kind":"result",...}`` line — the last thing on stdout
- staging-relative artifact declarations + SHA-256 of every artifact
- ``metrics.json`` evidence artifact (wall time, peak RSS self+children)
- safe error strings (no absolute paths, bounded length)

A worker calls ``run_worker(fn)``; ``fn`` receives the parsed spec and a
``WorkDir`` rooted at the staging dir, returns a list of artifact paths
(staging-relative) plus warning strings. Every failure becomes a
protocol ``result/failed`` line — the exit code stays 0 so the runner
reads the structured error instead of classifying a crash.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import resource
import sys
import time
import traceback
from pathlib import Path

# Budget guards: worker result/warning strings never carry paths or
# secrets — the runner shows them to users as-is.
_ABS_PATH = re.compile(r"(/[\w.\-]+)+/?")
_MAX_MSG = 400


def _emit(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def progress(percent: float | None = None, message: str | None = None) -> None:
    line: dict = {"v": 1, "kind": "progress"}
    if percent is not None:
        line["percent"] = max(0, min(100, int(percent)))
    if message:
        line["message"] = message[:_MAX_MSG]
    _emit(line)


def safe_msg(text: str) -> str:
    """Strip absolute paths and bound length for user-safe strings."""
    return _ABS_PATH.sub("<path>", text)[:_MAX_MSG]


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def read_spec() -> dict:
    raw = sys.stdin.read()
    if not raw.strip():
        raise ValueError("empty job spec on stdin")
    return json.loads(raw)


def resolve_input(spec: dict, staging: Path) -> Path:
    """Resolve the scoped input asset for a job.

    Accepted parameters (first match wins):
    - ``inputFile``:  path relative to the staging dir — used when the
      coordinator pre-stages input bytes into the job staging dir.
    - ``input``:      asset sha256 — resolved against the project
      container's content-addressed store at
      ``<staging>/../../assets/sha256/<sha>.<ext>``. Reads are
      content-addressed and immutable; staging remains the only WRITE
      scope.
    - ``inputPath``:  absolute path resolved by the coordinator
      (e.g. an ``AssetStore::find`` result). Read-only.
    """
    params = spec.get("parameters") or {}
    if "inputFile" in params:
        p = staging / params["inputFile"]
    elif "input" in params:
        sha = str(params["input"])
        root = staging.parent.parent
        for ext in ("wav", "aiff", "flac", "bin"):
            cand = root / "assets" / "sha256" / f"{sha}.{ext}"
            if cand.is_file():
                return cand
        p = root / "assets" / "sha256" / f"{sha}.wav"
    elif "inputPath" in params:
        p = Path(params["inputPath"])
    else:
        raise ValueError("spec.parameters carries no input (inputFile|input|inputPath)")
    p = p.resolve()
    if not p.is_file():
        raise FileNotFoundError(f"input asset not found for parameters {sorted(params)}")
    return p


def self_peak_rss() -> int:
    """Peak RSS of this process + waited children, in bytes (Linux KiB)."""
    own = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    kids = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    # On Linux ru_maxrss is KiB; children and self are separate peaks,
    # sum is the honest upper bound for a process tree.
    return int((own + kids) * 1024)


def write_metrics(staging: Path, started_ns: int, extra: dict) -> str:
    """Write metrics.json into staging; return its artifact path."""
    metrics = {
        "v": 1,
        "wallMs": max(0, (time.monotonic_ns() - started_ns) // 1_000_000),
        "peakRssBytes": self_peak_rss(),
        "python": sys.version.split()[0],
    }
    metrics.update(extra)
    (staging / "metrics.json").write_text(json.dumps(metrics, indent=2) + "\n")
    return "metrics.json"


def finish_ok(artifacts: list[str], warnings: list[str]) -> None:
    _emit(
        {
            "v": 1,
            "kind": "result",
            "status": "succeeded",
            "artifacts": [{"path": a} for a in artifacts],
            "warnings": warnings,
            "error": None,
        }
    )


def finish_fail(error: str, warnings: list[str] | None = None) -> None:
    _emit(
        {
            "v": 1,
            "kind": "result",
            "status": "failed",
            "artifacts": [],
            "warnings": warnings or [],
            "error": safe_msg(error),
        }
    )


def parse_staging() -> Path:
    argv = sys.argv[1:]
    staging = None
    i = 0
    while i < len(argv):
        if argv[i] == "--staging" and i + 1 < len(argv):
            staging = Path(argv[i + 1])
            i += 2
        else:
            i += 1
    if staging is None:
        sys.stderr.write("void audio worker: --staging <dir> required\n")
        sys.exit(64)
    staging.mkdir(parents=True, exist_ok=True)
    return staging


def run_worker(fn) -> None:
    """Protocol entry point: parse spawn, read spec, run, report once."""
    started = time.monotonic_ns()
    staging = parse_staging()
    try:
        spec = read_spec()
    except Exception as e:  # noqa: BLE001 - surface as protocol failure
        finish_fail(f"bad job spec: {e}")
        sys.exit(0)
    progress(2, "spec parsed")
    try:
        artifacts, warnings = fn(spec, staging, started)
        artifacts = list(artifacts)
        warnings = [safe_msg(w) for w in warnings]
        for a in artifacts:
            p = staging / a
            if not p.is_file():
                raise FileNotFoundError(f"declared artifact missing: {a}")
        finish_ok(artifacts, warnings)
        sys.exit(0)
    except Exception as e:  # noqa: BLE001 - worker failure, not a crash
        sys.stderr.write(traceback.format_exc())
        finish_fail(str(e))
        sys.exit(0)
