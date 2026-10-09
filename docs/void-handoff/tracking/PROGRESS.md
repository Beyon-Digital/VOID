
## 2026-10-09 00:07 UTC — Lanes P (W18), R (W20), S (W21) merged

- **W18 mixer/automation/MIDI** (`devin/void-lane-w18` @ 77ab63c): `crates/void-mix` routing graph (sends/aux/multi-out/external-IO/VCA, named loop rejection, layout legality matrix, per-node delay compensation), studio automation modes + midi/mpe/articulation/step. tests/mix 14/14; T69–T71 pass_linux.
- **W20 exchange/registry** (`devin/void-lane-w20` @ 59f0a80): `crates/void-exchange` ~5.4k lines — `.dawproject` ZIP+XML roundtrip byte-deterministic + zero-loss (T77), `void-plugin-registry/1` + `void-missing-plugins/1` preserved-blob rehydrate verdicts (T76 spec-side), `IsolationPolicy` validation (T75 spec-side). tests/exchange 12/12.
- **W21 producer** (`devin/void-lane-w21` @ b07c972): `crates/void-producer` deterministic accompaniment/inpaint/vary (T78), BS.1770 loudness + mastering plan (T79), stem-batch export + producer gate (T80); shortcuts + screensets stores. tests/producer 12/12.
- Post-merge: `cargo test --workspace --exclude void-tauri` exit 0; vitest 405 (studio 377); studio+ui tsc clean.
- Also fixed: PR #42 ts job — `pnpm -r build` reached `apps/void-tauri`'s `vite build && tauri build` which needs WebKitGTK; excluded void-tauri from recursive build, kept `vite build` + `tsc --noEmit` coverage steps.

## 2026-10-09 00:10 UTC — Lane Q (W19) merged

- **W19 content/sound-library** (`devin/void-lane-w19` @ fc49c3c): `crates/void-content` (sha256 manifest verify, zones/round-robin, preload/stream/offline + bounded VoiceAllocator, install→quarantine→relink lifecycle, stock inventory + license ledger LGR-0001..04), `packages/void-studio/src/audio-edit/` (transients/varispeed/pitch/tails op plans). 72 authored stock descriptors. tests/content 8/8, void-content 18/18; T73 pass_linux, T74 model-side (engine render NEEDS).
- Post-merge: workspace cargo green; studio vitest 400; builds clean.

## 2026-10-09 00:42 UTC — Lane W (W26+W27) merged

- **W26 spatial** (`devin/void-lane-w26w27` @ 7057034): `crates/void-spatial` mono→22.2 layout ladder (BS.2493 table, Illegal/RequiresDeclaredDownmix matrix), ADM objects (118 cap), calibrated monitoring + declared fallback, SpatialGate gated on ValidatorRecord. T92/T93 model-side.
- **W27 sync/show** (same branch): `crates/void-sync` byte-exact MTC (full SysEx + quarter-frame demux), 24ppqn clock math, full MMC vocabulary + SHUTTLE packing, single-master arbiter w/ drift + conflict resolution, OSC literal/pattern matching, DMX universes + 10Hz strobe ceiling, Ableton Link honest NotImplemented ADR. Studio: cue list + pairing/arming + idempotent PANIC, bounded mapping engine (MAX_CHAIN_STEPS=8). T94–T96 pass_linux.
- tests/spatial 17/17, tests/sync 22/22; workspace + clippy clean; studio vitest 419.

## 2026-10-09 00:51 UTC — Lane X (W28) merged

- **W28 wasm sandbox** (`devin/void-lane-w28` @ 43ac142): `crates/void-wasm` real wasmtime 37 restricted host — capability grant set + ambient deny (single void_host.log surface), manifest v1 + DeclaredLimits, sha256+import-scan spec vetting, void-wasm/1 ABI (24B NoteEvent), fuel/epoch deadline + CancelToken, ResourceLimiter, sha256-verified versioned registry w/ revoke+LKG, ARA/plugin-export evidence-only gates. tests/wasm 31/31 (12 real WAT→wasm fixtures incl. adversarial). Studio extensions surface (view-state, snake_case i64 DTOs). T97 pass_linux, T98 gates-side.
- Workspace clippy -D warnings clean; studio vitest 428.
