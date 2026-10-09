# VOID native control protocol — wire semantics

Schema: `void_control.fbs` (protocol major 1, minor 0). Normative rules live in
`docs/void-handoff/CONTRACTS.md`; this file records the concrete wire layout.
Generated bindings are produced by `tools/protocol-gen.sh` with pinned
`flatc 25.9.23` and are **not** committed — they land in `protocol/generated/`
(gitignored) so Cargo/CMake builds regenerate deterministically.

## Channels

Two channels connect the Rust coordinator (`crates/void-worker` supervisor) to
each native worker (`native/void-engine`, `native/void-plugin-scanner`):

| Channel | Socket | Direction | Reliability |
|---|---|---|---|
| control | Unix-domain socket (Linux/macOS) / named pipe (Windows), per-worker | bidirectional | reliable, ordered |
| telemetry | second UDS/pipe per worker | worker → app only | lossy: frames may be coalesced/dropped to ≤30/s |

Both sockets live under a per-launch directory owned by the current user with
`0700` permissions (e.g. `$XDG_RUNTIME_DIR/void/<launch_id>/`). The path and a
128-bit `launch_token` are handed to the child via environment (`VOID_CONTROL_SOCK`,
`VOID_TELEMETRY_SOCK`, `VOID_WORKER_TOKEN`) — never argv.

**Handshake:** the worker must connect and send `WorkerHello` (with the launch
token, worker kind, protocol major/minor, `engine_epoch` for ENGINE workers)
within the startup deadline before any other frame. The coordinator rejects
wrong/missing tokens and unsupported `protocol_major` and closes the connection.
After hello, the coordinator sends `ControlFrame{ControlRequest{...}}` frames and
receives `ControlFrame{ControlEvent{...}}` responses.

## Framing

Every message on both channels is:

```
u32 LE frame_length | FlatBuffer payload (frame_length bytes)
```

- `frame_length` ≤ **1 MiB** (`CONTROL_FRAME_MAX`); larger frames are rejected
  and the connection closed.
- Buffers are verified (`flatbuffers::Verifier` / Rust `root` with verifier)
  before any field access. FlatBuffers is not authentication — the token is.
- `ControlRequest` (persistent commands) queue depth per project: **128**
  pending — excess gets `BUSY`, never silent loss.
- `ReadRequest` responses page at ≤2000 items or ≤512 KiB.
- `PANIC` transport requests are served ahead of queued edits: the supervisor
  keeps headroom in the control queue for them.

## Ordering and acks

`PersistentCommand` frames on a project are applied strictly in socket order on
the engine's message thread — never on the audio callback. Every command gets
exactly one `CommandReceipt` with `APPLIED | DUPLICATE | REJECTED |
OUTCOME_UNKNOWN` semantics per CONTRACTS.md §3. `DUPLICATE` requires the same
`command_id` **and** identical canonical payload hash; same id + different
payload → `COMMAND_ID_REUSE`.

`TransportRequest` results arrive as `TransportAck` on the telemetry channel;
`ReadRequest` → `ReadResponse` on control. `SaveProjectOp` produces a command
receipt when the save *request* is accepted and a `SaveResultEvent` when the
checkpoint becomes durable.

## Version negotiation

`WorkerHello.protocol_major` must equal the coordinator's compiled major (1).
Minor mismatches are tolerated upward-compatible only: a worker with a *lower*
minor may lack operations the coordinator sends; those commands fail
`UNSUPPORTED_CAPABILITY`. Unknown union variants in either direction →
`UNSUPPORTED_VERSION` rejection without state mutation.

## Object identity

`track_id`/`clip_id`/`note_id`/… are coordinator-issued UUIDs, opaque to the
engine (which stores them as its own metadata map — see
`docs/engine/ENGINE_API_MAP.md` once the engine lane writes it). They never
encode relationships; scene slots carry `sceneId` + `trackId` fields.
