# UIP2 evidence — S01 Arrange flagship, S22 compact, S23 daylight

Lane: `devin/void-lane-uip2` (base `79ef7f22fe68142eb498677e911176ee77353db2` on `devin/void-ui`).

## What landed

`apps/void-tauri/src/ui/studio/screens/arrange/` evolved from the W09 editor reuse into the
S01 Figma layout (figma nodes `4:2`, `4:2108`, `4:2208`). Auto-discovered by the shell
registry — no shared-file edits.

| File | Role |
| --- | --- |
| `index.tsx` | Screen shell: three-column layout (library w224 / arrangement / inspector w264), compact drawers (`role="dialog"`, Escape-close), TRACK_LIST priming via `loadViewPage` on attach+project. |
| `data.ts` | Shared layer: lazy `ClipEditor`/`NoteEditor`/`arrangement`/`automation`/`instrumentBrowse` singletons (no new `studioStore` keys), `useTrackRows` (TRACK_LIST), `useTrackClips` (bounded CLIP_LIST slices keyed by view key + `parseClipItem`), `useTrackPlugins` (PLUGIN_LIST), `useSelectedClip`. |
| `SoundLibrary.tsx` | SOUNDS rail: search → `InstrumentBrowseStore.filterText`, instruments + presets from `BUILTIN_INSTRUMENTS`/`presetsFor`, double-click loads onto selected track through `loadInstrument` (one transaction → one undo), ASSET_LIST count for Imported assets, collections that the engine doesn't expose show `n/a`, "Create a sound" disabled with reason, audition card states preview playback isn't exposed. |
| `ArrangementPane.tsx` | Tools row (snap toggle + division cycle, automation-lane visibility for selected track, undo via `client.undo`, add MIDI track via `client.addTrack` w/ uuid + commandId, zoom → `setTicksPerPixel`), ruler row (TRACKS label, SectionStrip from `arrangementStore.sections` — honest empty state, `TimelineRuler` `onSeek` → `client.seek`, loop bar from transport cycle state), virtualized track rows (scroll-window slice ±1), wheel pan/zoom mapping to `setViewport`/`setTicksPerPixel`, `tempo_map_revision` → `setSnap` sync, per-track `AutomationStrip` from `visibleByTrack`. |
| `TrackLaneRow.tsx` | `TrackHeader` (select → `selectTrack`; M/S → `setTrackMute`/`setTrackSolo` with commandId) + clip lane: bounded CLIP_LIST read (viewport ±8 bars), `layoutClips`, culling, `TimelineClip` audio/midi variants, drag gestures identical to W09 (`beginClipDrag` → `previewClipDrag` → optimistic `setDrag` → `commitDrag` → view re-read), split-armed mode, Delete/`s` handlers, additive selection (`selectClips`/`toggleClip` + `selectClip`/`selectTrack`). |
| `DeviceDock.tsx` | Instrument / Piano roll / Gesture modes; identity card from `descriptorByUid`; 4 `ParameterKnob`s from `descriptor.params.slice(0,4)` with `paramValues` fallback `defaultValue`, `onCommit` → `setInstrumentParam` (clamped, transactionId); DEVICE CHAIN via `DeviceSlot` (powered ← `bypassed`, remove → `removeInstrument`); piano-roll embeds the real `PianoRollPane` when selection is a MIDI clip, otherwise an honest prompt; gesture mode redirects to `#/compose`. |
| `RegionInspector.tsx` | REGION kind badge, clip name, "Bars X–Y · track"; TIMING & FEEL with `ValueEntry`-editable Start (moveClip) / Length (trim-end commit) in bars → ticks through `barTicks`; Quantize mirrors `snap.division`; Strength/Swing/PITCH/Output rows render `—` with reason tooltips (not on the wire); SIGNAL Instrument from PLUGIN_LIST slot 0; "Continue phrase" → `#/compose` (selection follows via `studioStore`). |

## Screen id → wired data sources

- **S01** (`#/arrange`): TRACK_LIST, CLIP_LIST (bounded slices), PLUGIN_LIST, ASSET_LIST, PROJECT_SUMMARY (header/health via shell), `transportBarState` cycle, `arrangementStore` sections/markers, `automationStore` lanes, `instrumentBrowseStore`, `viewport`/`snap`/`clipSelection` — all through `studioStore`/`editorStore` + real client commands (`addTrack`, `setTrackMute/Solo`, `undo`, `seek`, `commitDrag`, `moveClip`, `removeClip`, `loadInstrument`, `setInstrumentParam`, `removeInstrument`).
- **S22**: same screen at ≤1280px (`useStudioCompactContext`): library + inspector collapse to drawers from tools row, transport + selection stay visible.
- **S23**: daylight theme — all colors via `tokens.*` CSS vars; verified through header ☀/☾ toggle.

## Gates (commands + exit codes)

| Command | Exit | Notes |
| --- | --- | --- |
| `pnpm install --frozen-lockfile` | 0 | pre-edit |
| `pnpm -r --if-present build` (baseline + final) | 0 packages / 1 void-tauri | `tauri build` fails ONLY at updater signing — `TAURI_SIGNING_PRIVATE_KEY` unset (pre-existing env gap, same as UI01; `vite build` + cargo compile succeed) |
| `pnpm -r --if-present test` | 0 | 462 vitest green |
| `npx tsc --noEmit` (apps/void-tauri) | 0 | strict bundler TS |
| `pnpm --filter void-tauri exec vite build` | 0 | ~415 kB bundle |
| `cargo test --workspace --exclude void-tauri` | — | no rust touched |
| Visual: `vite dev :5175` + `#/arrange` | ✓ | rendered with view-cache projection data in plain Chrome; drag/seek command round-trips not exercised (no live engine session) |

## NEEDS entries (honest gaps)

- **Section ops**: `SectionStrip` renders `arrangementStore.sections` (empty by default) with "no sections — section ops aren't in the wire yet" — no create/rename/resize command exists in `void-client`.
- **Sound collections**: Favorites / Recently used / My recordings show `n/a` — not exposed by the engine.
- **Sound audition**: preview playback isn't exposed; card states this and offers "Load on \<track\>" (real `loadInstrument` write) instead.
- **Region PITCH / Strength / Swing / Output**: `—` with tooltips — not in the CLIP_LIST projection or command surface for region clips.
- **Per-device editor**: "Open editor" disabled — no rack/editor surface exists to route to; params edit via the 4 knobs below.
- **Automation button**: only toggles visibility for lanes already recorded in `automationStore.visibleByTrack` for the selected track — arm/record ops aren't in the wire.
- **Live round-trips**: clip drag-commit, seek, M/S, undo go through the real command surface but were verified statically (tsc/vite) + rendered — mock worker stubs `read_view` (`{"kind":"mock"}`) so a fully live lane run needs a real engine session.
- **Peak-LOD hooks (UI-T30)**: lane culls off-viewport clips; waveform/note LOD beyond the `TimelineClip` variants is not implemented (component has no LOD prop yet).
