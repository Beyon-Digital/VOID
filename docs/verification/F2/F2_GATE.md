# F2 Gate — W16 exit note

Verdict: **F2 NOT declared complete.** T63 and T65 pass against real
code paths with artifacts to show for them; T64 is honestly blocked on
a human session; and material phase gaps remain that the matrix calls
out below. Per-lane detail lives in `CAPABILITY_MATRIX.md` and
`USABILITY_PROTOCOL.md`.

## Per-test result

| Test | Verdict | Evidence |
|---|---|---|
| **T63** AI concurrency regression | **PASS** | `tests/journeys/ai` — 6 storm tests + 2 invalidation tests, all real `JobRunner`/worker/`void-proposals` paths |
| **T64** Musician usability | **BLOCKED** | needs a human musician; runnable protocol scripted at `USABILITY_PROTOCOL.md` — no result file yet |
| **T65** Capability evidence | **PASS** | `offline_gate.sh` exit 0 — all sockets denied incl. loopback; manual ops + save path + all 5 capabilities verified inside the gate |

## T63 coverage (what was actually asserted)

- **Admission bounds**: heavy (ram>4 GiB) ⇒ max 1 running, next queues;
  small ⇒ max 2 running, third queues; 4-deep waiting room ⇒
  `JobError::Busy` for both classes. Real running jobs, not mocks.
- **Budgets**: 0.6 s wall deadline kills a 30 s job (`Failed`, no
  provenance); 512 MiB `RLIMIT_AS` kills `oom` mode; 1 s `RLIMIT_CPU`
  kills `spin` mode.
- **SIGKILL mid-run**: worker child located via its unique
  `--staging job-<id>` argv, `kill -9`; runner reports
  `Failed{error:"worker exited … without a result"}`; job row `failed`;
  zero provenance. Retry on a fresh job produces real
  `provenance.json` + `result.json`.
- **Crash between stdout lines**: `void-fixture-midline-crash` emits a
  progress line then a truncated result line and SIGKILLs itself —
  `Failed`, never `Succeeded`. Fake worker's `crash` mode same verdict.
- **Two-project storm**: 9 jobs across 2 open project containers on one
  app db — mixed `succeeded/failed/cancelled`, mid-storm cooperative
  cancel + SIGKILL; provenance lands only in the owning container;
  `list_for_project` disjoint; strays counted in provenance, excluded
  from the committed artifact list.
- **Late results**: `record_result` on a cancelled job stays
  `Cancelled` and sets `quarantined` — no fake 'done' reachable.
- **Invalidation**: proposals `pending→ready→stale|rejected|accepted`
  under region edits (context-sha mismatch ⇒ `Revalidation`), clip gone,
  project switch (`different project open`), `SessionEnded` sweep on
  ready+pending, `Cancelled`→`stale(cancelled)`, `Failed`→`failed`,
  `revalidate` mints a NEW record via `supersedes` (stale never revives),
  `LockedCollision` + `drop_locked` + subset selection.
- **Prompt injection surface**: context `labels`/`notes` are inert
  data — worker treats them as parameters only; malformed context is
  rejected by `RegionContext::validate` before admission.

## T65 coverage

See `CAPABILITY_MATRIX.md` — every claimed AI/gesture capability has
its own evidence row produced inside the socket-denied namespace.

## Phase gaps before F2-complete can be declared

| Gap | Evidence | Needed by |
|---|---|---|
| **T64 unrun** | no `T64_RESULT.md` | T64 row itself |
| **Preview/audition layer absent** | a proposal can only be heard by accepting it to the clip — blind accept | NEEDS §12 |
| **Proposal wire ops absent** | `packages/void-studio/src/jobs/job.ts` parses job DTOs but there are no `SubmitJob`/`CancelJob` ops, no `JOB_LIST` view, no `JobEvent` telemetry wiring — proposals are exercised via crate API, not through the wire | §13–14 |
| **Engine input plumbing** | accepted `AcceptPlan.inserts` mint `insert_note` ops but nothing on the engine boundary consumes them end-to-end in the app | engine lane |
| **Gesture commit path is unit-level only** | gestures verified via vitest op-builders (79/79 inside the gate); no wire-level gesture→engine commit test exists yet | §13–14 |
| **musicgen license** | CC-BY-NC-4.0 weights — non-commercial only; gate flags it but product must surface it at audition time | NEEDS §12 |
| **GPU-class workers untested** | all evidence is CPU-only; no vram reservation path exercised (`vram_bytes` always 0) | scale-down note |

None of this blocks the T63/T65 verdicts above; it blocks the *F2
complete* declaration. The gate will fail loudly if any of these gaps
get silently closed by an incorrect claim (dependency boundary test,
protocol-crash tests, quarantine assertions).

## Commands (reproduce)

```
# full T63+invalidation+latency+T65 rust suite — exit 0, 17 tests
cargo test --manifest-path tests/journeys/ai/Cargo.toml -- --nocapture

# T65 offline gate — exit 0 ("OFFLINE GATE PASS")
tests/journeys/ai/offline_gate.sh

# baselines re-verified on this branch — exit 0
cargo test --workspace --exclude void-tauri     # 23 suites ok
pnpm -r test                                   # 250+ vitest ok
```
