# UIP3 evidence — Signal Studio screens S04 / S11 / S12

Lane: `devin/void-lane-uip3` (base `79ef7f22fe68142eb498677e911176ee77353db2` on `devin/void-ui`).

## What landed

- **`packages/void-studio/src/mixer/devices.ts`** — defensive `PLUGIN_LIST` → `DeviceItem[]` parser (`deviceFromSummary`, `deviceChainFromItems`, `devicesFromItems`). Unknown summary shapes drop out; failed/missing/quarantined/error flags surface verbatim; powered state is never assumed.
- **`packages/void-studio/src/export/preflight.ts`** — `runPreflight`: the real S11 preflight (UI-T29). Checks: engine attached, project content, routing reaches an output (real buses, or the engine's implicit master — send ops don't exist), referenced assets located, no failed/disabled devices, durable checkpoint recorded (`telemetry.save.checkpoint_id`), output range non-empty. Detached engine gates every content check to `unavailable` — nothing fakes a pass. Blocking failures stop submit; warnings don't.
- **S04 `apps/void-tauri/src/ui/studio/screens/mix/`** — real mixer: strips from `TRACK_LIST` via `stripsFromTrackItems`; `ChannelFader` (dB, −60..+6, `gainToDb`/`dbToGain` — engine units) + `ParameterKnob` (pan, `formatPan` L/C/R) + M/S buttons driving the mixer controller (`setGainDb`/`setPan`/`setMuted`/`setSoloed` → `sendWithStaleRetry`, one gesture = one fresh transaction id, receipts update the store — UI-T05). Live meters from `telemetry.meters` via `meterStrip` (idle ≠ decayed). Device chain per channel via `DeviceSlot`; "Open editor" sends the real `OpenPluginEditorOp` and quotes the rejection status when the native editor is unavailable (UI-T35). Left rail: real bus list from `routeModelFromTrackItems` + honest "sends not expressible" note; automation lane toggles + mode select from `mixAutomation` (view-state, labeled as such); selected-channel signal path + "delay compensation: not reported by the engine". Master column: real BUS strip fader/meter or the honest implicit-master note; peak readout from live frames only; "no loudness target is applied automatically". Compact ≤1280 collapses rail and master column behind `IconButton` toggles.
- **S11 `apps/void-tauri/src/ui/studio/screens/export/`** — export dialog backed by `exportView` (`createExportDialogStore`): WAV/MIDI selector gates audio-only fields (approved sample rates, bit depths, channels), tick-range fields validated as i64, output-name + tail policy bound to the spec model. Normalize/dither render **disabled with the reason** — no such fields exist in `ExportSpecDto` (no silent or double application). Preflight column runs `runPreflight` live over `TRACK_LIST`/`ASSET_LIST`/`CLIP_LIST`/`PLUGIN_LIST` + `telemetry.save`; each card shows status + observed detail. Export button → real `ExportContext` (uuid jobId, real projectId, real `checkpoint_id`, current revision, sha256s the asset view reported, tempo from `PROJECT_SUMMARY.bpm`) → `store.submit(ctx)` (build + validate spec). A valid spec is stored for S12 and the screen reports the honest gap: **no export-runner invoke exists on this shell** (see Gaps).
- **S12 `apps/void-tauri/src/ui/studio/screens/export-done/`** — reads `exportView.results`/`jobs` (only real `ExportResult` cards and `JobEvent` progress the engine supplies). On mount it issues the honest `EXPORT_LIST` read (`exportListRequest` shape) — the view is not in the schema, so the coordinator's `UNSUPPORTED`/error reply is quoted as the result-unavailability note. Success card shows real file/sha256/bytes/frames + spec context; failure state shows the job's reported `message`; empty state says "no completed export on record". Reveal-in-folder/audition are noted as unavailable (no filesystem surface).
- **Tests**: `export/preflight.test.ts` (7) + `mixer/devices.test.ts` (8) — all preflight blockers/passes and the parser's defensive cases.

## Gates (commands + exit codes)

| Command | Exit | Notes |
| --- | --- | --- |
| `pnpm -r --if-present test` | 0 | 43 files / 477 tests green (462 baseline + 15 new) |
| `pnpm --filter void-tauri exec tsc --noEmit` | 0 | screens + studio modules |
| `pnpm --filter void-tauri exec vite build` | 0 | production bundle OK |
| `pnpm -r --if-present build` | 0 packages / void-tauri vite OK | `tauri build` Rust phase fails ONLY at updater signing — `TAURI_SIGNING_PRIVATE_KEY` unset (pre-existing env gap, same as UI01 baseline) |

## Wired data sources

| Screen | Sources |
| --- | --- |
| S04 `mix` | TRACK_LIST → strips/routing; PLUGIN_LIST → device chains; `telemetry.meters`/`meterStrip`; mixer controller ops (gain/pan/mute/solo), `OpenPluginEditorOp`, `removeInsertOp`; `mixAutomation` feature store |
| S11 `export` | `exportView` dialog store → `buildExportSpec`/`validateExportSpec`; TRACK_LIST/ASSET_LIST/CLIP_LIST/PLUGIN_LIST/PROJECT_SUMMARY → `runPreflight`; `telemetry.save` checkpoint |
| S12 `export-done` | `exportView.results`/`jobs` (EXPORT_LIST read attempted — honest error quoted); `lastSubmit.spec` for spec context |

## Honest gaps (capability absent → shown, not faked)

1. **Export submit**: the Tauri invoke surface has no export/job command (`engine_status`, `spawn_engine`, `stop_engine`, `send_command`, `send_transport`, `read_view` only) and the wire schema has no render-submit op. A validated spec therefore cannot reach void-export — S11 reports "submission unavailable" verbatim and S12 reads the real (empty) result set. UI-T28's end-to-end render needs the integrator's runner bridge.
2. **Sends/aux routing**: no send/route op in the schema — `routeModelFromTrackItems` marks `sendsExpressible:false`; the rail says so rather than drawing fake sends.
3. **Automation persistence**: no automation-point wire ops — lanes are view-state "unsent intent" (labeled).
4. **Delay compensation**: the engine reports no latency field — the detail panel says "not reported by the engine".
5. **Normalize/dither/loudness**: not fields of `ExportSpecDto` — disabled rows, never applied.
6. **Plugin bypass**: no bypass op — `DeviceSlot.powered` reflects the reported flag only; removal is the documented path.
7. **EXPORT_LIST view**: not in `ViewKindName` — S12 issues the request with the honest view id and quotes the rejection.
8. **Reveal-in-folder / in-app audition / destination picker**: no filesystem/playback surface on this shell — noted inline.
