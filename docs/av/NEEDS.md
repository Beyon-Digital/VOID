# NEEDS — gaps found by the AV export lane (W24 / Lane U)

Owner: `protocol/` is integrator-owned; engine/GPU lanes are native-side.
This file records what the audiovisual pipeline needed vs what exists on
this Linux box / in `void_control.fbs` as of `devin/void-implementation`.

1. **No AV view kinds.** `AV_EXPORT_LIST` is requested by
   `void-studio/src/av/results.ts` but does not exist in
   `void_control.fbs` `ViewKindName` — same gap class as `EXPORT_LIST`
   (audio lane). Until the coordinator serves it, expect
   UNSUPPORTED_CAPABILITY; the request is built honestly regardless.

2. **No job submit/cancel ops.** Same gap the W12 lane recorded
   (`SubmitJobOp`/`CancelJobOp` absent from `PersistentOp`). The void-av
   runner is exercised in-process via `AvRunner` (JobDb), as designed.

3. **No job telemetry kind.** `JobEvent` is parsed defensively by the
   studio store (reused shape from the audio lane); the coordinator
   does not emit it yet. No progress is fabricated.

4. **Real-GPU/visual-program rendering is engine-side.** `void-av`
   encodes pinned inputs (checkpoint files, asset blobs) or lavfi
   fixture sources; it does not composite the visual program. Feeding
   it rendered frames needs the native visual pipeline
   (void-visual/engine) to emit a pinned input — model/policy tested,
   GPU-driven pixels out of scope on this box.

5. **Long-program probe bound.** The matroska/nut audio-duration
   fallback decodes audio and sums `nb_samples`, capped by
   `MAX_PROBE_OUTPUT` — a ~multi-hour program could exceed the bound
   and honestly `ProbeFailed` rather than verify. Per-stream
   `duration_ts` containers (mp4/mov/any with durations) skip the
   fallback entirely.

6. **Container duration is not audio duration.** `format.duration` on
   matroska reports max-of-streams including mux padding (measured
   1.201 s for a 1.0 s/30-frame program) — deliberately NOT used for
   audio verification; only stream-level `duration_ts`/`duration`, else
   the bounded decode count.

7. **`development_only` codecs are encodable but rights-flagged.**
   `h264_aac_mp4` and `prores_pcm_mov` run when encoders exist; the
   rights flag + license note travel in provenance for downstream
   distribution gating. Redistribution clearance is a legal/business
   decision, not a lane decision.
