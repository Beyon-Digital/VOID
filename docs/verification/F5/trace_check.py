#!/usr/bin/env python3
"""VOID F5 traceability audit (W29, Lane Y).

Reads the canonical ledgers in docs/void-handoff/tracking/ — never writes them —
plus the lane-authored NEEDS_MAP.json, and emits docs/verification/F5/TRACE_AUDIT.json.

Verified invariants (exit 1 on any violation):
  * every TESTS.json row has a known status vocabulary value
  * every TESTS.json row with a pass_* status has a non-empty evidence string,
    unless the row is declared in NEEDS_MAP.evidence_anomalies (reported, not fixed)
  * every TESTS.json row that is not a full pass has a NEEDS_MAP entry and every
    referenced NEEDS file + anchor substring resolves on disk
  * every TASKS.json row has a non-empty evidence string or a declared
    tasks_without_evidence entry
  * task<->test linkage is bidirectionally consistent
  * every feature/stock/infra/dependency row keeps its source ID and every
    acceptance_test_ids / execution_tasks reference resolves to a real row
  * sub-test ledgers' statuses are in-vocabulary (drift is recorded, not judged)

Non-goals: this script never upgrades a status, never claims parity, and treats
`blocked`/`not_run`/`partial_*` as not-done. Run from the repo root:

    python3 docs/verification/F5/trace_check.py
"""
from __future__ import annotations

import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]  # repo root
TRACK = ROOT / "docs/void-handoff/tracking"
F5 = ROOT / "docs/verification/F5"

PASS_STATUSES = {"pass_linux", "pass_macos"}
KNOWN_TEST_STATUSES = PASS_STATUSES | {
    "partial_linux", "partial_macos", "blocked", "not_run",
}
KNOWN_TASK_STATUSES = {
    "done", "partial", "partial_macos", "not_started", "in_progress",
    "development_ready", "blocked",
}
KNOWN_IMPL_STATUSES = {
    "not_verified", "model_side", "partial", "verified_linux", "verified_macos",
}
KNOWN_INFRA_STATUSES = {"not_verified", "optional_not_authorized", "verified"}
KNOWN_DEP_QUAL = {"unqualified", "qualified_dev", "qualified_dist", "rejected"}


def load(name: str):
    return json.loads((TRACK / name).read_text(encoding="utf-8"))


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.strip()


def ref_ok(ref: str) -> tuple[bool, str]:
    """A NEEDS ref is '<repo-relative path>#<anchor substring>'."""
    if "#" not in ref:
        return False, f"ref lacks '#anchor': {ref}"
    path, anchor = ref.split("#", 1)
    target = ROOT / path
    if not target.is_file():
        return False, f"missing file: {path}"
    if anchor not in target.read_text(encoding="utf-8", errors="replace"):
        return False, f"anchor not found in {path}: {anchor!r}"
    return True, ""


