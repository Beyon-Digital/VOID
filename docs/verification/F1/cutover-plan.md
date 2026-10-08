# F1 — Electron cutover plan (T47) · status: staged, not executed

**Rule (KICKOFF/CONTRACTS):** Tauri shell beside Electron; Electron preserved until
native equivalence for implemented workflows is demonstrated. Removal happens only
after the F1 native gate passes (T44 first-song + T45/T46 workload evidence).

## Current state (2026-10-08)

| Shell | Path | Status |
|---|---|---|
| Electron (legacy) | `apps/void-desktop` (electron-vite; main + preload + renderer) | Preserved in tree. Still the declared "app container" in README. `pnpm dev`/`build` at repo root route through turbo into it. |
| Tauri (target) | `apps/void-tauri` (Tauri v2, Rust shell + WebView) | Builds on Linux: `pnpm -r build` → deb + rpm + AppImage (verified). Full JSON↔FlatBuffer codec, dispatcher, ≤30Hz telemetry pump, dashboard + StudioShell. |
| Engine | `native/void-engine` (Tracktion/JUCE UDS worker) | macOS-qualified; all 32 ops + transport + save + render + recording internals. |

## What remains before Electron can be removed

1. **T44** complete-offline-song journey green on the Tauri shell (macOS; live audio device required — headless boxes can't certify).
2. **T45/T46** workload + memory-churn evidence on the Tauri shell.
3. **WebAudio production routing:** confirm no production path schedules audio through WebAudio (dev/mock path is allowed and does not count as "implemented audio").
4. **Legacy project migration:** legacy `.void`/Electron-sidecar project formats need either a tested importer or documented compatibility limits (preserve git history; keep the importer if actually needed).
5. Then: delete `apps/void-desktop` default-startup wiring (root `dev`/`build` scripts, `electron-builder.config.ts`, CI references), keep source in history.

## Files that flip at cutover

- `package.json` root scripts: `dev`/`build` turbo targets → `void-tauri` (drop electron-vite pipeline).
- `electron-builder.config.ts` → remove.
- `README.md` architecture section → Tauri is the production path (interim note added 2026-10-08).
- `.github/workflows/ci.yml` → drop any electron-specific jobs (none added yet; native loop is per-project cmake).
- `apps/void-desktop/` → removed from tree at cutover commit (history preserved).

## Honest status

- Tauri equivalence is demonstrated for: command dispatch + receipts, telemetry, save→SAVE_DURABLE plumbing, studio workspaces (compose/arrange/mix), export UI → ArgvRenderer provenance, instruments/mixer surfaces.
- NOT yet demonstrated: live audio-device playback through Tauri (headless), plugin editor windows, long-session workload, real first-song journey.
- Conclusion: cutover staged; Electron stays until the gate above is green.
