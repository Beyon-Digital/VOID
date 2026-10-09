# Exchange (W20) — interchange, registry, isolation policy

Lane R. The non-native half of "Plugin compatibility, optional isolation
and exchange" (tests T75–T77 where Linux-verifiable). Everything here is
real and runnable on this box; the native halves are recorded as
NEEDS-style entries in [`docs/engine/NEEDS.md`](../engine/NEEDS.md)
items 32–35.

## Crate: `crates/void-exchange`

| Module | What it does |
| --- | --- |
| `document` | `ExchangeDocument` — the format-neutral song model. Wider than the wire (fades, note channel/release velocity, clip enable/offset, plugin state refs, markers, loop range) so foreign formats round-trip honestly and loss reports have somewhere to point. `validate()` collects every shape error. |
| `loss` | `LossReport` — deterministic per-element loss ledger. Sorted canonically (`element, aspect, kind, reason`); `canonical_json` is byte-identical for identical input (T77). `dropped` = gone; `approximated` = kept with reduced fidelity, reason says what changed. |
| `dawproject` | Real DAWproject subset: ZIP container (`project.xml` + `metadata.xml` + payload files), XML written in schema order, ZIP entries sorted with a fixed timestamp → byte-deterministic archives. Reads the same subset back; everything outside it (warps, scenes, clip-slots, sends, video, nested clip timelines, automation points, seconds time units, clip references, non-PoT denominators on import) lands in the loss report. Unsafe zip paths (absolute/`..`/backslash) rejected. |
| `midi` | Real SMF codec. Export: type-1, conductor track with tempo + time-sig + markers, per-track note tracks. Division chosen adaptively (largest exact divisor of 960000 ≤ 32767) so every tick converts exactly; only when no division fits does it quantize — with per-element approximations. bpm is μs-quantized (SMF integer μs/quarter) and reported. Import: type 0/1 (type 2 rejected), running status, note on/off pairing, tempo/sig/marker metas, CC/bend/program/aftertouch/SysEx recorded as dropped counts. |
| `stems` | `void-stems/1` manifest: per-stem source track, rel path, media type, sha256, byte count (string), length ticks+seconds, rate/channels, render tempo map. `stage_bundle`/`load_bundle` verify every payload hash — tamper is an error, not a warning. |
| `registry` | `void-plugin-registry/1`: `PluginDescriptor` (format/uid/name/vendor/version/role/arch/state-format-version) keyed `format/uid`, `hosted_on` platforms, quarantine. `compatibility()` returns a typed `CompatibilityStatus` — `aax` → `FormatNotHostable` always, `au` → `FormatNotHostable` off macOS, declared-arch mismatch → `ArchMismatch`, quarantined → `Quarantined`, never-seen → `Unknown` (never assumed compatible), observed on platform → `Compatible`. |
| `preserve` | `void-missing-plugins/1`: `PreservedPluginState` = instance/track/slot + descriptor + hex state blob + sha256. Survives save/reopen via JSON round-trip; `resolve()`/`resolve_for_arch()` returns `Rehydrate` (descriptor + blob) or typed `StillMissing` (NotInstalled / StateVersionMismatch / ArchMismatch). Tested with fake descriptors — no plugin code runs. |
| `isolation` | `IsolationPolicy` = mode + bounded bridge budgets + deadline + restart budget + latency + death action. `validate()` rejects unbounded configs — in-process policies must carry zeroed budgets (nonzero claims enforcement nothing provides). `requested_policy()` yields typed `IsolationUnavailable` (`FormatNotHostable`, `PlatformNotSupported`, `NoIsolatedHost`, `DeviceBound`). This is the policy model a native plugin worker enforces — not isolation itself. |
| `plan` | `plan_import(doc)` → ordered wire ops (`PlannedOp` with string-int64 payloads) + loss entries for every wire gap (fades #18, note channel/release velocity, clip enable/offset, markers, loop range, plugin state #32). The coordinator applies the plan inside its usual transaction. |

## Studio: `packages/void-studio/src/exchange`

- `types.ts` — LossReport/registry/preserved DTOs (string-int64) +
  `parseLossReport` (malformed → null, never an empty report).
- `importDialog.ts` — phase machine `idle → parsing → review → applying
  → done/failed`. Non-empty reports require explicit `acknowledge()`
  before `beginApply()`; an unreadable report is `failed`, not lossless.
- `exportDialog.ts` — format/include-payloads selection + run report;
  unreadable report → `reportUnreadable` failure flag.
- `badges.ts` — per-slot `PluginBadge` (`missing` / `quarantined` /
  `unavailable` / `unverified`) from PLUGIN_LIST items + registry +
  preserved records. AAX and AU-off-platform badge `unavailable` without
  any record; unrecorded is `unverified`, never assumed fine.

## Supported / not supported — stated honestly

Supported on this lane: `.dawproject` subset above, `.mid` (SMF 0/1),
`void-stems/1` bundles, registry JSON, missing-plugin store JSON, policy
descriptors.

Not supported, never claimed: `.logicx` round-trip, AAX (no host element
exists — descriptor records badge it unavailable), runtime plugin hosting
(AU/VST3/CLAP binaries — NEEDS 34), actual crash isolation (NEEDS 33),
wire-level restore of plugin state (NEEDS 32), clip enable/offset/
marker/loop ops (NEEDS 35), tempo ramps (NEEDS 27), clip fades on the
wire (NEEDS 18).

## Tests

`tests/exchange/` (detached workspace, `cargo test --manifest-path
tests/exchange/Cargo.toml`): 12 tests — dawproject round-trip
semantic-equal + zero loss, loss determinism byte-identical + re-sort on
parse, MIDI round-trip preserving the tempo curve + notes with
approximation entries for μs quantization, stems round-trip + tamper
rejection, registry verdict matrix, missing-plugin reopen + rehydrate +
state-version mismatch, isolation validate/reject incl. in-process
honesty, typed `IsolationUnavailable`, wire-plan ops + loss aspects +
string-int64 payloads, unsafe/malformed input rejection, full document
validation.
