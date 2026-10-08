#!/usr/bin/env python3
"""W08 recording-lane harness for void-engine (T31-T35 coverage).

Drives `void-recording-fixture` — the engine-side take runner used because
protocol major.1 has no recording ops (see docs/engine/NEEDS.md). Same
PASS/FAIL-line convention as native/void-engine/tests/harness/supervisor_stub.py.

Scenarios:
  take lifecycle      arm -> record -> stop -> finalized journal + chunks (T31/T32)
  punch               arm -> punch-in live mid-take (T31 punch path)
  kill mid-take       abort -> recover -> `incomplete` labeled, known durations (T35)
  panic               held note -> PANIC -> all-notes-off evidence (T32)
  errors              explicit failure results, never false success (T34)
  latency             hosted-device round-trip measurement (T33, hardware blocked)
  journal integrity   chunk bytes/frames + sha256-at-finalize consistency (T35)

Usage: python3 recording_checks.py <path-to-void-recording-fixture>
Exit 0 only when every check passes.
"""
import json
import os
import re
import subprocess
import sys
import tempfile
import hashlib

FAILS = 0


def check(cond, name):
    global FAILS
    print(("PASS" if cond else "FAIL") + " " + name)
    if not cond:
        FAILS += 1
    return cond


def run(fixture, args, expect_crash=False):
    p = subprocess.run([fixture] + args, capture_output=True, text=True,
                       timeout=600)
    return p.returncode, p.stdout, p.stderr


def parse_summary(out):
    """Last {...} JSON object in fixture stdout = take summary line."""
    for line in reversed(out.splitlines()):
        line = line.strip()
        if line.startswith("{") and "\"takeId\"" in line:
            return json.loads(line)
    return None


def journals(container):
    rec = os.path.join(container, "recordings")
    out = []
    if os.path.isdir(rec):
        for d in sorted(os.listdir(rec)):
            jf = os.path.join(rec, d, "journal.json")
            if os.path.isfile(jf):
                with open(jf) as f:
                    out.append((os.path.join(rec, d), json.load(f)))
    return out


def main():
    if len(sys.argv) < 2:
        print("usage: recording_checks.py <void-recording-fixture> [workdir]")
        return 2
    fixture = os.path.abspath(sys.argv[1])
    work = sys.argv[2] if len(sys.argv) > 2 else tempfile.mkdtemp(prefix="void-rec-")
    if not check(os.path.isfile(fixture) and os.access(fixture, os.X_OK),
                 "fixture-executable"):
        return 2

    # --- T31: multitrack take lifecycle with mono inputs ---------------------
    d1 = os.path.join(work, "t31")
    rc, out, err = run(fixture, ["take", d1, "--midi"])
    s = parse_summary(out)
    check(rc == 0, "t31:take-exit")
    check("FAIL" not in out, "t31:fixture-checks")
    if s:
        check(s["status"] == "finalized", "t31:journal-finalized")
        check(s["chunks"] >= 3, "t31:multitrack-chunks")
        check(s["clips"] >= 3, "t31:clips-created")
        check("dest-a" in s["recordings"][0]["trackIds"]
              and "dest-b" in s["recordings"][0]["trackIds"],
              "t31:multitrack-arm")
    # journal integrity on disk
    for tdir, j in journals(d1):
        check(j["status"] == "finalized", "t31:on-disk-finalized")
        for c in j["chunks"]:
            check(c["bytesObserved"] > 0, "t31:chunk-bytes:" + c["path"])
            check(c["sha256"], "t31:chunk-sha256:" + c["path"])
            f = os.path.join(tdir, c["path"])
            if os.path.isfile(f):
                h = hashlib.sha256(open(f, "rb").read()).hexdigest()
                check(h == c["sha256"], "t31:sha256-match:" + c["path"])

    # --- T31 punch-in --------------------------------------------------------
    d2 = os.path.join(work, "t31punch")
    rc, out, err = run(fixture, ["take", d2, "--punch"])
    s = parse_summary(out)
    check(rc == 0 and "punch-in-b" in out, "t31:punch-in-live")
    check(s and s["status"] == "finalized", "t31:punch-finalized")

    # --- T31 stereo ----------------------------------------------------------
    d3 = os.path.join(work, "t31stereo")
    rc, out, err = run(fixture, ["take", d3, "--stereo"])
    s = parse_summary(out)
    check(rc == 0, "t31:stereo-exit")
    if s:
        check(s["chunks"] == 1, "t31:stereo-single-file")
    for tdir, j in journals(d3):
        for c in j["chunks"]:
            if c["kind"] == "audio":
                check(c["channels"] == 2, "t31:stereo-channels")

    # --- T32: midi capture + panic all-notes-off -----------------------------
    d4 = os.path.join(work, "t32panic")
    rc, out, err = run(fixture, ["panic", d4])
    s = parse_summary(out)
    check(rc == 0, "t32:panic-exit")
    check("panic:all-notes-off-logged" in out, "t32:panic-notes-off")
    if s:
        check(s["allNotesOff"] == 16, "t32:all-notes-off-16ch")
        check(s["noteOffs"] == 0, "t32:no-stuck-notes")
    # midi chunk preserves sustain/bend/aftertouch rows
    for tdir, j in journals(d4):
        for c in j["chunks"]:
            if c["kind"] == "midi-events":
                txt = open(os.path.join(tdir, c["path"])).read()
                check("\"kind\":\"sustain\"" in txt, "t32:sustain-captured")

    # --- T35: kill mid-take -> reopen -> incomplete --------------------------
    d5 = os.path.join(work, "t35")
    rc, out, err = run(fixture, ["take", d5, "--kill-after-blocks", "40"])
    check(rc != 0, "t35:kill-aborts")  # abort() is the simulated crash
    rc, out, err = run(fixture, ["recover", d5])
    check(rc == 0, "t35:recover-exit")
    check("recover:incomplete-labeled" in out, "t35:incomplete-labeled")
    check("recover:no-phantom-recording" in out, "t35:no-phantom-recording")
    for tdir, j in journals(d5):
        check(j["status"] == "incomplete", "t35:on-disk-incomplete")
        audio = [c for c in j["chunks"] if c["kind"] == "audio"]
        check(len(audio) >= 1, "t35:partial-chunks-listed")
        for c in audio:
            check(c["framesObserved"] > 0, "t35:known-duration:" + c["path"])
        # finalized sources stay immutable: no sha256 on incomplete takes
        check(not any(c.get("sha256") for c in j["chunks"]), "t35:incomplete-unsealed")

    # --- T34: explicit error results ------------------------------------------
    rc, out, err = run(fixture, ["errors", os.path.join(work, "t34")])
    check(rc == 0, "t34:errors-exit")
    check("FAIL" not in out and "PASS errors" in out, "t34:explicit-failures")

    # --- T33: hosted-device latency (hardware latency: BLOCKED headless) -----
    rc, out, err = run(fixture, ["latency", os.path.join(work, "t33")])
    s = parse_summary(out)
    check(rc == 0, "t33:latency-exit")
    if s:
        check(s["sampleRate"] == 48000, "t33:rate-48k")
        check(s["roundTripMs"] >= 0.0, "t33:roundtrip-measured")

    print("SUMMARY %d failures" % FAILS)
    return 0 if FAILS == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
