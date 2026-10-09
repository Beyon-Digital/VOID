# VOID install / verify / run — per OS (T27/W07 artifact half)

This documents the end-user install path for bundles produced by
`.github/workflows/release.yml`. The **clean-machine install test itself has
not run** — T27 requires installing on a VM without dev tools and launching
offline; that is recorded open in `docs/ops/NEEDS.md` (OPS-N06).

> **Unsigned-build notice:** every artifact below is unsigned. Browsers,
> Gatekeeper and SmartScreen will warn. These builds are internal-testing
> only until signing/notarization lands (handoff D06, OPS-N01).

## What the installer gives you — and what it does not

The Tauri bundle installs the **VOID Studio shell only**: the WebView UI +
Rust coordinator (`void-app`) + process supervisor (`void-worker`). The
**engine worker binary is not bundled** (no `externalBin`/`resources` in
`tauri.conf.json` — the app spawns an engine executable the UI points at via
`spawn_engine`). Until engine packaging lands (OPS-N07), a clean machine also
needs a `void-engine` binary built on a machine with the C++ toolchain
(`native/void-engine`, CMake). The shell runs and its UI is fully drivable
without it; it cannot produce sound without an engine.

**No flatc, Node, pnpm, Rust or C++ toolchain is needed on the end-user
machine** — flatc is a build-time dependency of `void-protocol`'s `build.rs`
only; all generated bindings are baked into the binary at compile time.

## Linux (`.deb` / `.rpm` / `.AppImage`)

```bash
# Debian/Ubuntu
sudo dpkg -i void_*.deb            # or: sudo apt install ./void_*.deb
# Fedora/RHEL
sudo rpm -i void-*.rpm             # or: sudo dnf install ./void-*.rpm
# Any distro — no install
chmod +x VOID_*.AppImage && ./VOID_*.AppImage
```

Runtime deps (already present on a stock desktop install; the `.deb`
declares them): WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`), GTK3, libsoup3,
libayatana-appindicator3.

Verify: `which void` (or the .deb's `/usr/bin/void-tauri`), launch, confirm
the Studio shell opens. Offline: launch with networking disabled — the app
makes no required outbound calls (supply-chain/privacy audits assert no
telemetry sinks; updater checks are opt-in via the configured endpoint only
when invoked).

## macOS (`.app` / `.dmg`)

```bash
open VOID_*.dmg        # drag VOID.app into /Applications
open /Applications/VOID.app
```

Unsigned `.app` → Gatekeeper blocks the first open: right-click → Open, or
`xattr -dr com.apple.quarantine /Applications/VOID.app` on a test machine.
Requires macOS 12+ (Tauri baseline). Verify: Studio window opens; the
engine-spawn control is reachable (dev dashboard).

## Windows (`.msi` / `.nsis` installer)

```text
VOID_*.msi             — per-machine MSI
VOID_*.exe             — NSIS installer (per-user by default)
```

SmartScreen will warn (unsigned). WebView2 runtime is required — present on
Windows 10 1803+/11 and auto-provisioned by the Tauri bootstrapper if absent
(the one justified network fetch; everything else is offline). Verify:
launch VOID from Start Menu.

## Verify summary per install

| Check | Pass shape |
|---|---|
| Launch offline | Studio shell opens, no login/network requirement |
| Engine spawn | dev dashboard `spawn_engine` accepts a `void-engine` binary path (not bundled — OPS-N07) |
| Update check | disabled until signed manifests exist (placeholder pubkey fails closed — `UPDATER.md`) |
| Diagnostic/telemetry | none emitted (privacy-audit static sweep; runtime drill = OPS-N08) |
