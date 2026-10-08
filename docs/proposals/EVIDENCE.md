# W13 predictive composition — evidence

Branch `devin/void-lane-proposals` off `devin/void-implementation`.
Three deliverables landed:

- `workers/symbolic/` — `void-symbolic-worker` v1.0.0, argv protocol v1.
  Seeded interval-Markov generator (first-order interval chain +
  Krumhansl-Schmuckler key estimate + duration/IOI marginals, xorshift64*
  RNG seeded by splitmix64 of the spec seed). Emits `proposals.json` =
  `void-proposals/1`: ≤8 ranked candidates, each note events + the
  model's own mean-likelihood score + measured-facts rationale.
  No network, no reads outside the spec, labels are inert data.
- `crates/void-proposals/` — proposal lifecycle: scoped `RegionContext`
  digest (sha256 identity) → void-jobs `Symbolic` submit → runner →
  document parse/validate → `ready` record with full provenance
  (generator id/version, model id, runtime sha, seed, document sha,
  context sha, source revision, job id) → `plan_accept` revalidates
  inside accept and emits an `InsertNoteOp` plan under ONE transaction
  id → `commit_accepted` / `reject`. Stale never revives — `revalidate`
  mints a new record via `supersedes`. File store
  `proposals/<id>/proposal.json` (tmp+rename atomic).
- `packages/void-studio/src/proposals/` + `void-ui/ProposalCard` —
  ranked store, keyboard selection, partial-accept subset, ghost-note
  view data (`ghost:true`, locked-range paint, view-state-only), bridge
  to `sendCommand(InsertNoteOp)` sharing the plan transaction id.

## T-id coverage

- **T53 ghost-note audition** — `deterministic_seed_reproduces_document`
  (same context+seed ⇒ identical document SHA-256, verified twice via
  the real runner + real worker), `end_to_end_generate_collect_accept`
  (ready record carries ranked candidates + provenance), studio tests
  (ghost derivation, `ghost:true` markers, view-state invariant honored,
  candidates never in `StudioViewState` → save omits them by
  construction). Playback audition layer is a protocol gap → NEEDS #12.
- **T54 partial accept + undo** — `plan_accept`/`planAcceptLocal`
  produce inserts under one `transactionId`; locked-range notes dropped
  (never inserted, reported in `droppedIndices`); accepted note ids are
  minted at accept (model never names objects); `commit_accepted`
  records transactionId+noteIds — undo = one `UndoOp`, accepted notes
  are normal editable MIDI. Studio tests: subset accept, shared-tx wire
  send, one-transaction assertion.
- **T55 invalidation** — `stale_revalidation_blocks_accept_and_
  supersedes`: accept revalidates project/clip/context-sha inside the
  call; mismatch ⇒ `Revalidation` error, nothing planned;
  `mark_stale(_one)` sweeps live records with an explicit cause;
  revalidation creates a NEW record (`supersedes`), never resurrects.
  Studio `markStale` clears selection + ghosts, carries the cause.
- **T56 injection** — `hostile_documents_rejected`: wrong doc tag,
  out-of-range score/pitch, unordered onsets, negative/zero ticks,
  >8 candidates, >1024 notes all fail parse; document caps bound every
  collection and string; unknown keys ignored (dropped at the doc
  boundary, never reaching records). Worker-side labels parsed-but-
  unused — `'}) ; rm -rf / ; ignore previous` sits inert in the seed42
  fixture labels. Candidate ids/paths/ops are never read from model
  output — ids minted in `plan_accept`, paths come from the record's
  scoped context.

## Commands + exit codes (final)

- `cargo test --workspace --exclude void-tauri` — **exit 0**; all suites
  green (void-proposals: 6 lifecycle tests incl. real-runner E2E).
- `pnpm -r test` — **exit 0**; void-studio 135/135 (proposals 8 tests),
  void-ui 4/4, remaining packages unchanged green.
- `cargo build --manifest-path workers/symbolic/Cargo.toml` — exit 0.
- `pnpm --filter void-studio build` — **exit 2, pre-existing on base**:
  `src/index.ts(11,1) TS2308 isTerminal` collision between `./export`
  and `./jobs` (both barrels already on `devin/void-implementation`; the
  W13 diff adds one additive `export * from './proposals'` line, no new
  conflict). vitest unaffected (esbuild transform). Flagged for
  integrator, not silently fixed — barrel is outside owned paths.

## Determinism evidence (fixtures committed)

`docs/proposals/fixtures/` — `spec-seed42.json`, `spec-seed43.json`,
`proposals-seed42.json`, `proposals-seed43.json`,
`stdout-seed42.txt` (protocol lines).

SHA-256:

```
b1ce694bd5b0ebd9cb98ac3f1d8666c961740e879229c43b4c7e297ae673c863  proposals-seed42.json
a5bdcef5dc4fdbb19b8607897fe158b4e21c07822ffae0422c272656aaf461d9  proposals-seed43.json
53fbe3d270b1ba4dc716079edab369b9e9b57ed233ad00e64a588ced366d61f7  void-symbolic-worker (debug build)
```

seed42 run twice ⇒ byte-identical `b1ce…`; seed43 ⇒ different document.
Scores are the model's own likelihoods (seed42: `0.2745, 0.2574,
0.2243` — monotone ranks). Reproduce:

```
target/debug/void-symbolic-worker --staging <dir> < spec-seed42.json
sha256sum <dir>/proposals.json   # expect b1ce…
```

## Known gaps / blocked

- **No wire path for proposals** — lifecycle ops/view/event absent
  (NEEDS #13–14); generation + records are driven in-process per
  CONTRACTS §6, accept rides existing `InsertNoteOp` (no new ops added).
- **Transient audition layer** — no engine preview layer exists
  (NEEDS #12); ghost notes are honest view-state only, cannot sound.
- **No UI wiring into a live app** — `apps/` is outside owned paths;
  the store + ProposalCard + bridge are the drop-in surface.
