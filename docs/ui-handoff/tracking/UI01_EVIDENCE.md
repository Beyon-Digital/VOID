# UI01 evidence — Signal Studio design-system foundation

Lane: `devin/void-lane-ui01` (base `e3f18a1540c1d9fb2be20c150a4144a843c3773c` on `devin/void-ui`).

## What landed

- **Theme**: `packages/void-ui/theme/tokens.css` = verbatim port of `docs/ui-handoff/VOID_Tokens.css` (dark default on `:root`/`[data-void-theme="dark"]`, `[data-void-theme="daylight"]` override, `.void-studio` scope, `.void-type-*` scale, focus-visible rule, reduced-motion). `theme/legacy.css` maps pre-Signal var names (`--void-border`, `--void-surface-raised`, `--void-text-primary/secondary/muted`, `--void-warn`, `--void-ok`, `--void-radius`) so existing components keep working. `theme/voidTheme.ts` embeds the same CSS for JS injection; `theme/useVoidTheme.tsx` provides `VoidThemeProvider` (injects stylesheet, sets `data-void-theme`, persists choice to localStorage) + `useVoidTheme()`. `components/styles.ts` `tokens` map now references Signal vars with dark-theme fallbacks.
- **13 canonical families** in `packages/void-ui/components/`: `ActionButton`, `IconButton`, `WorkspaceTab`, `StatusBadge`, `Field` (+`TextInput`), `TrackHeader` (existing, token-driven), `TimelineClip` (audio/midi/ghost/selected in ONE component), `ParameterKnob`, `ChannelFader` (continuous), `DeviceSlot`, `AssetRow`, `SceneCell` (existing phases), `Toast` (+`useToasts`/`ToastViewport`). Legacy names `Button`, `Knob`, `Fader`, `ClipBlock` are adapter re-exports — every existing callsite keeps compiling.
- **Behavior core**: `components/editing.ts` (clamp, arrow/page/home/end keys, fine-adjust Shift modifier, numeric entry parse/format, gesture session → exactly one `{from,to}` commit, `EscapeStack` for Escape-priority, editable-target + IME guards) + `components/usePointerGesture.ts` (pointer capture, pointercancel/lostpointercapture still commit once, Escape cancels mid-gesture). Tested by `components/editing.test.ts`.
- **Shell** (`apps/void-tauri/src/ui/studio/`): `StudioShell.tsx` is the new default route (`main.tsx` renders it). `StudioHeader` (project name from `PROJECT_SUMMARY`, saved-state from `telemetry.save`, 5 workspace tabs, library/jobs/setup/export affordances, theme + dev-surface toggles). `TransportBar` (real play/stop/panic/cycle via client, musical loop range via `formatBarBeat`, tempo from `PROJECT_SUMMARY.bpm`, status badge, export). `HealthBar` (engine attach, audio sample rate, save outcome — no fabricated readings). `registry.ts` auto-discovers `screens/<id>/index.tsx` via `import.meta.glob` — later lanes add screens by adding a directory, zero shared-file edits. Missing ids → `ScreenPlaceholder` naming the id. `screens/{arrange,compose}` reuse the real `TrackListColumn`/`TimelinePane`/`PianoRollPane`; `screens/mix` keeps the existing honest placeholder; `screens/dev-gallery` renders all families × variants in both themes at `#/dev/gallery`. Compact ≤1280px collapses the track column into a drawer (`useStudioCompact` + Escape-closeable dialog). Existing `dashboard.tsx`/`support.tsx` remain fully reachable via the "Developer surface" toggle (old `StudioShell` intact).
- **Workspaces** (`void-studio`): `WORKSPACE_ORDER` is now the Signal five `arrange, compose, mix, perform, visuals` (Ctrl/Cmd+1..5).

## Gates (commands + exit codes)

| Command | Exit | Notes |
| --- | --- | --- |
| `pnpm install --frozen-lockfile` | 0 | pre-edit |
| `pnpm -r --if-present build` (baseline) | 0 packages / 1 void-tauri | `vite build` + cargo release + bundling succeed; `tauri build` fails ONLY at updater signing — `TAURI_SIGNING_PRIVATE_KEY` unset (pre-existing env gap on `devin/void-ui`, bundles produced) |
| `pnpm -r --if-present test` (baseline) | 0 | 462 vitest green before edits |
| `pnpm --filter void-ui build` | 0 | tsc |
| `pnpm --filter void-ui test` | 0 | 22 tests (editing.test.ts 21 + ticks 1) |
| `pnpm --filter void-studio build && test` | 0 | 462 tests green (workspaces.test updated for 5 workspaces) |
| `npx tsc --noEmit` (apps/void-tauri) | 0 | strict bundler TS |
| `npx vite build` (apps/void-tauri) | 0 | 275 modules |
| `cargo test --workspace --exclude void-tauri` | — | no rust touched |
| Visual: `vite` + `#/dev/gallery` | ✓ | both themes rendered side-by-side; accessible names confirmed in DOM |

## NEEDS entries (honest gaps)

- **Playhead in BARS.BEATS.TICKS**: `ClockSnapshot` ships `timeline_sample` (samples) and loop bounds in ticks, but no playhead-tick field — tick conversion needs the tempo map engine-side. TransportBar shows the truthful `pos <n> samples` + loop `B.bb.qq → B.bb.qq`. NEEDS: `position_ticks` (or tempo-map read view) on the wire → S01/S04 transport readout.
- **Key/meter readout**: `SetTimeSignatureOp` exists but no view exposes key signature or meter → TransportBar renders `—`. NEEDS: key + meter fields on `PROJECT_SUMMARY` (or a METER view).
- **CPU%/block-size in HealthBar**: only `sample_rate` is on `ClockSnapshot`. NEEDS: engine cpu/load + buffer-size telemetry.
- **Compact drawer visual check**: the ≤1280px track-column drawer is code-verified (`useStudioCompact`, Escape-closeable) but needs a live project for a screenshot pass.
- **`tauri build` signing**: `TAURI_SIGNING_PRIVATE_KEY` absent in this environment; bundles produced, only the updater-signature step fails. Pre-existing on base.
- **SCREENS.json**: untouched — this lane owns the shell chrome and routing, not any of S01–S28's screen bodies.
