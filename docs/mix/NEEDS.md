# NEEDS — mixer/automation/MIDI gaps found by the W18 lane

Same convention as `docs/engine/NEEDS.md`: numbered append-only entries;
`protocol/` is integrator-owned so this file records what W18 needed vs
what `void_control.fbs` / the PersistentOp union carries in major.1.
Names below are proposals, not implemented schema.

## Routing graph (T69)

M1. **No send/route ops.** `crates/void-mix` models sends, aux buses,
    multi-out nodes, external-I/O descriptors and VCA groups as a pure
    graph with loop rejection — but nothing on the wire can create one.
    Needed: `InsertSendOp{from_track,to_track,tap,kind,gain,pan,
    downmix}`, `RemoveSendOp`, `InsertAuxBusOp` (or a `kind` on
    `AddTrackOp`), `SetNodeLatencyOp{node, samples, measured}` and a
    `ROUTING` ViewKind so the client can rebuild the graph from project
    truth instead of a client-side model.

M2. **No channel-layout field on tracks/nodes.** `AddTrackOp` has
    `kind`/`name` but no channel count, so the `ChannelLayout` legality
    matrix can't be enforced on the wire. Needed: `layout` on
    `AddTrackOp` (mono/stereo/quad/5.1/7.1) + `SetTrackLayoutOp`.

M3. **No PDC readback.** Even once sends exist, nothing reports the
    engine's computed per-path latency or applied compensation. Needed:
    `CompensationPlan` view (topo order, edge delays, accumulated per
    node, unmeasured flags) matching `void_mix::CompensationPlan` so UI
    can show actual alignment rather than the model's prediction.

## Automation (T70)

M4. **No automation-point or lane ops.** The lane curve, its mode, and
    its trim layer have no PersistentOp — `automation/` keeps them as
    view-state models and write passes emit real *parameter* ops, but
    the recorded curve itself cannot persist. Needed:
    `InsertAutomationPointOp{lane, ticks, value}`,
    `RemoveAutomationPointsOp{lane, range}`,
    `SetAutomationModeOp{lane, mode}`, `SetAutomationInterpOp`,
    and a lane identity (`AutomationLaneId` on a param ref) in the
    schema.

M5. **No param-ref addressing.** `ParamRef{trackId, kind, paramId}` is
    client-side; the wire can set gain/pan/mute/plugin params but has
    no generic "this curve drives this param" declaration. Needed: a
    `ParamRef` DTO the other lanes can reuse.

M6. **A/B toggle has no snapshot read.** `abFlipOps` writes the other
    side's values, but the client must already know them — there is no
    `SNAPSHOT`/`STATE_DUMP` view to capture "all mixer params now".
    Needed: a mixer-state view page so a real A/B compare can capture A
    before flipping to B.

## MIDI (T71)

M7. **No expression/MPE ops.** `InsertNoteOp`/`SetNoteOp` cover pitch,
    velocity and geometry; per-note bend/pressure/CC (the MPE model)
    and channel-level events have no wire form. Needed:
    `SetNoteExpressionOp{note_id, kind, points}` +
    `InsertChannelEventOp{clip_id, kind, channel, ticks, value}` and an
    `MPE_ZONE` view (master/member channel assignment).

M8. **No articulation-set persistence.** `ArticulationSet` +
    `keyswitchesFor` are client models; a project's articulation maps
    would need a document area or `SetArticulationOp{note_id,
    articulation_id}` plus a per-clip set reference.

M9. **No SMF/MIDI-file export op.** `flattenToMidi1` produces the
    event surface with explicit `MpeLoss` entries, but writing a .mid
    needs an export-family op returning an asset id (same shape as the
    proposed `RenderTrackOp` in engine NEEDS #29).

M10. **MIDI 2.0 transport does not exist and is not claimed.**
    `MIDI2_TRANSPORT` is `false` by contract; `midi2Descriptor` is a
    declared model for forward work (per-note 32-bit expression rides
    the note, no channel allocation). Any wire claim needs a schema
    major bump carrying UMP descriptors — until then nothing must
    pretend MIDI 2.0 events reach a device.
