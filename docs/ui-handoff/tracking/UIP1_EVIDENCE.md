# UIP1 — lane evidence

Lane: `devin/void-lane-uip1` (base `devin/void-ui` @ `79ef7f22fe68142eb498677e911176ee77353db2`)
Screens: S13 projects/start-here, S14 new-project, S27 arrange empty-state, S15 setup audio+MIDI, S05 record/take lanes.

## What landed

| Screen | Path | Wired data sources |
| --- | --- | --- |
| S13 (4:1208) | `apps/void-tauri/src/ui/studio/screens/projects/` | `recents()` store (`support.tsx`) → `RecentProjectRow` list (no seeded rows — honest empty box); open-by-path → `setProject` + `OpenProjectOp` via `lifecycle.ts`; rail links navigate `library`/`jobs`/`setup`; attach state from `useStudio(s => s.engine.attached)` |
| S14 (4:1308) | `apps/void-tauri/src/ui/studio/screens/new-project/` | `BUILTIN_TEMPLATES` (`void-studio/templates`) → `TemplateCard`s; create → `createProjectFromTemplate` → `applyProjectTemplate` (real one-transaction ops) → `setProject` + `recents.recordOpen('created')` → navigate `arrange`; name/location/BPM editable; readiness aside shows real `engine.attached` + `clock.sample_rate` |
| S27 (12:2479) | `apps/void-tauri/src/ui/studio/screens/arrange/EmptyState.tsx` | Rendered only when TRACK_LIST view `done && items.length===0` (loaded via `loadViewPage`); choices call real ops: `addTrack` AUDIO→`record`, INSTRUMENT→`sendCommand(insertInstrumentOp)`→`compose`, MIDI→`compose`; drag-in note is honest NEEDS §17 (no ingest op) |
| S15 (4:1408) | `apps/void-tauri/src/ui/studio/screens/setup/` | Real attach/clock state: output=`engine.attached`+state, sample rate=`clock.sample_rate`; input/buffer/MIDI enumeration show `—` + NEEDS §8 pointer (no `INPUT_DEVICE_LIST` view on the wire — UI-T09 honest state, no fake meters); permissions copy per metadata; recording-in-progress warning from `parseRecordingSummary`; theme toggle via `useVoidTheme` |
| S05 (4:408) | `apps/void-tauri/src/ui/studio/screens/record/` | `parseRecordingSummary(PROJECT_SUMMARY[0].recording)` → phase/armed-track badges (`armedTracks` voidIds vs TRACK_LIST items); live meter `telemetry.meters[focusTrackId]` peak/rms; Stop → real `client.stop()` transport op + error surface; `lastError` surfaced verbatim; monitor/count-in/metronome = intent-only `createRecordingViewStore` (disabled + NEEDS §7 reason, no invented flags); takes from `createTakeStudioStore` folders→`TakeLane`; UI-T10: review box opens only on recording→idle transition carrying `takeId` |
| — | `packages/void-studio/src/recording/` | New view-model: `parseRecordingSummary`, `recordingUiState` (unavailable/ready/armed/countIn/recording/stopping/review/failed), `createRecordingViewStore` (monitorMode/countInBars/metronome/reviewTakeId — view state only). 10 vitest cases in `recording.test.ts` |
| — | `apps/void-tauri/src/ui/studio/StudioShell.tsx` | `PROJECT_FREE_SCREENS` {projects,new-project,setup,dev-gallery}: these render without an open project; other routes fall back to S13 instead of LauncherPanel |

## Gates

| Command | Exit |
| --- | --- |
| `pnpm install --frozen-lockfile` | 0 |
| `pnpm -r --if-present build` | 0 except `tauri build` fails at updater signing (`TAURI_SIGNING_PRIVATE_KEY` unset) — pre-existing on base, bundles still produced |
| `pnpm -r --if-present test` | 0 — 472 vitest (462 baseline + 10 new `recording.test.ts`) |
| `pnpm --filter void-studio build` (tsc) | 0 |
| `pnpm --filter void-tauri exec tsc --noEmit` | 0 |
| `pnpm --filter void-tauri exec vite build` | 0 (283 modules) |
| `pnpm lint` | 0 |
| `cargo build -p void-worker --bin void-mock-worker` | 0 |
| `cargo build --bin void-tauri` (debug) | 0 |

## Live verification

- `pnpm exec vite` (:5175) + `DISPLAY=:0 ./target/debug/void-tauri` — S13/S14/S15 rendered and checked detached: S13 rail + empty recents, S14 three real templates + "No audio engine attached" readiness, S15 honest `—` device fields + NEEDS §8 hints, `#/record` with no project falls back to S13.
- Not verified live: engine-attached S05 meters/armed badges and post-create S27 (dev-surface spawn panel unreachable on this box's scaled display; parse/store logic is covered by vitest).

## NEEDS references (already in `docs/engine/NEEDS.md`, no edits made)

- §7: no arm/record/monitor/punch/count-in/metronome ops on the wire — S05 renders engine-reported `recording` state + real STOP only; controls are honest intent-only.
- §8: no `INPUT_DEVICE_LIST`/buffer/MIDI enumeration views — S15 shows `—` + reason (UI-T09).
- §17: no ingest/relink op — S27 drag-in copy states this instead of pretending a picker exists.

## Honesty notes

- No Figma example values, no seeded recents/takes/tracks, no invented capability flags anywhere in production UI.
- All wire int64 fields handled as decimal strings; project ids via `crypto.randomUUID()`; ops use tagged-union envelopes; receipts checked with `receiptOk` (APPLIED||DUPLICATE); errors surfaced verbatim.
