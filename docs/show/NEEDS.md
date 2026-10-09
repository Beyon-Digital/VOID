# NEEDS — W26/W27 hardware, validator and licensed-path gaps

Lane W (W26 spatial routing/delivery + W27 sync/show-control) on Linux x86_64.
The honest policy: models, state machines and policy enforcement are real and
tested; anything requiring licensed toolchains, external validators, or physical
hardware is recorded here as an open need — never approximated (stereo is never
a proxy, sync is never "probably fine", stage outputs are never actuated by
generated instructions).

`protocol/` is integrator-owned; where a needed op/view is proposed, the name is
a suggestion, not implemented schema.

## Spatial / licensed deliverables (W26)

1. **Dolby Atmos ADM BWF encode + external conformance validation.** The gate
   (`SpatialGate::check`) answers `LicensedRendererRequired` until an
   `Approved` validator record with `evidence_ref` covering `atmos-adm-bwf`
   exists. The metadata path is real (`ObjectContainerSpec` validates against
   the ADM 118-object bound, legal beds only); the bitstream encode and its QC
   are a licensed-toolchain need. Proposed protocol surface when the integrator
   takes this on: `spatial.check { path } -> { status, validator, reason }`.

2. **Dolby Atmos MP4 (IMM) encode.** Same treatment, separate scope
   `atmos-mp4` — codec/container permissions are distinct from ADM BWF and
   gated separately on purpose.

3. **Head-tracking hardware probing.** `Binaural { head_tracked: true }` is
   `HeadTrackingUnavailable` unless the environment declares probed hardware.
   There is no probe on this box and no pretending otherwise — the honest
   untracked binaural path is available; head-tracked waits for real hardware
   plus a `head_tracking_declared` environment fact supplied by the engine lane.

4. **Monitor reachability on real rigs.** `can_drive` is a speaker-set
   containment model — placement, not upmix. Wider-than-set content resolves
   through `DeclaredDownmix`/`Binaural`/`None` exactly as declared; a rig with
   no legal fallback is `MonitorUnreachable`, not "stereo, close enough".
   Physical trim verification (level-cal sweeps) is a hardware-measurement
   need — this lane provides the declared-config model and its strict
   validation only.

## Sync / show hardware (W27)

5. **MIDI wire endpoints (MTC/MIDI Clock/MMC).** `void-sync` implements the
   byte-exact codecs and the transport/arbiter models; there is no MIDI port
   plumbing on this box. Needed from the platform/engine lane: an endpoint
   abstraction carrying raw MIDI bytes to `QuarterFrameAssembler`/`demux` and
   taking encoded frames out, plus liveness timestamps for `sync_health`.
   Proposed: `sync.endpoint { id, kind }` + `sync.frame { endpoint, bytes }`.

6. **Ableton Link.** Deliberately a non-implementation — see
   `ADR-LINK.md`. `SyncSource::LinkPeer` exists so a Link master *claim* is
   typed and arbitrable; there is no peer stack. A real Link integration is a
   future work package requiring UDP discovery + a quantum/timeline engine
   hook, not something this lane can verify on Linux without the app.

7. **DMX output path.** `DmxRig`/`DmxUniverse`/`DmxFixture` are the real
   descriptor model (512-channel universes, overlap conflicts, the 10 Hz
   strobe bound per W27's seizure-safety clause). No ENTTEC/Art-Net/sACN
   output driver exists — PANIC maps to `universe.blackout()` in the model
   and the physical equivalent (all channels zeroed, output hold) needs the
   transport layer. Proposed op family: `show.dmx.set { universe, address,
   value }`, `show.dmx.blackout { universe }` — emitted by the coordinator on
   panic/disconnect *unconditionally*.

8. **OSC transport.** The address-space model + bounded validation are real
   (`OscAddress`, `validate_against`); UDP sockets are not. Needed: endpoint
   lifecycle + a bound-control registry enforcing `OscControl` specs at
   ingress — a message that fails bounds must be dropped and logged, never
   dispatched, which the model already enforces at the seam.

9. **PANIC → physical output release wiring.** `show`'s panic is complete in
   the model (armed pairs released, queue dropped, blackout intents per
   output kind, idempotent, sync-independent). The last mile — turning those
   intents into zeroed DMX frames, stopped MMC, released OSC claims — needs
   the endpoint layer of items 5/7/8. The contract is already enforceable:
   nothing outside the manual panic path may release output state.

10. **Hardware-validated show runs.** W27 done-when says sync/show fixtures
    pass *on declared devices* — no declared MIDI/DMX/OSC devices exist on
    this lane's box, so per T94 the honest state is: model-level fixtures pass;
    on-device fixtures remain open pending the transports above.
