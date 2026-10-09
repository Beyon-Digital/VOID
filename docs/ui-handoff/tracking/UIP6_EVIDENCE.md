# UIP6 evidence — S09 Visuals / preview and program (figma 4:808)

Lane: `devin/void-lane-uip6` branched from `devin/void-ui` @ `79ef7f22fe68142eb498677e911176ee77353db2`.
Screen: `apps/void-tauri/src/ui/studio/screens/visuals/` (route `#/visuals` — the Visuals workspace tab).

## What is real

- **Preview monitor** — renders the REAL preview-channel layer stack from `visualsStore` (order, kind, blend, opacity, visibility) as a labeled compositing projection, plus last-frame identity (`frameSha256`, `renderUs`, `timelineSample`, `clockSequence`) and the drop-counter strip verbatim. Empty state names the missing snapshot feed. No pixels are ever held or fabricated by the webview.
- **Program monitor + output state machine** — full contract vocabulary: `unavailable | previewOnly | displaySelected | armed | live | blackout`. Output starts DISARMED (UI-T23): `Choose display` → explicit target pick; `Arm output` disabled until a target is chosen; changing the target drops the arm (`selectTarget` resets `armed`). `live` requires a real `lastFrame.program` — it cannot be faked by view state. `blackout` is always reachable (inspector + bottom bar) and overrides every state.
- **Display targets** — only `offscreen` (readback render — the exercised engine path) is selectable; `window`/display output is listed as unavailable with the deferred-winit reason.
- **Visual timeline** — layer trim windows on the shared tick ruler (`TimelineRuler`, `TICKS_PER_QUARTER`); seeking rides the real `send_transport/SEEK` path.
- **Inspector** — real layer fields (blend, opacity, quantize, trim, asset/preset ids); ParameterKnobs show real scale/opacity/rotation values, read-only with the honest reason (no visual command channel).
- **AV export dialog** — real `AvExportDraft` from real state only: `checkpointId`/`manifest_sha256` arrive via `SaveResultEvent` telemetry after a real `CreateCheckpointOp` send (button in dialog); `jobId` minted once per open; inputs are sha-pinned asset blobs filtered from the real `ASSET_LIST` read; codec picker is gate-driven (`avCodecGate` verbatim reasons). `Launch export` runs `buildSpec()` — the real validator — and reports the missing submit seam instead of animating a fake export.
- **Clock honesty** — `ClockSnapshot` telemetry (native audio timing) renders verbatim in the footer ("Audio clock → visual frames. Never the other way around."). No JS clock schedules anything.
- **Both themes** (token CSS vars only) and **compact ≤1280** (library + inspector collapse to drawers, monitors/timeline stay mounted, blackout stays visible).

## Honest gaps (named in the UI, not worked around)

| Seam | Status | Where the UI says it |
|---|---|---|
| `voidvis` command channel (SetOutputRoute/AddVisualLayer/… ops) | NOT in shipped `PersistentOp` union; no Tauri invoke | `REASON_NO_CHANNEL` — asset attach note, read-only knobs, armed-route note |
| Visual snapshot / frame-meta feed | No view kind or telemetry member for visuals | `REASON_NO_SNAPSHOT` — preview empty state + armed reason |
| Display enumeration / winit surface | Deferred per `docs/visual/README.md` | `REASON_NO_DISPLAY_ENUM` — window target row |
| `submit_job` persistent op | Protocol gap (same as W12 jobs surface) | `REASON_NO_JOB_SUBMIT` — generate + export launch notes |
| `ffmpeg -encoders` probe | No invoke | codec gates stay closed with `gate.reason` verbatim |
| Asset ingest op | NEEDS.md §rev2 — `AttachAssetOp` only registers in-container blobs | `REASON_NO_INGEST` — Import visual button |
| `AV_EXPORT_LIST` view | Protocol gap (av/results.ts) | results list renders only if the view lands |
| Audio-reactivity bindings | `analysis.bindings` is a proposed contract, no store | honest "no bindings" section |

## Files

- `screens/visuals/index.tsx` — layout, title/status badge, compact drawers, blackout + export actions, bottom honesty bar
- `screens/visuals/model.ts` — pure logic: output state machine, layer/asset projection, clock lines, seam constants (no DOM deps)
- `screens/visuals/stores.ts` — `visualsStore`/`visGenStore`/`avPanelStore`/`outputIntentStore` singletons + hooks + ASSET_LIST feed
- `screens/visuals/LibraryColumn.tsx` — visual assets (ASSET_LIST→`parseAssetItem`→image/video) + layer list + import/back
- `screens/visuals/PreviewMonitor.tsx` — preview stack projection + frame meta + drop counters
- `screens/visuals/ProgramMonitor.tsx` — output state machine UI, target picker, arm/disarm, failure surfacing
- `screens/visuals/VisualTimelinePane.tsx` — layer rows + ruler + real seek
- `screens/visuals/InspectorPane.tsx` — layer fields/knobs, audio-reactivity honesty, generation card, blackout
- `screens/visuals/AvExportDialog.tsx` — real checkpoint create + draft + gated codec + validated launch attempt
- `screens/visuals/model.test.ts` — 19 vitest cases (state machine, projections, honesty lines)
- `apps/void-tauri/vitest.config.ts`, `apps/void-tauri/package.json` — `test`/`typecheck` scripts + vitest devDep (additive; app had no test runner)

## Wired data sources

| Surface | Source |
|---|---|
| Layer stacks, channels, transitions, outputs, frames, counters | `createVisualsStore()` projections (`applySnapshot`/`noteFrame`/`noteCounters` when the feed lands) |
| Visual assets + AV input lists | `ASSET_LIST` read view → `parseAssetItem` (relink model) |
| Engine clock / transport / loop | `telemetry.clock` (ClockSnapshot) via `studioStore` |
| Checkpoint for export | `CreateCheckpointOp` → `SaveResultEvent` (`checkpoint_id`, `manifest_sha256`) |
| Export spec + codec gates | `createAvExportPanelStore` (`buildSpec`, `avCodecGate`, `avFramePlan`) |
| Generation records | `createVisGenStore` (empty until job events exist) |
| Seek | `send_transport` SEEK (shipped op) |

## Gates

| Gate | Result |
|---|---|
| `pnpm -r --if-present build` | exit 1 — all packages green; fails only at `tauri build` updater signing (`TAURI_SIGNING_PRIVATE_KEY` not provisioned — environment gap, pre-existing on the base SHA) |
| `pnpm -r --if-present test` | exit 0 — 462 existing + 19 new (void-tauri `model.test.ts`) |
| `pnpm --filter void-tauri exec tsc --noEmit` | exit 0 |
| `pnpm --filter void-tauri exec vite build` | exit 0 — 284 modules, 422 kB bundle |
| `cargo test --workspace --exclude void-tauri` | not run — no rust touched |

## Acceptance mapping

- **UI-T23** — output disarmed until explicit target+arm (model.test.ts asserts every transition); changing the target re-disarms; blackout always visible; preview never goes live by itself (live requires `lastFrame.program`).
- **UI-T24** — failure honesty: engine-detached → `unavailable`; armed without frames → `armed` (not live); dropped frames/clocks/skipped renders surface in orange while armed; blackout/fallback visible; no pixel or progress is ever fabricated.
