# WASM module threat notes (W28)

Every mitigation listed has a real test in `tests/wasm/` — no claimed
defense is untested.

| threat | mitigation | test |
|---|---|---|
| infinite-loop module hangs the host | fuel metering kills deterministic compute; ~1 ms epoch deadline kills wall-time runaway; cross-thread `CancelToken` checked every tick | `infinite_loop_module_dies_on_fuel`, `..._on_epoch_deadline`, `cancel_flag_kills_guest_calls` |
| memory-bomb module | declared wasm memory-minimum vs manifest ceiling → rejected BEFORE instantiate; runtime `memory.grow` hits the typed `ResourceLimiter` | `oversized_module_rejected_before_instantiate`, `runtime_memory_growth_hits_the_limiter` |
| ambient fs/net/env/wasi import | manifest `ambient_requests` deny + static import-section scan → `ImportDenied` + linker contains only `void_host.log` | `ambient_request_in_manifest_is_never_grantable`, `denied_import_fails_to_instantiate_{net,wasi}` |
| native binary shipped as a "module" | `\0asm` magic check → `InvalidModulePackage`; modules are declarative spec (manifest+wasm) — no native path exists | `native_binary_is_not_a_module` |
| swapped/modified package on disk | sha256 bind at spec build + per-load re-hash of manifest AND wasm against `index.json` → `Tampered` | `manifest_binds_wasm_bytes_by_sha256`, `registry_tamper_detection_on_disk`, `hot_reload_onto_tampered_version_keeps_old_live` |
| hostile upgrade kills running module | hot reload instantiates next BEFORE evicting old; state carried only if both declare `state`; any failure keeps last-known-good live | `hot_reload_failure_keeps_last_known_good`, `hot_reload_migrates_state_and_swaps_code` |
| revoked module keeps running | `ModuleRuntime::revoke` marks + unloads live instances in one step; revoked entries can never load again | `revoke_unloads_live_and_blocks_reload` |
| log flooding from guest | `void_host.log` clamps each line ≤256 chars, total bytes ≤64 KiB per instance | `the_one_granted_import_works_and_is_bounded` |
| oversized claimed state blob | `void_state_len` bound checked at init → `StateTooLarge` | `oversized_declared_state_rejected_at_init` |
| malformed transform output | every emitted `NoteEvent` re-validated host-side (pitch/velocity/channel) → `AbiViolation` | `malformed_transform_output_is_rejected` |
| non-finite params leak downstream | `param_set` refuses NaN/±Inf host-side | `non_finite_param_is_rejected` |
| over-cap output claims | `midi_xform` return value must be ≤ out_cap AND a whole number of events → `OutputOverflow`/`AbiViolation` | (covered by `OutputOverflow` variant + cap probe in `host.rs`) |
| capability-vs-export mismatch | declared caps require their export set at instantiate | `declared_capability_requires_the_export` |
| shared-memory/thread escape | `shared` memory flag → `AmbientDenied("threads")` at scan | (scan rule; see `spec::scan_wasm`) |
| unqualified real-time execution | `placement` enum cannot represent the rt audio path — the value does not exist to write | `placement_is_a_schema_level_guard` |

## Residual risks recorded honestly

- **Timing side channels**: a module can measure nothing — no clock
  import exists — but its *host-observed* call latency leaks its own
  workload, which is expected and bounded by deadline.
- **Fuel granularity**: fuel is instruction-count based, not cycle-
  accurate; a hostile-but-halted module can still burn budget to
  `FuelExhausted` quickly. Deadline + cancel bound wall time anyway.
- **Registry on a hostile filesystem**: hashes verify bytes at load
  time, but a root-capable attacker could swap files between verify
  and instantiate; the registry is a dev/project store, not a signing
  root (module signing is out of scope for W28 — see NEEDS.md).
