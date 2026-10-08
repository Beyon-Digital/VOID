# NEEDS — engine-resident gaps for the W19 content/audio-edit lane

Owner: this lane owns the *model/policy/inventory* layer only
(`crates/void-content`, `packages/void-studio/src/audio-edit`,
`content/`, `docs/content-rights/`). Everything below needs the native
engine, real DSP, or licensed content this Linux box cannot supply —
recorded honestly, no invented passes. Append-only numbering; new lanes
continue the sequence.

1. **Stretch/warp DSP (EDIT-01).** `audio-edit/varispeed.ts` covers the
   rational rate→sample mapping and `linked | time-only | pitch-only`
   mode contract, but `time-only` (constant-pitch stretch) and
   `pitch-only` resample are engine renderers. T72 needs silence/
   impulse/sweep conformance runs against real DSP before any entry
   moves off `needs_dsp`.

2. **Transient detection (EDIT-05).** `transients.ts` is the marker
   model (insert/delete/shift/split/trim/move survival). Onset
   *detection* itself — the analysis that produces `OnsetPoint`s — is
   engine-side; drum replacement and audio-to-MIDI beyond "derived MIDI
   stays hand-editable" is unbuilt there too.

3. **Pitch analysis + correction renderers (EDIT-02, FX-06).**
   `pitch.ts` keeps the nondestructive model (detected cents immutable,
   correction/shift/formant layered, exact revert tested). Detection,
   the correction-curve renderer, drift/vibrato controls and the
   realtime Pitch Correction / Vocal Transformer processors are
   engine DSP — T74 musical-quality fixtures pending.

4. **Repair tools (EDIT-04).** Sample-editor ops exist at the model
   level only via marker/edit banks. Zero-crossing snaps, normalize,
   DC removal, phase invert and silence renderers are engine-side;
   "protected source copies" is satisfied at the asset layer
   (void-assets sha-addressed immutability + this lane's quarantine).

5. **Sampler playback + streaming IO (SND-03).** `zones.rs`/`streaming.rs`
   implement the zone/layer/round-robin selection model and bounded
   voice/cache budgets with determinism proofs — the voice allocator
   *models* the policy. Sample streaming, interpolation and real voice
   lifecycle live in the engine; its allocator must honour the same
   `VoiceSteal` policy enum for parity.

6. **Stock-processor DSP (T72 sweep).** Every `content/inventory.json`
   `kind: effect|instrument` entry needs a real implementation +
   listening/numerical fixtures before `status` may leave `needs_dsp`.
   No entry is `ready` — that is the honest current state.

7. **Licensed content (SND-06).** `ledger.json` rows LGR-0001..0004 are
   `required`, not executed: orchestral sample sets, convolution IRs,
   tape-keyboard samples and legacy packs need agreements + real
   artifacts before those `needs_license` entries can ship. Packs can
   only install through `ContentStore::install` once manifest + files
   verify.

8. **Wire surface.** Marker/pitch/tail edits are model layer: there is
   no `void_control.fbs` op for transient markers, per-note pitch, or
   render-tail params yet (`PitchEditSpec`/`RenderTailParams` document
   the proposed payloads, including the `tailFade*` fields void-export's
   `TailPolicy` does not carry today — schema addition is
   integrator-owned).

9. **Six-stem separation (EDIT-06).** Belongs to the W15 analysis lane
   (htdemucs_6s worker per FEATURE_MAP); W19 contributes nothing engine-
   side here. Recorded so T74 readers don't look for it in this lane.

10. **Signed manifests.** Pack integrity today is sha256 checksum via
    `ContentPackManifest::parse_and_verify`. Cryptographic *signing*
    (D06 key-management decision) is out of scope — `manifest_sha256`
    detects tampering but does not prove authorship.
