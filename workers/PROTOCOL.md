# void-job-worker protocol v1

The argv-worker contract VOID AI/analysis jobs run under (W12,
CONTRACTS.md §6). The coordinator spawns the worker; it never installs
or downloads code at job time.

## Spawn

```
<worker-executable> --staging <staging-dir>
```

- argv-only invocation: `Command` + argument vector, `env_clear`,
  never a shell. The only process arguments are the executable and
  `--staging <dir>`; everything else arrives over stdin.
- The staging directory is the worker's entire write scope. Absolute
  paths, `..` segments and sibling writes are contract violations —
  the runner treats them as worker failure, not bad luck.
- Environment is empty except explicit coordinator grants. Secrets
  never appear in argv or env (§2).

## stdin: job spec

One JSON document, UTF-8, then EOF — the `job/1.0.0` shape
(`docs/void-handoff/schemas/job.schema.json`, serialized
`void_jobs::JobSpec`). `parameters` is the task-scoped object the
runtime manifest was qualified for.

## stdout: JSON lines

Each line is a complete JSON object with `"v": 1` and a `kind`:

```json
{"v":1,"kind":"progress","percent":42,"message":"optional stage text"}
{"v":1,"kind":"result","status":"succeeded","artifacts":[{"path":"out.wav"}],"warnings":[],"error":null}
{"v":1,"kind":"result","status":"failed","artifacts":[],"warnings":[],"error":"safe message"}
```

- `progress`: optional, repeatable, monotone-ish percent (0–100) or
  omitted entirely (indeterminate). Never authoritative.
- `result`: exactly one, and it must be the last JSON line. `status`
  is authoritative — the exit code only distinguishes protocol
  completion from worker crash.
- `artifacts[].path` is relative to the staging dir. The runner
  re-verifies every declared artifact exists, is inside staging and
  recomputes its SHA-256 — worker-reported success is never evidence.
- `warnings`/`error` are user-safe strings: no absolute paths, no
  secrets, bounded length.

Non-JSON stdout lines are tolerated as log noise and ignored.

## stderr

Human diagnostics only. Bounded tail captured for failure reports.

## Exit codes

- `0` — protocol completed (check `result.status` for the job outcome).
- nonzero — worker crashed/aborted; the job fails regardless of any
  result line seen.

## Cancellation / budgets

The runner may SIGKILL the process at any point (cancel, deadline,
cpu/memory rlimit trip). A clean worker exits promptly on stdin EOF;
it is not required to handle signals — dead is dead, and late output
is quarantined rather than applied (§6).

## Reference implementation

`workers/fake/` is the v1 reference: a real but trivial deterministic
synth (sine WAV / tiny MIDI) used by the runner boundary tests. It
genuinely renders bytes per run — it is honest about being a fixture,
never a canned result.
