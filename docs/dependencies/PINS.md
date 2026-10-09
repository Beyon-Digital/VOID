# VOID dependency pins and rights (W01)

Recorded 2026-10-08 on `devin/void-implementation`. Rights columns reflect what
was verified from upstream licence texts this session; legal review for
distribution is **not** done — see "Rights state" per row.

| Component | Pin | Source | License (upstream) | Development use | Distribution rights |
|---|---|---|---|---|---|
| Tracktion Engine | `develop` @ `e7607547293fac421e1a9ec9898410c1893f570e` | https://github.com/Tracktion/tracktion_engine | GPL-3.0 / commercial (dual) | Permitted for internal development builds | **BLOCKED pending review** — GPL-3.0 would apply to distributed binaries unless a commercial licence is purchased (not approved, D06) |
| JUCE | engine-supplied submodule revision (`modules/JUCE` under the pinned engine commit; resolve via `git submodule` after clone) | https://github.com/juce-framework/JUCE | AGPL-3.0 / commercial (dual) | Permitted for internal development builds | **BLOCKED pending review** — same dual-licence situation; record resolved submodule SHA at first engine build |
| FlatBuffers (flatc + runtime) | `v25.9.23` (`03fffb25e2d777462b719cb4964249c30b19d58f`, tag object `edbe17738352418245d7228e7fd9f12c3ddc34c4`) | https://github.com/google/flatbuffers | Apache-2.0 | Permitted | Permitted (notice preservation) |
| Tauri | v2 (exact lockfile pins after `cargo add`) | https://github.com/tauri-apps/tauri | Apache-2.0 / MIT | Permitted | Permitted (notice preservation) |
| SQLite | system/bundled build recorded by `pnpm doctor` at runtime | https://sqlite.org | Public domain | Permitted | Permitted |
| Rust toolchain | rustc 1.97.1 (installed 2026-07-14 build) | rustup | Apache-2.0 / MIT | Permitted | Permitted |
| Node / pnpm | node 22.20.0 / pnpm 12.9.1 (repo pins `packageManager: pnpm@9.0.0` — mismatch recorded) | nodejs.org / pnpm.io | MIT | Permitted | Permitted |

## Notes

- Tracktion Engine upstream has no `main` branch; `develop` is the integration
  branch. The engine carries JUCE as a submodule (`modules/JUCE`) — the pinned
  pair is therefore (engine commit, its recorded JUCE submodule SHA). Build
  evidence lands in `docs/dependencies/QUALIFICATION.md` (written by the engine
  lane after the first upstream example build).
- Tracktion Engine officially supports macOS/Windows; Linux is not an
  officially supported target. Linux boxes in this workspace can still develop
  the coordinator/protocol/UI/persistence lanes; engine qualification happens
  on the macOS lane (and any Windows lane when spawned). A Linux engine build
  attempt may be recorded as evidence but cannot certify the supported targets.
- No licence purchase, relicensing, binary publishing, store submission or paid
  cloud enablement is in scope (handoff D06).
- Advisory review TODO for T04: check the SQLite version actually linked by
  `rusqlite`/`sqlx` once the lockfile exists (`pnpm doctor` reports it), and
  review OpenSSL/other bundled runtime deps at package time.
