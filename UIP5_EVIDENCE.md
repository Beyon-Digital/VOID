# UIP5 evidence — Signal Studio screens S06, S07/S28, S18, S19, S21

Lane: `devin/void-lane-uip5` (off `devin/void-ui`).
Screens live under `apps/void-tauri/src/ui/studio/screens/` and are
auto-discovered by the file-convention registry — hash routes
`#/comp`, `#/perform`, `#/plugin-recovery`, `#/engine-recovery`,
`#/save-recovery`. New shared helpers sit in `screens/shared/` (no
`index.tsx`, so the registry never registers them).

## Screen → wired data sources

| Screen | Route | Wired sources |
| --- | --- | --- |
| S06 Comp | `#/comp` (`screens/comp`) | `void-studio/src/takes` store (folders/comps/selection/audition/lastApply), `planCompApply` + `applyCompPlan` → `sendWithStaleRetry` (one transactionId per commit), `client.undo(transactionId)`, CLIP_LIST for target clips, TRACK_LIST for lane headers |
| S07 Perform + S28 | `#/perform` (`screens/perform`) | TRACK_LIST (columns), CLIP_LIST (slot content), `void-studio/src/scenes` store (grid/launch/quantize/meters), `telemetry.clock` for the settlement boundary, `client.panic()`/`stop()`/`seek()`/`setCycle()` |
| S18 Plugin recovery | `#/plugin-recovery` (`screens/plugin-recovery`) | PLUGIN_LIST → `exchange/pluginList.parsePluginSlots` (`{instanceId,name,status,reason}`), RemovePluginOp + InsertPluginOp via `sendWithStaleRetry`, TRACK_LIST for the replace-target picker |
| S19 Engine recovery | `#/engine-recovery` (`screens/engine-recovery`) | `studioStore.engine`, `void-studio/src/recovery` store (engine-lost note, last durable save, last spawn path), `spawnEngine` + `engineStatus` |
| S21 Save recovery | `#/save-recovery` (`screens/save-recovery`) | `telemetry.save` (latest SaveResultEvent), recovery store (last SAVE_DURABLE vs last SAVE_FAILED), `client.saveProject`, CreateCheckpointOp, `studioStore.revision` for the unsaved-edit delta |

New package surface (additive, deep-imported — barrels untouched):

- `packages/void-studio/src/recovery/` — zustand store retaining the last
  SAVE_DURABLE event, last SAVE_FAILED event, engine-lost note and last
  spawn executable, plus `bindRecovery(store, client)`. Bound lazily by
  the recovery screens via `screens/shared/recovery.ts`.
- `packages/void-studio/src/exchange/pluginList.ts` — defensive parser
  for the engine's PLUGIN_LIST summary shape.

## Behavior contracts honored

- **UI-T10/T11 (S06)** — comp edits go through `planCompApply` +
  `applyCompPlan`: all Remove/Insert ops under one `transactionId`;
  undo is exactly `client.undo(lastApply.transactionId)`. Source takes
  are never mutated — the folder is untouched by commit. `validateComp`
  runs before planning; failures surface `describeReceiptError`.
- **UI-T21 (S07/S28)** — a queued scene stays `pending` until the native
  clock crosses the quantized boundary (`settleLaunch` only fires from
  `ClockSnapshot` ticks, converted via project bpm); with no clock the
  queue stays visibly pending. Region-bound scenes send the real
  `sceneTransportOps` (SEEK + SET_CYCLE) at the boundary.
- **UI-T22 (S07)** — Panic is `sendTransport('PANIC')`: acknowledged by
  send, never queued behind edits, always responsive.
- **UI-T25 (S18)** — missing/bypassed slots render straight from
  PLUGIN_LIST; saved state + routing retention is stated because the
  engine keeps the slot; replace/remove/locate are explicit actions and
  locate is marked unavailable (no rescan command on the wire).
- **UI-T26/T31 (S19)** — "Playback has stopped" is stated; restart goes
  through `spawn_engine` and returns stopped (no auto-play); checkpoint
  info comes only from delivered SAVE_DURABLE events; engine state is
  renderer-independent (event channel, not WebView timers).
- **UI-T27 (S21)** — "Saved" appears only on a reported SAVE_DURABLE; a
  failed write keeps the in-memory song and never overwrites the last
  good checkpoint record.

## Honest gaps (NEEDS, never faked)

- **Take folders (S06)** — protocol major.1 has no TAKE_LIST view; folders
  reach the take store from the recording/caller side. With none present
  the screen shows a truthful empty state — no seeded takes.
- **Scene definitions (S07)** — no SCENE_LIST view and no scene ops on the
  wire; scenes are session view-state (`createSceneStore`). "New scene"
  binds a region only to the live cycle range. Per-slot clip triggering
  is not wired — the launch maps to SEEK/SET_CYCLE for region scenes.
- **Slot→clip assignment (S07)** — cells assign real CLIP_LIST clip ids as
  session content (no persistence op exists for slot content).
- **Audition (S06)** — `auditionTakeId` is display state (store documents
  the preview engine layer as NEEDS §12); the lane marks the auditioning
  take in its label instead of pretending to play audio.
- **MIDI comp segments (S06)** — `compToOps` inserts empty MIDI clips;
  the commit result line reports `midiSegmentClipIds` explicitly.
- **Locate/rescan (S18)** — no rescan command exists on the control
  surface; the button is disabled with the reason shown.
- **Bypass toggle (S18)** — PLUGIN_LIST reports BYPASSED; there is no
  bypass command, so the screen shows the state without a fake toggle.
- **Save destination (S21)** — no save-as/destination op; "choose another
  location" is guidance plus a disabled control, not an invented picker.
- **Track for missing plugin (S18)** — PLUGIN_LIST missing rows carry no
  trackId, so Replace asks the user to pick the target track explicitly.

## Gates (run on this branch)

- `pnpm --filter void-studio build` — clean.
- `pnpm --filter void-studio test` — 43 files / 471 tests passed
  (includes new `recovery` + `pluginList` suites).
- `pnpm --filter void-tauri exec tsc --noEmit` — clean.
- `pnpm --filter void-tauri exec vite build` — clean (285 modules).
- `pnpm -r --if-present test` — exit 0 (all packages incl. new suites).
- `pnpm -r --if-present build` — every package builds; the recursive run
  exits 1 only at `void-tauri`'s final updater-signing step
  (`A public key has been found, but no private key` — the repo's
  `tauri.conf.json` carries a placeholder pubkey and
  `TAURI_SIGNING_PRIVATE_KEY` is not provisioned on this machine).
  vite build, cargo release compile, and all bundles (deb/rpm/AppImage)
  complete; `tauri build --no-bundle` exits clean. Environmental,
  identical at base SHA.
- Live UI check — release binary launches, LauncherPanel and hash
  routing verified on X (:0). Screens could not be driven end-to-end
  here: they mount behind the project's engine-attached + project-open
  gate, and the dev-surface Dashboard that spawns the mock worker sits
  below a non-scrolling fold in the legacy shell (pre-existing app-shell
  issue, unrelated to this lane). Detached/empty states render honestly
  by construction; data bindings are exercised by the package tests.
