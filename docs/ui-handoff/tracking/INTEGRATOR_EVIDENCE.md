# INTEGRATOR_EVIDENCE — merge integration + S16 library + ledger sweep

Integrator pass on `devin/void-ui` after all 8 lane merges (UI01, UIP1–UIP7).

## What the integrator added/fixed directly

- **S16 Library / sounds and assets** (`apps/void-tauri/src/ui/studio/screens/library/`)
  — was unassigned in the lane plan; implemented here to the same contract:
  - ASSET_LIST is the only real collection (parseAssetItem rows: present/missing,
    sha256 provenance, mediaType); instruments/presets from `BUILTIN_INSTRUMENTS`
    + `presetsFor` descriptor tables.
  - Search + category filters (All/Instruments/Presets/Audio) over real rows;
    NAME/TYPE/SOURCE/STATUS table per the Figma S16 layer tree.
  - Details inspector: status badge, provenance hash (truncated), honest
    audition note ("preview isn't exposed by the engine"), explicit target =
    current track selection; `Load on <track>` → `loadInstrument` (same path
    the arrange rail uses), `Place clip on <track>` → real `InsertAudioClipOp`
    for present assets carrying `asset_id`.
  - `Create something new` → `#/generate`; `Import your own` disabled with a
    reason (no asset-import op on this surface).
  - Compact (≤1280): inspector stacks below the table — no clipped controls.

## Gates run on the integrated tree

| Command | Exit |
| --- | --- |
| `pnpm install --frozen-lockfile` | 0 |
| `pnpm -r --if-present test` | 0 — 528 studio + 36 tauri + 22 ui + 17 client + 4 daw + 3 core |
| `pnpm --filter void-tauri exec tsc --noEmit` | 0 |
| `pnpm --filter void-tauri exec vite build` | 0 |
| `pnpm -r --if-present build` | packages 0; `tauri build` fails only at updater signing (`TAURI_SIGNING_PRIVATE_KEY` unprovisioned — pre-existing env gap identical on `main`) |

## Ledger normalization

- SCREENS.json: 21 `implemented` + 6 `verified` (S05, S13, S14, S15, S27, S24) +
  S16 `implemented`. Every row carries its lane evidence doc.
- UI_TESTS.json: UI-T34 `verified`; other rows keep their honest status
  (partial/not_started/blocked) — FINAL_MAP.md maps all 36 with remaining needs.
- FINAL_MAP rows that predated later merges were corrected by the sweep:
  screens are all built; remaining gaps are recorded runs and engine-side
  ops (RequestProposalOp, PreviewLayerOp/audition, SubmitJob/CancelJob,
  JOB_LIST/MODEL_LIST views, TAKE_LIST/SCENE_LIST views, scene/bypass ops,
  import op, export-runner seam) — all NEEDS-tracked in lane evidence docs.
