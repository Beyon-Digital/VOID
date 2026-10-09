# NEEDS — W28 WASM extensions lane

What exists (verified, Linux) vs what still needs the coordinator /
native engine / SDK rights. Append-only, numbered entries — same
pattern as docs/{engine,producer,mix}/NEEDS.md.

- **N1 — protocol wire ops.** Protocol major.1 has NO module/extension
  op members. The lifecycle ops exist as validated intended-shape
  builders in `packages/void-studio/src/extensions/ops.ts`
  (`install_module`, `load_module`, `unload_module`, `reload_module`,
  `revoke_module`, `cancel_module_call`) — they land byte-identical
  once protocol adds the members. Needs: protocol op members +
  coordinator dispatch into `void_wasm::ModuleRuntime`.
- **N2 — catalog read-view.** Studio ingests `void-wasm/catalog@1`
  payload (`extensions/types.ts::parseCatalog`) matching
  `RegistryRow` + `row_status`. Needs: coordinator emits the catalog
  view on registry change (install/revoke/live transitions).
- **N3 — module install pipeline.** `ModuleSpec::from_dir` verifies a
  package dir; a real install flow needs package upload/extraction
  into a staging dir before `ModuleRegistry::install`. Needs:
  coordinator-side package intake (zip/tar) + size caps; the
  verification itself is done.
- **N4 — ARA integration.** Gated by `gates.rs` + ADR 0001; needs the
  Celemony ARA SDK licence + a native host implementation. Out of
  scope until rights exist.
- **N5 — native plugin export (VST3/CLAP/AU).** Gated by `gates.rs` +
  ADR 0002; needs per-format SDK rights AND a signer policy (the
  artifact is a signed native binary — deliberately NOT the wasm
  declarative spec). Out of scope until both exist.
- **N6 — offline render scheduler.** `void_render_off` is implemented
  and proven by buffer round-trip, but nothing schedules it — needs
  the engine's offline-render path to invoke modules at declared
  `offline_render` placement (never the rt audio path; placement is
  schema-enforced, not advisory).
- **N7 — live event buffers for midi-transform.** `void_midi_xform`
  round-trips real `NoteEvent`s host-side; feeding it actual project
  note lanes needs the coordinator's note-buffer plumbing.
- **N8 — studio extensions UI.** `extensions/` provides view-store +
  ops; a panel surface (list/detail/reload/revoke actions) needs the
  studio's panel chrome — view-state wiring done, visual surface open.
- **N9 — module signing.** Registry verifies integrity (sha256), not
  provenance. Content signing of packages is a future decision tied
  to distribution policy.
- **N10 — epoch ticker sharing.** Each `Host` spawns its own 1 ms
  epoch thread; a coordinator process should share one engine-wide
  ticker across hosts when multiple runtimes coexist.
