# VOID updater and signing (T28/W07 — wiring half)

**Status:** wired, unsigned. The Tauri updater plugin is registered and the
config is complete, but no real key material exists yet — signing keys are
authorization-gated (handoff D06 + `docs/ops/NEEDS.md` OPS-N01). Nothing here
claims a working signed-update path.

## What exists today

| Piece | Where | State |
|---|---|---|
| `tauri-plugin-updater` Rust dep + registration | `apps/void-tauri/src-tauri/Cargo.toml`, `src/lib.rs` | compiled on Linux headless (`cargo check -p void-tauri`, exit 0) |
| `bundle.createUpdaterArtifacts = true` | `apps/void-tauri/src-tauri/tauri.conf.json` | bundler will emit `.sig` + archived updater payloads **only when** `TAURI_SIGNING_PRIVATE_KEY` is present at build time; without it the build still succeeds unsigned |
| Updater endpoint | `tauri.conf.json → plugins.updater.endpoints` | `https://github.com/Beyon-Digital/VOID/releases/latest/download/latest.json` — the manifest `tauri-action` generates once signing is on |
| Pubkey | `plugins.updater.pubkey` | **PLACEHOLDER** — valid base64 that decodes to a labelled non-key string. Replaced by the real public key at first key generation (below). An unverifiable update check fails closed (minisign verify error), never silently installs. |
| Capability grant | `capabilities/default.json` | `updater:default` = check + download + install + download-and-install for the `main` window |
| Windows install mode | `plugins.updater.windows.installMode` | `passive` — quiet NSIS/MSI relaunch install |
| Workflow env wiring | `.github/workflows/release.yml` | `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` passed from secrets into `tauri-action` |

## Key generation (one-time, authorization-gated)

Requires tauri CLI signer support (bundled in `@tauri-apps/cli` / `cargo tauri`):

```bash
# On a key holder's machine — NOT CI. Generates:
#   ~/.tauri/void-updater.key        (private key — secret, never commit)
#   ~/.tauri/void-updater.key.pub    (public key — embed in tauri.conf.json)
pnpm --filter void-tauri exec tauri signer generate -w ~/.tauri/void-updater.key
```

Then:

1. `tauri.conf.json → plugins.updater.pubkey` ← contents of `void-updater.key.pub`
   (a `RW...` base64 minisign public key).
2. Repository secrets (organization):
   - `TAURI_SIGNING_PRIVATE_KEY` ← contents of `void-updater.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` ← the password set at generation.
3. On the next `v*` tag, `release.yml` produces signed `.sig` updater
   artifacts and a signed `latest.json` on the draft release.

`tauri signer generate` without `-w` also prints the pair to stdout — store it
the same way. Keep the private key out of the repo, out of logs, out of the
draft-release body.

## What still needs doing (T28 runtime half — NEEDS OPS-N02)

- Real key generation + secret provisioning (owner action).
- Tampered-payload rejection drill: modify a signed bundle post-publish and
  confirm the updater rejects it.
- Interrupted-update recovery: cut network mid-download, confirm the app stays
  on the old version and retries cleanly.
- **Install/restart blocking during recording/performance** — the plugin
  currently has no hook into session state; T28 expects an update to refuse
  mid-take. Needs a coordinator-side "in session" flag the updater path must
  honour (NOT implemented — do not claim it).
- OS-level signing/notarization (separate from updater signing): Apple
  Developer ID + notarize for `.app`/`.dmg`, Authenticode for `.msi`/`.nsis`.
  Both gated on certificates VOID does not hold (D06).
