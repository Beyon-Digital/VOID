# F0 evidence — VOID native foundation

Every claim here is backed by a real command + exit code run in this
repository. No inferred passes; blocked items name the missing resource.

| ID | Claim | Command | Result |
|----|-------|---------|--------|
| T02 | Scene slots group strictly by `sceneId`/`trackId`; ID-prefix inference removed | `pnpm --filter void-core test` (vitest run) | PASS (3/3) |
| T05 | Worker lifecycle: spawn → authenticated handshake → orderly shutdown ×3; non-responsive executable rejected | `cargo test -p void-worker --test supervisor_lifecycle` | PASS (2/2) |
| T05b | Wire contract round trip: PersistentCommand → mock worker → CommandReceipt APPLIED over UDS framing | `cargo test -p void-worker --test wire_roundtrip` | PASS (1/1) |
| — | Protocol schema compiles to Rust+C++ under pinned compiler | `tools/protocol-gen.sh` (flatc 25.9.23 required) | PASS |
| — | Doctor: rust 1.97.1 / flatc 25.9.23 / node 22.20 / pnpm 9 / cmake / ninja / c++11.4 / webkit2gtk | `tools/doctor.sh` | 8 pass, 2 warn (platform/engine-binary), 0 fail |
| — | Rust workspace builds + all unit tests | `cargo build --workspace --all-targets && cargo test --workspace --exclude void-tauri` | PASS, 0 warnings |
| — | Tauri shell compiles (void-tauri lib+bin) | `cargo build -p void-tauri` | PASS |
| — | Full Tauri native bundle: `void-tauri` release binary + .deb + .rpm | `pnpm --filter void-tauri exec tauri build` | PASS (2m04s) |
| — | void-tauri renderer vite build | `pnpm --filter void-tauri exec vite build` | PASS (197kB bundle) |

## Blocked on this platform (queued for macOS lane)

| ID | Blocked on |
|----|-----------|
| T03 engine build qualification | macOS/Xcode — Tracktion+JUCE compile |
| T13/T14 playback + offline render | engine worker (native/void-engine) |
| T15/T16 RT-safety + plugin render | engine worker + audio device |
| T22–T24 plugin scan/editors | native/void-plugin-scanner + plugin host |

## In flight (lane branches)

| Lane | Branch | Scope |
|------|--------|-------|
| A (macOS) | `devin/void-lane-engine` | engine qualification, native/void-engine, scanner, ENGINE_API_MAP.md |
| B (Linux) | `devin/void-lane-persistence` | void-project/void-assets/void-jobs + tests/recovery (T17–T21) |
| C (Linux) | `devin/void-lane-ui` | void-client DTOs, void-studio workspaces, void-ui a11y (T36–T39) |

## Known gaps (honest)

- WebAudioAdapter in packages/void-daw is a stub — never counts as audio.
- No renderer-driven command flow exercised end-to-end yet (engine lane).
- No checkpoint publication code yet (persistence lane in flight).
- Telemetry pump forwards frames but no meter/clock producer exists yet.
