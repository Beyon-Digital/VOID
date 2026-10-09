# Producer lane NEEDS — engine/GUI/audio gaps recorded by Lane S (W21)

Format mirrors docs/engine/NEEDS.md (append-only, grouped by lane).
Each entry: what exists (model/spec verified on Linux) vs what needs
the native engine, GUI shell, or audio hardware this box lacks.

## Lane S — producer gate gaps

1. **Accompaniment wire ops.** `void_control.fbs` has no
   `RequestAccompaniment`/inpaint/continue/vary op. Exists now:
   `crates/void-producer` generates real note candidates +
   `request_accompaniment` mints `ProposalRecord`s through
   `ProposalStore`; studio `producer/ops.ts` emits the intended op DTO.
   Needs: a wire op member + coordinator handler calling
   `request_accompaniment` on the engine's live RegionContext.

2. **Accept apply path.** `plan_accept` returns `PlannedInsert`s under
   one transaction id; nothing on this box can apply them (insert_note
   ops are the W13 lane's engine bridge). Needs: the studio bridge
   route that executes the plan inside the accepted gesture's
   transaction, then `commit_accepted`.

3. **Mastering op execution.** `MasteringOp::{Gain, TruePeakLimiter,
   EqBand}` are validated parameter records. Needs: engine-side DSP to
   render the B-side buffer for audition/accept (the meter and
   suggestion derivation are real and verified; the renderer is not).

4. **A-B audition playback.** `AuditionSpec` carries level-match dB +
   loop range; actual alternating playback needs the audio device +
   engine transport. Needs: transport audition command consuming the
   spec (level-match applied to B).

5. **Loudness on rendered program.** `measure()` accepts decoded PCM;
   on-device it should run over the engine's rendered mix (via
   void-export render or a live meter tap). Needs: a job bridge that
   feeds engine render output (or tap) into `analyze()`.

6. **Stem render execution.** `StemBatchPlan` emits ExportSpecs; per
   -stem soloing/routing is a parameters-level concern the coordinator
   must honor when submitting each member job (the plan pins source ids
   on each member). Needs: `job_spec_for(spec)` submission loop that
   also carries `{stem_source_id}` in `parameters` for the renderer.

7. **Screenset shell.** `screensets` models panel visibility/geometry
   as normalized fractions; the Tauri shell needs a host that applies
   `fitToViewport` output to real dock widgets. Needs: shell layout
   adapter + persisted per-project active id.

8. **Keymap dispatch.** `shortcuts` resolves binding → command id;
   the app's command registry + focus-scoped dispatch (text fields
   exempt) is a shell concern. Needs: keydown router consulting
   `commandFor` in the studio shell.

9. **Import op execution.** `ImportPlan.ops` are ordered materialization
   intents (create_track/bus/clip/send). Needs: the coordinator to run
   them in ONE transaction + a container ingest for `source_path`
   assets (consolidation can collect them via `execute_consolidation`).

10. **Continuation window semantics.** `GenMode::Continue` uses
    `context.continuation_*` when set; the engine decides the real
    continuation anchor (clip end vs loop end). Needs: coordinator to
    populate `continuation_start_ticks`/`continuation_ticks` on the
    RegionContext it hands `request_accompaniment`.
