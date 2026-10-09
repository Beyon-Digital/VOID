# ADR — Ableton Link: typed source, no implementation (W27)

**Status:** Accepted — Link is modelled as a sync source and deliberately not
implemented in this lane.

## Context

TEST_MATRIX T94: "No competing transport master, drift and latency measured;
ordinary local transport works without network sync." W27 names Link among the
sync surfaces. A real Ableton Link integration requires a UDP discovery/
measurement service plus a continuous quantum/timeline coupling to the audio
engine — the parts that make Link *work* are exactly the parts this Linux lane
cannot verify honestly (no audio engine timing, no network peers, no device).

## Decision

- `SyncSource::LinkPeer { session }` exists in `void-sync`: a Link master claim
  is typed, arbitrable, and produces a real conflict record like any other
  source — the "no competing master" rule covers Link without pretending it
  exists.
- `link.rs` exposes `LinkAvailability::NotImplemented` — the honest state.
- No peer discovery, no quantum math, no half-verified "Link support" flag.

## Consequences

- Claiming `LinkPeer` master works through the arbiter but the source can never
  produce traffic — drift/liveness will report it `Lost`/`NoMaster` honestly
  rather than faking a session.
- A future Link package plugs in at: peer stack (discovery + quantum) →
  `SyncSource::LinkPeer` claims → `DriftModel` measurement. The model is
  already correct; what's missing is the transport it measures.
- Recorded as docs/show/NEEDS.md item 6.
