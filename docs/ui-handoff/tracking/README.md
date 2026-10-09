# UI traceability ledger

Per `VOID_UI_UX_Build_Prompt.md` UI00: every screen/component/test row moves
`not_started → in_progress → verified` (or `blocked` with a NEEDS-style
reason). Evidence = a command + exit code, a screenshot path, or a PR/SHA —
never a claim.

- `SCREENS.json` — one row per designed screen (S01–S28). `figmaNodeId` is the
  ground-truth node; `../figma/screens/<id>.png` its reference render,
  `../figma/metadata/<id>.xml` its layer tree. Larger screens also have a
  `../figma/context/<id>.txt` design-context dump (reference code + metadata).
- `COMPONENTS.json` — the 13 canonical families (with their Figma variant
  count). A component is `verified` when its variants exist in
  `packages/void-ui`, honor the dark/daylight themes and compact layout, and
  have behavior tests (pointer capture, numeric entry, arrows, reset,
  one-gesture-one-undo where applicable).
- `UI_TESTS.json` — the 36 acceptance scenarios (UI-T01..T36). `verified`
  requires an executable test or a recorded run; `blocked` requires a
  reproducible command + reason (hardware, licence, model).

Rules copied from the engine handoff: real implementations only; no seeded
demo data in production UI; fixtures belong in tests/stories. Ledger rows are
updated by the lane that lands the work and re-checked at merge.
