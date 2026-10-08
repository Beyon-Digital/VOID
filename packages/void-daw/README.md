# void-daw

Presentation-layer DAW pieces for VOID: the `SessionView` clip-matrix
renderer and browser-side audio helpers.

## Honesty note — `WebAudioAdapter` is NOT the audio engine

`adapters/WebAudioAdapter.ts` is a stub over the browser `AudioContext` API
left over from the pre-engine prototype. It **must never be counted as
implemented audio**:

- `getAnalyserData` returns a zero-filled buffer; `getPeakLevel` returns
  `{peak: 0, rms: 0}`. Nothing is connected to the engine's meter graph.
- Transport methods (`play`/`stop`/`pause`/`record`) do not issue engine
  transport commands; track/clip mutators do not issue `send_command` ops.
- Real audio lives in the C++/Tracktion worker (`native/`, lane A) and real
  transport/metering flows over `void://control` + `void://telemetry`
  (`packages/void-client`).

If a component renders WebAudioAdapter numbers as live meters, that is a bug
— meters must render whatever the telemetry channel actually delivers.

## Ownership

The WebView holds **only** transient view state (selection, zoom, viewport,
panel layout). `SessionView` renders a `ClipMatrix` that callers derive from
`read_view` pages — see `adapters/readProjection.ts`
(`clipMatrixFromReadItems`). The matrix is a render projection, not a
document clone: rebuild it from the next read page, never mutate it as
state.
