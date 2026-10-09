# WASM extension capability model (W28)

A VOID extension module is **declarative**: a package directory
containing `manifest.json` + `module.wasm`. There is no native-code
module path — `spec::scan_wasm` rejects anything that is not
`\0asm`-magic WebAssembly before the manifest's sha256 bind is even
checked.

## Manifest schema v1 (`void-wasm/1`)

| field | rule |
|---|---|
| `manifest_version` | `1` |
| `module_id` | `[A-Za-z0-9._-]{1,128}`, no path separators |
| `name` | display name ≤ 128 chars |
| `version` | semver-lite `MAJOR.MINOR.PATCH(-label)` |
| `wasm_sha256` | 64 lowercase-hex; binds `module.wasm` bytes |
| `capabilities` | ≥1 of the grantable set below |
| `ambient_requests` | recorded for audit; **any** entry → `AmbientDenied` |
| `limits` | declared budgets, all string-int64, ≤ `HostPolicy` ceilings |
| `placement` | `param_surface` / `offline_render` / `midi_transform` |
| `abi` | `void-wasm/1` |

Limits serialize as **decimal strings** (string-int64 boundary rule,
CONTRACTS §1) in manifests, catalog rows, and studio DTOs.

## Grantable capabilities → required exports

| capability | required ABI exports | meaning |
|---|---|---|
| `params` | `void_param_count/get/set` | indexed f64 parameter surface |
| `state` | `void_state_len/save/restore` | opaque blob persisted across reload |
| `render-off` | `void_render_off` | OFFLINE render only — see placement |
| `midi-transform` | `void_midi_xform` | bounded note-event transform |
| `no-fx` | — | honesty label: not an FX processor |
| `generator` | — | honesty label: source-of-notes/audio |

Every declared capability's required exports are checked at
instantiate (`MissingCapabilityExport`). An undeclared call (e.g.
`midi_xform` on a params module) is refused (`CapabilityNotDeclared`).

## Ambient powers — never grantable

`fs`, `net`, `env`, `spawn`, `clock`, `threads`, `random`, `wasi` —
rejected at manifest parse. Enforcement is layered, not advisory:

1. **Manifest**: `ambient_requests` is a non-empty-means-deny field.
2. **Static scan**: `spec::scan_wasm` walks the wasm import section;
   any import outside the granted surface → `ImportDenied` before
   instantiate. WASI families, `env.*`, anything a native loader could
   satisfy is denied by name.
3. **Linker**: the store's `Linker` contains exactly one function —
   `void_host.log` (bounded, in-memory). There is no WASI context to
   even resolve a sneaked import against.
4. **Shared memory** (threads) → `AmbientDenied` at scan.

## Runtime limits (three independent kills + resource ceiling)

| mechanism | kills | mechanism detail |
|---|---|---|
| fuel | deterministic runaway compute | `consume_fuel`; budget re-armed per call, `FuelExhausted` |
| epoch deadline | time runaway | host thread increments the engine epoch every ~1 ms; `UpdateDeadline` traps at the declared ms, `DeadlineExceeded` |
| cancel token | cross-thread stop | `AtomicBool` checked at every epoch tick → `Cancelled` |
| `ResourceLimiter` | memory | declared-min checked at scan; growth trapped at declared ceiling → `MemoryLimitExceeded` |

## Placement — schema-level honesty

`placement` declares WHERE the runtime may schedule the module.
`param_surface` / `offline_render` / `midi_transform` are the only
representable values — **there is no real-time-audio value a manifest
can write**. A module can never claim the critical audio path; the
render-off contract is a buffer callback the offline scheduler may
invoke, and `render-off` capability requires `offline_render`
placement (`InvalidManifest` otherwise).

## Host ABI `void-wasm/1`

Narrow descriptor — the only host import is `void_host.log(level,
ptr, len)` (line-clamped, byte-capped). Module exports:

```
void_abi_version() -> i32     must be 1
void_alloc(len i32) -> i32    guest bump allocator for in/out buffers
void_init() -> i32            0 = accept
memory                        exported linear memory (single, non-shared)

params:   void_param_count() -> i32
          void_param_get(i32) -> f64
          void_param_set(i32, f64) -> i32    0 = accepted (NaN/Inf refused host-side)
state:    void_state_len() -> i32            ≤ max_state_bytes
          void_state_save(ptr) -> i32        bytes written
          void_state_restore(ptr, len) -> i32
render:   void_render_off(out_ptr, frames) -> i32   f32 frames written
midi:     void_midi_xform(in_ptr, in_len, out_ptr, out_cap) -> i32  bytes written
```

Out buffers are host-probed: over-cap claims → `OutputOverflow`;
malformed `NoteEvent`s (pitch >127 etc.) → `AbiViolation`; oversized
state claims → `StateTooLarge` at init.

## Registry

On-disk `modules/<id>/<version>/{manifest.json,module.wasm}` +
`index.json` (v1). Both files' sha256 are recorded at install and
**re-hashed at every load** — tampered bytes → `Tampered`, never
silently loaded. Versions are semver-ordered; `revoke` marks an entry
(can never instantiate again) and the runtime unloads it live.
Duplicate versions are rejected; revoked files are kept for audit.

## Hot reload — last-known-good

`hot_reload` builds the new instance BEFORE touching the old one,
carries the 24-byte state blob only when both versions declare
`state`, then swaps. **Any** failure (tamper, instantiate, restore)
leaves the previous instance live with its state intact — the
old-known-good module remains recoverable (T97).
