# VOID — Release candidate report (W29 / F5, Lane Y)

**This document reports release readiness. It does not authorize, publish, or
submit a release** — public release, store submission, relicensing and licence
purchase all require explicit authorization (decision D06, HANDOFF.md; open as
F5-N12 in `docs/verification/F5/NEEDS.md`).

- **Candidate version:** `1.0.0-rc.1` (proposed tag target on
  `devin/void-implementation`; the integrator owns the actual cut)
- **Audited head:** `9099089` on `devin/void-implementation`
- **Audit artifacts:** `docs/verification/F5/` — `F5_RECONCILIATION.md`,
  `FEATURE_MATRIX.md`, `RESIDUAL_RISKS.md`, `NEEDS.md`, `NEEDS_MAP.json`,
  `TRACE_AUDIT.json`, `trace_check.py`

## 1. Supported platforms — honest split

| Platform | Class | Evidence pointer |
|---|---|---|
| Linux x86_64 (Ubuntu) | **verified** | workspace `cargo test` 146/146; `pnpm -r test` all green; 13 detached suites 230/230; suite commands + exits in `F5_RECONCILIATION.md` |
| macOS Apple Silicon | **engine-qualified** | T03 TestRunner 450 assertions/22,282 samples; T14 render 1,536,000-frame WAV checksum; T22/T24 plugin scan; T31–T35 recording fixtures — all in `docs/engine/EVIDENCE.md` and lane evidence strings in `TEST_MATRIX.md`. Re-run on hardware before tagging (F5-N02) |
| Windows x64 | **not tested** | `PLATFORMS.json` row `not_tested`; bring-up is a new lane (F5-N02) |

`partial_macos` rows mean model-side evidence exists on Linux plus partial
macOS evidence — not full macOS parity.

## 2. Install / update / recovery notes

- **Install (dev):** `pnpm install --frozen-lockfile` (pnpm 9.0.0, node ≥22),
  `pnpm --filter void-client --filter void-core --filter void-daw --filter void-ui build`,
  then `cargo test --workspace --exclude void-tauri`. `void-tauri` GUI needs
  WebKitGTK on Linux / Xcode on macOS — excluded from the green gate by design
  (F5-N13).
- **AI model workers:** `workers/audio/<worker>/setup.sh` per worker; up to ~3 GB
  of vendored weights, gitignored. Without provisioning, `tests/models` fails
  5/6 by design (F5-N10).
- **Project storage:** `.void` container layout — `CURRENT` pointer,
  `checkpoints/`, `assets/sha256/`, `staging/`, `recordings/`; op/journal ack
  statuses `APPLIED|DUPLICATE|REJECTED|OUTCOME_UNKNOWN` — contract in
  `docs/void-handoff/CONTRACTS.md`. Crash-recovery is failpoint-tested
  (tests/recovery 22/22, kill at every journal boundary).
- **Update path:** unsigned dev bundles only. Code signing, notarization and
  the updater channel are open (F5-N03; tests T26–T28 unrun).
- **Rollback:** per-project, restore via `checkpoints/` + `CURRENT` pointer
  (single-transaction saves make torn states impossible). App-level rollback
  playbook below is a skeleton — rehearse before tagging (F5-N03/N05).

## 3. Incident / rollback playbook (skeleton)

1. **Detect:** crash reports via diagnostics bundle (redaction policy in
   `docs/void-handoff/CONTRACTS.md`; privacy sweep T29 open — F5-N04).
2. **Triage:** repro on Linux gate (`cargo test --workspace --exclude
   void-tauri` + `pnpm -r test`); if project-file corruption, restore last
   checkpoint and replay journal.
3. **Mitigate:** flag-gate the offending surface (AI workers, visfx, wasm
   extensions are independently disableable); panic/reset ops per show NEEDS.
4. **Rollback:** repoint `CURRENT` to last-good checkpoint (project) /
   re-deploy previous unsigned bundle (app — no auto-updater yet, so rollback
   = reinstall; see F5-N03).
5. **Postmortem:** append NEEDS entry + add regression row to TESTS.json via
   integrator (lanes do not edit tracking).

## 4. Rights audit summary

- **Engine licences:** Tracktion Engine GPL-3.0, JUCE AGPL-3.0 — dual-licensed;
  development/internal use fine, **distribution BLOCKED** pending licence
  review or commercial purchase (`docs/dependencies/PINS.md`; DEPENDENCIES.json
  `license_approval` unset on all 41 rows → F5-N12).
- **Model weights:** `musicgen-small` is **CC-BY-NC-4.0 — non-commercial
  only** (`docs/ai-jobs/QUALIFICATION.md`); must be excluded or relicensed for
  a commercial build. Other workers' licences itemized in the same doc.
- **Content ledger:** `docs/content-rights/ledger.json` — `LGR-0001`…`LGR-0004`
  status `required`, **not executed**; third-party content clearance is open
  (`docs/content-rights/NEEDS.md`).

## 5. Privacy summary

- **Camera/conducting:** landmark-only by construction — the camera path emits
  skeletal landmarks to the visual engine; raw frames never leave the device
  boundary (`docs/visfx` camera policy; tracker backend itself is NEEDS W23-02).
- **Diagnostics:** bundle contents + redaction contract defined; the
  end-to-end privacy sweep (T29) is unrun — F5-N04.
- **Telemetry:** none shipped; job telemetry hooks are model-side only
  (`docs/av/NEEDS.md`).

## 6. What this candidate is — and is not

Is: a fully verified Linux model-side workspace + engine-qualified macOS
evidence package, ready for the integrator's tagging decision.
Is not: a shippable binary, a signed installer, a macOS/Windows-verified build,
or a rights-cleared distribution. Every not-done row traces to a NEEDS entry —
machine-checked by `trace_check.py` (exit 0, output committed as
`TRACE_AUDIT.json`).