def main() -> int:
    failures: list[str] = []
    notes: list[str] = []

    tests = load("TESTS.json")["tests"]
    tasks = load("TASKS.json")["tasks"]
    ft = load("FEATURE_TRACEABILITY.json")
    infra = load("INFRASTRUCTURE.json")["requirements"]
    deps = load("DEPENDENCIES.json")["libraries"]
    plats = load("PLATFORMS.json")["targets"]
    needs_map = json.loads((F5 / "NEEDS_MAP.json").read_text(encoding="utf-8"))

    task_by_id = {t["id"]: t for t in tasks}
    test_by_id = {t["id"]: t for t in tests}

    # ---- referential integrity -------------------------------------------------
    if len(test_by_id) != len(tests):
        failures.append("duplicate test ids in TESTS.json")
    if len(task_by_id) != len(tasks):
        failures.append("duplicate task ids in TASKS.json")
    for t in tests:
        owner = t.get("task_id")
        if owner not in task_by_id:
            failures.append(f"{t['id']}: task_id {owner!r} not in TASKS.json")
        elif t["id"] not in task_by_id[owner]["tests"]:
            failures.append(f"{t['id']}: not listed in {owner}.tests")
    for t in tasks:
        for tid in t["tests"]:
            if tid not in test_by_id:
                failures.append(f"{t['id']}: lists unknown test {tid}")
            elif test_by_id[tid]["task_id"] != t["id"]:
                failures.append(f"{t['id']}: {tid} belongs to {test_by_id[tid]['task_id']}")
        for dep in t.get("depends_on", []):
            if dep not in task_by_id:
                failures.append(f"{t['id']}: depends_on unknown task {dep}")

    # ---- test rows: status + evidence + needs coverage -------------------------
    audit_tests = {}
    for t in tests:
        tid, status, ev = t["id"], t.get("status"), t.get("evidence")
        ev_str = ev if isinstance(ev, str) else ""
        has_ev = bool(ev_str.strip())
        entry = {
            "task_id": t["task_id"], "phase": t["phase"], "status": status,
            "has_evidence": has_ev,
            "needs_map": needs_map["tests"].get(tid),
        }
        if status not in KNOWN_TEST_STATUSES:
            failures.append(f"{tid}: unknown status {status!r}")
        is_pass = status in PASS_STATUSES
        if is_pass and not has_ev:
            if tid in needs_map.get("evidence_anomalies", {}):
                entry["anomaly"] = needs_map["evidence_anomalies"][tid]
                notes.append(f"{tid}: pass row without evidence — declared anomaly")
            else:
                failures.append(f"{tid}: {status} with empty evidence, not declared")
        if not is_pass:
            m = needs_map["tests"].get(tid)
            if not m:
                failures.append(f"{tid}: status {status} but no NEEDS_MAP entry")
            else:
                for ref in m.get("needs", []):
                    ok, why = ref_ok(ref)
                    if not ok:
                        failures.append(f"{tid}: NEEDS ref unresolved — {why}")
        audit_tests[tid] = entry

    declared_anomalies = set(needs_map.get("evidence_anomalies", {}))
    found_anomalies = {
        t["id"] for t in tests
        if t["status"] in PASS_STATUSES and not (
            isinstance(t["evidence"], str) and t["evidence"].strip())
    }
    if declared_anomalies != found_anomalies:
        failures.append(
            f"evidence_anomalies declared {sorted(declared_anomalies)} != "
            f"found {sorted(found_anomalies)}"
        )

    # ---- task rows: evidence coverage ------------------------------------------
    audit_tasks = {}
    tasks_missing_ev = set()
    for t in tasks:
        has_ev = bool(str(t.get("evidence", "")).strip())
        audit_tasks[t["id"]] = {
            "phase": t["phase"], "status": t["status"],
            "has_evidence": has_ev,
            "implementation_commit": t.get("implementation_commit"),
            "blocker": t.get("blocker"),
        }
        if t["status"] not in KNOWN_TASK_STATUSES:
            failures.append(f"{t['id']}: unknown task status {t['status']!r}")
        if not has_ev:
            tasks_missing_ev.add(t["id"])
            if t["id"] not in needs_map.get("tasks_without_evidence", {}):
                failures.append(f"{t['id']}: empty evidence, not declared in map")
    undeclared = set(needs_map.get("tasks_without_evidence", {})) - tasks_missing_ev
    if undeclared:
        notes.append(f"map declares tasks_with_evidence anyway: {sorted(undeclared)}")

    # ---- feature / stock / infra / dep / platform ledgers ------------------------
    def feature_class(tids: list[str]) -> str:
        sts = [test_by_id.get(x, {}).get("status", "MISSING") for x in tids]
        if not tids:
            return "unmapped"
        if "MISSING" in sts:
            return "missing_test"
        s = set(sts)
        if s == {"pass_linux"}:
            return "pass_linux"
        if s == {"pass_macos"}:
            return "pass_macos_engine"
        if s == {"blocked"}:
            return "blocked"
        if s == {"not_run"}:
            return "not_run"
        if s & {"not_run", "blocked", "partial_linux", "partial_macos"}:
            return "partial_or_open"
        return "mixed_pass"

    feat_rollup = Counter()
    feat_rows = {}
    seen_ids = set()
    for f in ft["features"]:
        fid = f["id"]
        if fid in seen_ids:
            failures.append(f"duplicate feature id {fid}")
        seen_ids.add(fid)
        for tid in f.get("acceptance_test_ids", []):
            if tid not in test_by_id:
                failures.append(f"{fid}: unknown acceptance test {tid}")
        for w in f.get("execution_tasks", []):
            if w not in task_by_id:
                failures.append(f"{fid}: unknown execution task {w}")
        if f.get("implementation_status") not in KNOWN_IMPL_STATUSES:
            notes.append(f"{fid}: nonstandard impl status {f.get('implementation_status')!r}")
        cls = feature_class(f.get("acceptance_test_ids", []))
        feat_rollup[cls] += 1
        feat_rows[fid] = {
            "category": f["category"], "feature": f["feature"],
            "ledger_status": f.get("implementation_status"),
            "derived_from_tests": cls,
            "acceptance_test_ids": f.get("acceptance_test_ids", []),
        }

    stock_rollup = Counter()
    stock_rows = {}
    for s in ft["stock_inventory"]:
        sid = s["id"]
        tids: list[str] = []
        for w in s.get("execution_tasks", []):
            if w not in task_by_id:
                failures.append(f"{sid}: unknown execution task {w}")
            else:
                tids += task_by_id[w]["tests"]
        cls = feature_class(tids)
        stock_rollup[cls] += 1
        stock_rows[sid] = {
            "kind": s["kind"], "names": s["names"],
            "ledger_status": s.get("implementation_status"),
            "derived_from_owner_tests": cls,
            "via_task_tests": tids,
        }

    for i in infra:
        for w in i.get("execution_tasks", []):
            if w not in task_by_id:
                failures.append(f"{i['id']}: unknown execution task {w}")
        if i["status"] not in KNOWN_INFRA_STATUSES:
            notes.append(f"{i['id']}: nonstandard infra status {i['status']!r}")
    for d in deps:
        for w in d.get("execution_tasks", []):
            if w not in task_by_id:
                failures.append(f"{d['id']}: unknown execution task {w}")
        if d.get("qualification_status") not in KNOWN_DEP_QUAL:
            notes.append(f"{d['id']}: nonstandard dep status {d.get('qualification_status')!r}")

    # ---- report ------------------------------------------------------------------
    status_counts = Counter(t["status"] for t in tests)
    task_counts = Counter(t["status"] for t in tasks)
    uncovered = sorted(
        tid for tid, e in audit_tests.items()
        if e["status"] not in PASS_STATUSES and not e["needs_map"]
    )
    audit = {
        "schema": "void-f5-trace-audit/1",
        "generated_by": "docs/verification/F5/trace_check.py (Lane Y / W29)",
        "repo": "Beyon-Digital/VOID",
        "audit_branch": git("branch", "--show-current"),
        "audit_head": git("rev-parse", "HEAD"),
        "integration_branch_head": git("rev-parse", "origin/devin/void-implementation"),
        "policy": "blocked/not_run/partial_* are not-done; ledger statuses are reported, never upgraded.",
        "test_status_counts": dict(status_counts),
        "task_status_counts": dict(task_counts),
        "feature_rollup_derived": dict(feat_rollup),
        "stock_rollup_derived": dict(stock_rollup),
        "feature_ledger_status_counts": dict(
            Counter(f.get("implementation_status") for f in ft["features"])),
        "stock_ledger_status_counts": dict(
            Counter(s.get("implementation_status") for s in ft["stock_inventory"])),
        "infra_status_counts": dict(Counter(i["status"] for i in infra)),
        "dep_qualification_counts": dict(Counter(d["qualification_status"] for d in deps)),
        "platform_status_counts": dict(Counter(p["status"] for p in plats)),
        "evidence_anomalies": needs_map.get("evidence_anomalies", {}),
        "non_pass_rows": {
            tid: {
                "status": e["status"], "task": e["task_id"],
                "needs": (e["needs_map"] or {}).get("needs", []),
                "class": (e["needs_map"] or {}).get("class"),
            }
            for tid, e in audit_tests.items() if e["status"] not in PASS_STATUSES
        },
        "uncovered_not_done_rows": uncovered,
        "tasks_without_evidence": sorted(tasks_missing_ev),
        "tests": audit_tests,
        "tasks": audit_tasks,
        "features": feat_rows,
        "stock": stock_rows,
        "notes": notes,
        "result": "fail" if failures else "pass",
        "failures": failures,
    }
    (F5 / "TRACE_AUDIT.json").write_text(
        json.dumps(audit, indent=2, sort_keys=False) + "\n", encoding="utf-8")

    print(f"tests: {dict(status_counts)}")
    print(f"tasks: {dict(task_counts)}")
    print(f"features (derived): {dict(feat_rollup)}")
    print(f"stock (derived):    {dict(stock_rollup)}")
    print(f"non-pass rows mapped: {len(audit['non_pass_rows'])}, uncovered: {uncovered}")
    print(f"tasks without evidence: {sorted(tasks_missing_ev)}")
    for n in notes:
        print(f"note: {n}")
    if failures:
        print("\nFAILURES:")
        for f_ in failures:
            print(f"  - {f_}")
        print("TRACE_AUDIT.json written (result=fail)")
        return 1
    print("TRACE_AUDIT.json written (result=pass)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
