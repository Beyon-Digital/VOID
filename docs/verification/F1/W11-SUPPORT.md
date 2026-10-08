# F1 evidence — W11-SUPPORT lane (app-level shell features)

Lane: `devin/void-lane-w11ui` off `devin/void-implementation`.
Scope: WORK_PACKAGES W11 item 2 — templates, recent projects, project/track
notes, relink recovery surface, help/onboarding, localizable copy. The live
first-song journey run is a separate macOS lane; this lane ships the
UI/coordinator-side features only.

## What shipped where

| Feature | Module | App surface |
| --- | --- | --- |
| i18n (localizable copy, en catalog) | `packages/void-studio/src/i18n/` — `catalog.ts` (Translator, `{param}` interpolation, layered fallback, `⟦key⟧` missing-key sentinel, `missingKeys`), `en.ts` (~90 keys) | all copy in `apps/void-tauri/src/ui/support.tsx` |
| Project templates | `packages/void-studio/src/templates/` — `templates.ts` (Empty / Vocal+Drums / Live Band descriptors: sample rate, bpm, time signature, real track specs with builtin instruments), `apply.ts` | LauncherPanel → `applyProjectTemplate` |
| Recent projects | `packages/void-studio/src/recents/` — `persistence.ts` (injected KeyValueStore), `recents.ts` (zustand store, dedup by containerDir, cap 20) | LauncherPanel recents column; open → `OpenProjectOp`; `RecentProjectRow` (void-ui) |
| Notes | `packages/void-studio/src/notes/` — `dto.ts` (`NOTE_OPS_AVAILABLE=false`, proposed op shapes), `parse.ts` (PROJECT_SUMMARY/TRACK_LIST note fields, defensive), `drafts.ts` (unsent-intent drafts store) | NotesPanel — document value vs labelled local draft |
| Relink recovery | `packages/void-studio/src/relink/` — `model.ts` (MediaLink serde parsing → `AssetRow`), `verify.ts` (`sha256Hex`, `verifyCandidate` → relink vs explicit-replace, `attachRelinkOp` → `AttachAssetOp`) | RelinkPanel + `MissingAssetRow` (void-ui); ASSET_LIST read |
| Help / onboarding | `packages/void-studio/src/onboarding/` — `onboarding.ts` (5-step tour state, seen flag, help open/close) | `SupportOverlays` (tour + help dialog via `OverlayShell`), HelpButton in shell header |
| void-ui components | `TemplateCard`, `RecentProjectRow`, `MissingAssetRow`, `OverlayShell` (all additive, presentation-only, tokens-styled) | `packages/void-ui/components/` |

Wiring (`apps/void-tauri/src/ui/`): `support.tsx` (new) — singleton app-local
stores + all panels/overlays; `shell.tsx` — launcher replaces the workspace
main area when no project is open, a `notes·assets` toggle shows
NotesPanel + RelinkPanel, `HelpButton` in the header, `SupportOverlays`
mounted at root; `dashboard.tsx` — two pre-existing `next_cursor`→`nextCursor`
type fixes only.

## Protocol ops used (existing union members only — no new ops)

- `CreateProjectOp` (name, container_dir, sample_rate, initial_bpm)
- `SetTimeSignatureOp` (only when a template deviates from 4/4)
- `AddTrackOp` per track spec; `InsertPluginOp` (`VOID-BUILTIN` format) per
  instrument spec — all inside one transaction id per template gesture
- `OpenProjectOp` for recents / open-by-path
- `AttachAssetOp` for the relink commit step
- `ASSET_LIST`, `PROJECT_SUMMARY`, `TRACK_LIST` view reads

## Ownership honesty (CONTRACTS.md)

- `assertViewStateOnly` untouched — no new fields in `studioStore`; each
  feature is its own zustand store (jobs/ precedent).
- Recents/notes-drafts/onboarding flags persist app-locally through an
  injected `KeyValueStore` (localStorage in the app, memory in tests).
  The Tauri app has **no** fs/store plugin and `src-tauri` is outside the
  lane's owned paths, so the brief's "Tauri fs plugin" path does not exist
  yet — the stores are documented as caches, and the coordinator-owned
  index is NEEDS.md §rev2 item 13. Entries are written only after an
  APPLIED create/open receipt (real opens, no fake data).
- Notes: no note ops in protocol major.1 → drafts are explicitly labelled
  "local, unsent" and never rendered as document values; `notes/parse.ts`
  reads real `note`/`notes` fields from views when the coordinator emits
  them (item 12).
- Relink: file pick → WebCrypto sha256 → `relink` (hash match) or
  `replace` (mismatch, explicit-consent checkbox) — mirrors
  `void-assets::link.rs` semantics; the coordinator-side ingest half is
  item 14, and the receipt is shown verbatim (no simulated success).

## Tests run on this branch

- `pnpm -r test` → exit 0 (void-studio 161 tests incl. +36 new: templates
  5, recents 7, notes 7, relink 6, onboarding 5, i18n 5; void-daw 4;
  void-client unchanged)
- `cargo test --workspace --exclude void-tauri` → exit 0 (20 suites ok)
- `tsc -p apps/void-tauri/tsconfig.json --noEmit` → clean
- `pnpm --filter void-studio build` / `void-ui build` → clean (pre-existing
  `isTerminal` star-export collision between `export/` and `jobs/`
  disambiguated in `index.ts`: `isExportJobTerminal` / `isAiJobTerminal`)

## Honest gaps

- Note ops absent (NEEDS.md §rev2 item 12) — notes surface stores unsent
  drafts, does not commit.
- Recents coordinator path absent (item 13) — app-local cache only.
- Asset ingest + `RelinkAssetOp` absent (item 14) — relink UI verifies and
  issues `AttachAssetOp`; bytes-in-container placement is coordinator work.
- `templates.apply` stops at the first non-APPLIED receipt and reports the
  partial apply (no rollback op exists to unwind it).
- The macOS live journey verification (mock/real engine end-to-end) is the
  sibling lane's scope — not run here.
