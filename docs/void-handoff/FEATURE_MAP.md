# VOID — Feature traceability

Generated from `tracking/FEATURE_TRACEABILITY.json`. All 116 original IDs and 22 stock-tool outcome groups are preserved. Original vendor/feature descriptions are historical comparison requirements; equivalent original musical outcomes are the implementation target, not copying Apple names/assets.

## Phase refinements

- Native plugin proof moves to F0 with usable VST3 in F1; full format/isolation depth remains F3.

- Foundational streaming/save correctness is required early even when specialist engine feature depth is F3.

- WASM/MIDI scripting hardening is F5, superseding earlier F4 scheduling; no feature is deleted.

- Optional camera conducting stays F4 after pointer/touch gestures, despite its dependency catalog earliest phase F2.

- F3 and F4 lanes may proceed independently once their listed prerequisites pass; F5 completion depends on all required outcomes.

## Projects and recovery

### DOC-01 · Project create, open, save and templates

**Original stage/status:** F1 / Contract only · **Execution:** W05, W11 (F0, F1) · **Current:** partial

**Required build outcome:** Versioned document, project folder, recent-project picker, atomic save and useful default templates.

**Individual acceptance:** Create a project; save; restart; reopen with the same musical result.

**Source dependency note:** F0 document schema; F1 asset store

**Integration tests:** T17, T18, T19, T20, T21, T44, T45, T46, T47, T48

**Source/evidence IDs:** A02, A18, R03, R09, R20

### DOC-02 · Project alternatives and track alternatives

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Branch revisions without duplicating immutable media; preserve plugin states per alternative.

**Individual acceptance:** Switch alternatives and return without changing the source take.

**Source dependency note:** F0 document schema; F1 asset store

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A02, A18, R03, R09, R20

### DOC-03 · Backups, undo and redo

**Original stage/status:** F1 / Not found · **Execution:** W05, W09 (F0, F1) · **Current:** verified

**Required build outcome:** Command journal, bounded undo snapshots, crash recovery and explicit destructive-change confirmation.

**Individual acceptance:** Crash during each save step; recover either the last complete revision or its successor.

**Source dependency note:** F0 document schema; F1 asset store

**Integration tests:** T17, T18, T19, T20, T21, T36, T37, T38, T39

**Source/evidence IDs:** A02, A18, R03, R09, R20

### DOC-04 · Consolidate, relink and clean media

**Original stage/status:** F3 / Not found · **Execution:** W21 (F3) · **Current:** verified

**Required build outcome:** Content-hashed assets, portable project archive, missing-file resolver and reference-aware cleanup.

**Individual acceptance:** Move a project to a new machine; never remove assets referenced by an alternative.

**Source dependency note:** F0 document schema; F1 asset store

**Integration tests:** T78, T79, T80

**Source/evidence IDs:** A02, A18, R03, R09, R20

### DOC-05 · Project/track notes and project information

**Original stage/status:** F1 / Not found · **Execution:** W11 (F1) · **Current:** partial

**Required build outcome:** Local notes, credits, sample rate, tempo/key summary, used assets and storage totals.

**Individual acceptance:** Notes survive save/reopen; metadata matches the document, not stale UI state.

**Source dependency note:** F0 document schema; F1 asset store

**Integration tests:** T44, T45, T46, T47, T48

**Source/evidence IDs:** A02, A18, R03, R09, R20

### DOC-06 · Import tracks/settings from another project

**Original stage/status:** F3 / Not found · **Execution:** W21 (F3) · **Current:** verified

**Required build outcome:** Merge selected tracks, routing, assets and plugin states with ID remapping and preview.

**Individual acceptance:** No ID collisions or dangling sends after repeated imports.

**Source dependency note:** F0 document schema; F1 asset store

**Integration tests:** T78, T79, T80

**Source/evidence IDs:** A02, A18, R03, R09, R20

## Audio engine and hardware

### ENG-01 · Audio device, input/output and buffer setup

**Original stage/status:** F1 / Not found · **Execution:** W04, W08 (F0, F1) · **Current:** partial

**Required build outcome:** Native device selection, channel configuration, sample-rate negotiation and a device-loss state. Certify macOS first; stage Windows/Linux separately.

**Individual acceptance:** Record/play on the declared interface matrix; recover safely after unplugging an interface.

**Source dependency note:** F0 engine proof; document-to-engine adapter

**Integration tests:** T13, T14, T15, T16, T31, T32, T33, T34, T35

**Source/evidence IDs:** A02, A07, A13, R02, R12, R19

### ENG-02 · Sample-accurate transport, seek, pause and stop

**Original stage/status:** F1 / Stub · **Execution:** W04 (F0) · **Current:** partial

**Required build outcome:** One audio clock, scheduled events, tempo conversion, note chase, click-free seek and explicit stop semantics.

**Individual acceptance:** Offline event positions agree with fixtures within one sample; no stuck notes on seek.

**Source dependency note:** F0 engine proof; document-to-engine adapter

**Integration tests:** T13, T14, T15, T16

**Source/evidence IDs:** A02, A07, A13, R02, R12, R19

### ENG-03 · Low-latency monitoring and recording alignment

**Original stage/status:** F1 / Not found · **Execution:** W08 (F1) · **Current:** verified

**Required build outcome:** Record-arm, input monitor, measured input/output compensation and documented low-latency bypass rules.

**Individual acceptance:** Loopback impulse test reports actual round-trip and recorded alignment error.

**Source dependency note:** F0 engine proof; document-to-engine adapter

**Integration tests:** T31, T32, T33, T34, T35

**Source/evidence IDs:** A02, A07, A13, R02, R12, R19

### ENG-04 · Plugin delay compensation and sidechain timing

**Original stage/status:** F3 / Not found · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Latency propagation through tracks, buses, sends, multi-output instruments and external inserts.

**Individual acceptance:** Impulse/null fixtures stay aligned when insert latency changes during playback.

**Source dependency note:** F0 engine proof; document-to-engine adapter

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A02, A07, A13, R02, R12, R19

### ENG-05 · Disk streaming, CPU meters, freeze and bounce-in-place

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Bounded read-ahead/cache, xruns telemetry, frozen renders and reversible offline processing.

**Individual acceptance:** Stress sessions exceed RAM-sized media safely; unfreeze restores editable state.

**Source dependency note:** F0 engine proof; document-to-engine adapter

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A02, A07, A13, R02, R12, R19

### ENG-06 · Mono, stereo, dual-mono and multichannel processing

**Original stage/status:** F3 / Not found · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Typed channel layouts, explicit conversion nodes, pan laws and format validation.

**Individual acceptance:** No silent channel loss; reject invalid graph connections before playback.

**Source dependency note:** F0 engine proof; document-to-engine adapter

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A02, A07, A13, R02, R12, R19

## Transport and global musical structure

### TIME-01 · Tempo, meter, key and cycle loop

**Original stage/status:** F1 / Stub · **Execution:** W04, W08 (F0, F1) · **Current:** partial

**Required build outcome:** Implement BPM/meter controls, musical ruler, loop boundaries and key metadata rather than empty setters.

**Individual acceptance:** A 7/8 loop repeats correctly; stopping sends note-off for sustained MIDI.

**Source dependency note:** F1 transport; stable timebase

**Integration tests:** T13, T14, T15, T16, T31, T32, T33, T34, T35

**Source/evidence IDs:** A02, A27, R02, R03, R24

### TIME-02 · Metronome, count-in and punch boundaries

**Original stage/status:** F1 / Not found · **Execution:** W08 (F1) · **Current:** verified

**Required build outcome:** Configurable click and count-in scheduled by the engine; distinguish pre-roll from recorded material.

**Individual acceptance:** Count-in and first recorded note align at every supported buffer size.

**Source dependency note:** F1 transport; stable timebase

**Integration tests:** T31, T32, T33, T34, T35

**Source/evidence IDs:** A02, A27, R02, R03, R24

### TIME-03 · Tempo/signature maps and tempo curves

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Versioned piecewise tempo and meter maps; convert ticks to samples and back deterministically.

**Individual acceptance:** Round-trip conversions remain correct across tempo ramps and meter changes.

**Source dependency note:** F1 transport; stable timebase

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A02, A27, R02, R03, R24

### TIME-04 · Markers, arrangement sections and global edits

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Named sections with move/copy/repeat operations that update clips, automation, chords and visuals together.

**Individual acceptance:** Moving a chorus preserves every linked lane and supports single-step undo.

**Source dependency note:** F1 transport; stable timebase

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A02, A27, R02, R03, R24

### TIME-05 · Chord track, region chords and harmonic follow

**Original stage/status:** F2 / Not found · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Structured chord objects with local overrides, voicing policy and explicit follow behavior.

**Individual acceptance:** A region override does not change unrelated accompaniment or original notes.

**Source dependency note:** F1 transport; stable timebase

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A02, A27, R02, R03, R24

### TIME-06 · Smart Tempo, beat mapping and groove track

**Original stage/status:** F3 / Contract only · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Analysis jobs produce editable beat/downbeat/tempo maps; allow keep/adapt choices and manual correction.

**Individual acceptance:** Bad beat detection can be repaired without stretching the original file irreversibly.

**Source dependency note:** F1 transport; stable timebase

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A02, A27, R02, R03, R24

## Recording and takes

### REC-01 · Audio recording: mono/stereo and multitrack

**Original stage/status:** F1 / Stub · **Execution:** W08 (F1) · **Current:** verified

**Required build outcome:** Stream input to recoverable files; arm per track; show actual waveforms and input levels.

**Individual acceptance:** Record two inputs simultaneously, stop, reopen and export both without lost samples.

**Source dependency note:** F1 engine + assets; F3 take graph

**Integration tests:** T31, T32, T33, T34, T35

**Source/evidence IDs:** A04, A03, R02, R03, R25

### REC-02 · MIDI and software-instrument recording

**Original stage/status:** F1 / Contract only · **Execution:** W08 (F1) · **Current:** verified

**Required build outcome:** Timestamp notes/CC, route devices/channels, render a built-in instrument and record MIDI separately from audio.

**Individual acceptance:** Recorded notes, sustain and pitch bend replay correctly after save/reopen.

**Source dependency note:** F1 engine + assets; F3 take graph

**Integration tests:** T31, T32, T33, T34, T35

**Source/evidence IDs:** A04, A03, R02, R03, R25

### REC-03 · Overdub, replace, step input and note repeat

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Explicit recording modes with clear destination, source retention and configurable note-repeat clock.

**Individual acceptance:** Each mode produces the expected event set with undo, including loop crossings.

**Source dependency note:** F1 engine + assets; F3 take graph

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A04, A03, R02, R03, R25

### REC-04 · Loop takes, take folders and punch recording

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Non-destructive take lanes, take IDs, punch ranges and compensating crossfades.

**Individual acceptance:** A failed take never overwrites an earlier recorded file.

**Source dependency note:** F1 engine + assets; F3 take graph

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A04, A03, R02, R03, R25

### REC-05 · Quick-swipe-style comping and take editing

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Audition lanes, select comp segments, edit seams, duplicate comps and reversible flatten.

**Individual acceptance:** Comp seams render without gaps/clicks on the test corpus; source takes remain available.

**Source dependency note:** F1 engine + assets; F3 take graph

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A04, A03, R02, R03, R25

### REC-06 · Flashback capture: recent MIDI and audio

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Consent-aware bounded capture buffers, clear armed-state indication and recovery into takes.

**Individual acceptance:** Capture the intended recent passage; purge disabled buffers and respect disk limits.

**Source dependency note:** F1 engine + assets; F3 take graph

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A04, A03, R02, R03, R25

## Tracks and linear arrangement

### ARR-01 · Track list, timeline, waveform and region editing

**Original stage/status:** F1 / Not found · **Execution:** W09 (F1) · **Current:** verified

**Required build outcome:** Build the actual arrangement workspace: add/select/move/split/trim/copy/delete clips with zoom, scroll and sample-aware waveforms.

**Individual acceptance:** Make a complete short arrangement using only manual tools and keyboard commands.

**Source dependency note:** F1 document/transport; F3 section-aware edits

**Integration tests:** T36, T37, T38, T39

**Source/evidence IDs:** A26, A02, R03, R19, R01

### ARR-02 · Snap, marquee, alignment guides and drag modes

**Original stage/status:** F1 / Not found · **Execution:** W09 (F1) · **Current:** verified

**Required build outcome:** A common edit-intent layer for pointer/keyboard, grid-relative and absolute snaps, overlap modes and precise numeric input.

**Individual acceptance:** Zoom level never changes the committed musical position.

**Source dependency note:** F1 document/transport; F3 section-aware edits

**Integration tests:** T36, T37, T38, T39

**Source/evidence IDs:** A26, A02, R03, R19, R01

### ARR-03 · Region loops, repeats, aliases and folders

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Distinguish linked instances from independent copies; support explicit flattening and nested arrangement groups.

**Individual acceptance:** Editing an alias follows documented propagation; copying does not accidentally link.

**Source dependency note:** F1 document/transport; F3 section-aware edits

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A26, A02, R03, R19, R01

### ARR-04 · Region gain, mute, solo, reverse and delay

**Original stage/status:** F1 / Stub · **Execution:** W09 (F1) · **Current:** verified

**Required build outcome:** Non-destructive per-clip transforms, visible gain and clearly scoped clip-versus-track control.

**Individual acceptance:** A reversed/gain-edited clip leaves its source asset hash unchanged.

**Source dependency note:** F1 document/transport; F3 section-aware edits

**Integration tests:** T36, T37, T38, T39

**Source/evidence IDs:** A26, A02, R03, R19, R01

### ARR-05 · Fades, crossfades and silence removal

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Editable fade curves, automatic seam fades, threshold-based strip-silence analysis and audition.

**Individual acceptance:** Selection boundaries and fades remain sample-consistent after trimming and stretching.

**Source dependency note:** F1 document/transport; F3 section-aware edits

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A26, A02, R03, R19, R01

### ARR-06 · Track stacks, groups, hide/protect and search

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Folder and summing stacks, group-scoped edits, hidden-track search and protected-track enforcement.

**Individual acceptance:** Group edits cannot modify locked tracks; routing survives stack reordering.

**Source dependency note:** F1 document/transport; F3 section-aware edits

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A26, A02, R03, R19, R01

### ARR-07 · Batch region processing and render-in-place

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Preview a processing chain; render to new assets; retain originals and undo relationships.

**Individual acceptance:** Cancel or failure never replaces the source clip with a partial render.

**Source dependency note:** F1 document/transport; F3 section-aware edits

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A26, A02, R03, R19, R01

## Audio correction and sample editing

### EDIT-01 · Flex Time: transient and sustained-material stretching

**Original stage/status:** F3 / Contract only · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Integrate or license stretch DSP; expose transient/tonal/polyphonic strategies, warp anchors and quality modes.

**Individual acceptance:** Blind listening and transient/tuning fixtures meet declared limits, not just a UI demo.

**Source dependency note:** F3 DSP pipeline + offline jobs

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A06, A12, R02, R20, R23

### EDIT-02 · Flex Pitch: note-level audio pitch editing

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Pitch analysis, correction curves, drift/vibrato/formant controls and reversible regional edits.

**Individual acceptance:** Monophonic vocal fixtures retain boundaries; uncertain detections are editable.

**Source dependency note:** F3 DSP pipeline + offline jobs

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A06, A12, R02, R20, R23

### EDIT-03 · Varispeed and linked/unlinked time-pitch changes

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Separate project speed, tempo adaptation and pitch shifting with explicit render semantics.

**Individual acceptance:** Playback and export agree under each mode and tempo map.

**Source dependency note:** F3 DSP pipeline + offline jobs

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A06, A12, R02, R20, R23

### EDIT-04 · Audio-file editing and repair tools

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Sample editor, zero-crossing snaps, normalize, DC removal, phase invert, silence and protected source copies.

**Individual acceptance:** Edits have a visible history and cannot corrupt the original audio asset.

**Source dependency note:** F3 DSP pipeline + offline jobs

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A06, A12, R02, R20, R23

### EDIT-05 · Transient detection, drum replacement and audio-to-MIDI

**Original stage/status:** F3 / Not found · **Execution:** W15, W19 (F2, F3) · **Current:** partial

**Required build outcome:** Onset/pitch analysis into auditionable triggers or MIDI; distinguish monophonic transcription from full mixed-audio extraction.

**Individual acceptance:** False triggers can be removed; derived MIDI remains hand-editable.

**Source dependency note:** F3 DSP pipeline + offline jobs

**Integration tests:** T60, T61, T62, T72, T73, T74

**Source/evidence IDs:** A06, A12, R02, R20, R23

### EDIT-06 · Stem Splitter: vocals, drums, bass, guitar, piano, other

**Original stage/status:** F3 / Contract only · **Execution:** W15, W19 (F2, F3) · **Current:** partial

**Required build outcome:** Expand four-stem contract into capability-negotiated separation; six-way model is required for equivalent coverage.

**Individual acceptance:** Align every stem to the source; compare leakage/artifacts and reconstruction on licensed fixtures.

**Source dependency note:** F3 DSP pipeline + offline jobs

**Integration tests:** T60, T61, T62, T72, T73, T74

**Source/evidence IDs:** A06, A12, R02, R20, R23

## MIDI editing and performance data

### MIDI-01 · Piano roll: pitch, duration and velocity

**Original stage/status:** F1 / Not found · **Execution:** W09 (F1) · **Current:** verified

**Required build outcome:** Virtualized note editor, draw/select/move/resize, audition, numeric edits and multi-region display.

**Individual acceptance:** Notes edited manually sound and serialize exactly as displayed.

**Source dependency note:** F1 MIDI event model; F2 proposal engine

**Integration tests:** T36, T37, T38, T39

**Source/evidence IDs:** A05, A24, A27, R03, R20, R24, R25

### MIDI-02 · Timing/pitch quantization, swing and humanize

**Original stage/status:** F2 / Contract only · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Reversible timing/pitch transforms; amount/range controls; scale snapping can be disabled.

**Individual acceptance:** Keep raw performance data; changing strength never compounds earlier quantization.

**Source dependency note:** F1 MIDI event model; F2 proposal engine

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A05, A24, A27, R03, R20, R24, R25

### MIDI-03 · CC lanes, sustain, pitch bend and aftertouch

**Original stage/status:** F1 / Not found · **Execution:** W08, W09 (F1) · **Current:** verified

**Required build outcome:** Event lanes with interpolation, MIDI channel ownership and chase/reset rules.

**Individual acceptance:** Seek into sustained phrases without stuck notes or discontinuous CC state.

**Source dependency note:** F1 MIDI event model; F2 proposal engine

**Integration tests:** T31, T32, T33, T34, T35, T36, T37, T38, T39

**Source/evidence IDs:** A05, A24, A27, R03, R20, R24, R25

### MIDI-04 · Event List, Step Editor and MIDI Transform

**Original stage/status:** F3 / Not found · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Precise event table, controller lanes, filter/transform operations and dry-run counts.

**Individual acceptance:** Batch transforms affect only their selected event types and undo atomically.

**Source dependency note:** F1 MIDI event model; F2 proposal engine

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A05, A24, A27, R03, R20, R24, R25

### MIDI-05 · MPE and MIDI 2.0 data handling

**Original stage/status:** F3 / Not found · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Per-note expression model and adapters; establish format-specific capability tests before claiming MIDI 2.0 parity.

**Individual acceptance:** Save/reopen retains expression; unsupported devices receive explicit fallback, not silent loss.

**Source dependency note:** F1 MIDI event model; F2 proposal engine

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A05, A24, A27, R03, R20, R24, R25

### MIDI-06 · Articulation sets, key switches and external MIDI

**Original stage/status:** F3 / Not found · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Named articulations mapped to switches/channels/CC, device-port assignments and virtual MIDI routing.

**Individual acceptance:** Changing an articulation preserves its intended instrument event on export and replay.

**Source dependency note:** F1 MIDI event model; F2 proposal engine

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A05, A24, A27, R03, R20, R24, R25

### MIDI-07 · MIDI import/export and track/channel demix

**Original stage/status:** F1 / Not found · **Execution:** W08, W10 (F1) · **Current:** verified

**Required build outcome:** Standard MIDI file exchange with timing/meter/tempo handling and a conversion report.

**Individual acceptance:** Round-trip a multi-track fixture; disclose non-portable articulations/plugin states.

**Source dependency note:** F1 MIDI event model; F2 proposal engine

**Integration tests:** T31, T32, T33, T34, T35, T40, T41, T42, T43

**Source/evidence IDs:** A05, A24, A27, R03, R20, R24, R25

## Patterns and Live Loops

### PAT-01 · Step Sequencer: note and automation patterns

**Original stage/status:** F2 / Not found · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Per-step pitch/velocity/gate, repeats/ties/probability, independent row lengths and deterministic playback seed.

**Individual acceptance:** Reloading a seeded pattern preserves its output; editing a row does not shift others.

**Source dependency note:** F1 scheduler; explicit SceneSlot model

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A09, A10, R04, R05, R19

### PAT-02 · Pattern recording, variation and MIDI conversion

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Live/step input, row learning, chord-degree patterns, pattern presets and MIDI materialization.

**Individual acceptance:** Converted MIDI matches the rendered pattern within the selected time resolution.

**Source dependency note:** F1 scheduler; explicit SceneSlot model

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A09, A10, R04, R05, R19

### PAT-03 · Live Loops: track-by-scene cell grid

**Original stage/status:** F3 / UI only · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Replace clip-ID prefix inference with explicit cells referencing scene, track and clip; implement empty slots and drop targets.

**Individual acceptance:** Arbitrary UUIDs render in the correct cell; prefix-sharing IDs cannot misroute clips.

**Source dependency note:** F1 scheduler; explicit SceneSlot model

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A09, A10, R04, R05, R19

### PAT-04 · Quantized clip/scene launch and live capture

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Queued/playing/stopping states, launch quantization, scene tempo policy and capture performance into the arrangement.

**Individual acceptance:** Launches land on the next valid boundary; repeated triggers neither double audio nor lose note-offs.

**Source dependency note:** F1 scheduler; explicit SceneSlot model

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A09, A10, R04, R05, R19

### PAT-05 · Remix performance and controller triggering

**Original stage/status:** F3 / Not found · **Execution:** W17 (F3) · **Current:** verified

**Required build outcome:** Map hardware/touch controls to loop actions and effects; record performance automation with safety limits.

**Individual acceptance:** Disconnecting a controller releases held controls and leaves transport running safely.

**Source dependency note:** F1 scheduler; explicit SceneSlot model

**Integration tests:** T66, T67, T68

**Source/evidence IDs:** A09, A10, R04, R05, R19

## Mixing and automation

### MIX-01 · Mixer strips: gain, pan, mute, solo and real meters

**Original stage/status:** F1 / Stub · **Execution:** W10 (F1) · **Current:** verified

**Required build outcome:** Actual signal-path control; implement accessible fader/knob gestures, typed values and unclipped gain ranges.

**Individual acceptance:** Fader movement changes measured audio level, not only a painted value.

**Source dependency note:** F1 graph + parameter IDs; F3 PDC

**Integration tests:** T40, T41, T42, T43

**Source/evidence IDs:** A07, A08, R02, R10, R11

### MIX-02 · Insert chains, bypass and channel-strip presets

**Original stage/status:** F1 / Not found · **Execution:** W10 (F1) · **Current:** verified

**Required build outcome:** Ordered processor chains with state snapshots, A/B, copy/paste and clear wet/dry policy.

**Individual acceptance:** Presets restore sound and automation target IDs after reordering.

**Source dependency note:** F1 graph + parameter IDs; F3 PDC

**Integration tests:** T40, T41, T42, T43

**Source/evidence IDs:** A07, A08, R02, R10, R11

### MIX-03 · Sends, aux returns, subgroups and sidechains

**Original stage/status:** F3 / Stub · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Directed routing graph with pre/post sends, independent send pan, feedback-cycle policy and bus meters.

**Individual acceptance:** Graph validation blocks unintended feedback; isolated sends reach only their destinations.

**Source dependency note:** F1 graph + parameter IDs; F3 PDC

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A07, A08, R02, R10, R11

### MIX-04 · VCA, mixer groups, multi-output instruments and external I/O

**Original stage/status:** F3 / Not found · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Group control separate from audio summing; multi-output routing and measured external-hardware latency.

**Individual acceptance:** Group faders preserve relative levels; external insert timing is reported and compensated.

**Source dependency note:** F1 graph + parameter IDs; F3 PDC

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A07, A08, R02, R10, R11

### MIX-05 · Automation lanes and read/touch/latch/write modes

**Original stage/status:** F3 / Not found · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Track and region automation, parameter addressing, interpolation and explicit write-arm protection.

**Individual acceptance:** Changing modes records the intended gesture while unrelated automation remains unchanged.

**Source dependency note:** F1 graph + parameter IDs; F3 PDC

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A07, A08, R02, R10, R11

### MIX-06 · Automation trim, relative edits and region moves

**Original stage/status:** F3 / Not found · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Offset layers and transform rules for moving, copying and resizing automated regions.

**Individual acceptance:** Round-trip moves preserve curves; trim changes do not bake over the base lane.

**Source dependency note:** F1 graph + parameter IDs; F3 PDC

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A07, A08, R02, R10, R11

### MIX-07 · Smart Controls, MIDI learn and performance macros

**Original stage/status:** F2 / Not found · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Common bounded mapping registry for controls, MIDI, touch and gestures; multi-target macros with preview.

**Individual acceptance:** Every mapped input updates the same parameter; out-of-range values are rejected or clamped.

**Source dependency note:** F1 graph + parameter IDs; F3 PDC

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A07, A08, R02, R10, R11

### MIX-08 · Mixer undo and loudness-aware A/B

**Original stage/status:** F3 / Not found · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Include mix/plugin edits in the project transaction model; allow gain-matched comparisons.

**Individual acceptance:** Undo restores plugin state and routes as well as fader values.

**Source dependency note:** F1 graph + parameter IDs; F3 PDC

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A07, A08, R02, R10, R11

## Instruments, sampling and sound library

### SND-01 · Playable stock instruments and synthesizer patches

**Original stage/status:** F1 / Not found · **Execution:** W10 (F1) · **Current:** verified

**Required build outcome:** Ship an original basic poly synth, drum instrument and sampler with useful licensed patches; expand by inventory below.

**Individual acceptance:** All factory patches load offline and pass voice, sustain and note-off tests.

**Source dependency note:** F1 built-in DSP; F3 library/content pipeline

**Integration tests:** T40, T41, T42, T43

**Source/evidence IDs:** A19, A18, A03, R08, R19, R20

### SND-02 · Quick sampling, slicing and drum-pad mapping

**Original stage/status:** F2 / Not found · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Drop/import/record a sound, trim/slice, map notes and preserve source references.

**Individual acceptance:** Dragging generated or recorded audio creates a playable instrument without file duplication bugs.

**Source dependency note:** F1 built-in DSP; F3 library/content pipeline

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A19, A18, A03, R08, R19, R20

### SND-03 · Multisampling, zones, layers and round robins

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Streaming sampler with velocity/key ranges, groups, articulations and sample-relink diagnostics.

**Individual acceptance:** Large instruments stream reliably; missing samples have actionable recovery.

**Source dependency note:** F1 built-in DSP; F3 library/content pipeline

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A19, A18, A03, R08, R19, R20

### SND-04 · Granular, additive, spectral, wavetable and physical synthesis

**Original stage/status:** F5 / Not found · **Execution:** W28 (F5) · **Current:** partial

**Required build outcome:** Build or license independently designed engines; use the named instrument inventory as outcome coverage, not binary cloning.

**Individual acceptance:** Preset banks have musical review and CPU budgets for the declared voice count.

**Source dependency note:** F1 built-in DSP; F3 library/content pipeline

**Integration tests:** T97, T98

**Source/evidence IDs:** A19, A18, A03, R08, R19, R20

### SND-05 · Loop/sample browser and musical preview

**Original stage/status:** F2 / Not found · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Tag by tempo/key/instrument/mood; preview in project time; favorites and user-library indexing.

**Individual acceptance:** Audition does not modify the project and never loses the source licence record.

**Source dependency note:** F1 built-in DSP; F3 library/content pipeline

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A19, A18, A03, R08, R19, R20

### SND-06 · Downloadable sound packs and storage management

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Signed/checksummed manifests, resume, disk-space checks, relocation and reference-aware removal.

**Individual acceptance:** Offline packs remain usable; interrupted updates do not break existing projects.

**Source dependency note:** F1 built-in DSP; F3 library/content pipeline

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A19, A18, A03, R08, R19, R20

### SND-07 · User patches, Auto Sampler-style capture and import

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Save complete device states and assets; staged capture of permitted instruments with key/velocity maps.

**Individual acceptance:** Captured instrument has consistent zone names; external capture can be cancelled safely.

**Source dependency note:** F1 built-in DSP; F3 library/content pipeline

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A19, A18, A03, R08, R19, R20

## Audio and MIDI effect capabilities

### FX-01 · EQ, filtering and spectrum matching

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Parametric/linear-phase/match/vintage-style EQ, modulation filters and analyzers; initial basic EQ lands in F1.

**Individual acceptance:** Frequency and impulse responses match specifications; high-Q settings remain stable.

**Source dependency note:** F1 basic DSP; F3 processor state/PDC

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A20, A21, A22, A23, A24, R02, R08, R19

### FX-02 · Compression, limiting, gating, de-essing and dynamics

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Core compressor/limiter first; then multiband, sidechain, envelope shaping and adaptive dynamics.

**Individual acceptance:** Attack/release, true-peak and sidechain fixtures pass across sample rates.

**Source dependency note:** F1 basic DSP; F3 processor state/PDC

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A20, A21, A22, A23, A24, R02, R08, R19

### FX-03 · Reverb, delay and convolution

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Original algorithmic reverb and sync delay first; then convolution, multitap and advanced spaces.

**Individual acceptance:** Tempo changes, tails and plugin bypass cause no discontinuity or truncated exports.

**Source dependency note:** F1 basic DSP; F3 processor state/PDC

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A20, A21, A22, A23, A24, R02, R08, R19

### FX-04 · Distortion, saturation, amps and pedals

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Original waveshaping/modelled processing, cabinets and routable pedal rack; do not distribute Apple assets.

**Individual acceptance:** Oversampling/aliasing and output-level tests accompany sound-design review.

**Source dependency note:** F1 basic DSP; F3 processor state/PDC

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A20, A21, A22, A23, A24, R02, R08, R19

### FX-05 · Modulation, imaging and rhythmic multi-effects

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Chorus/flange/phase/rotor/tremolo, stereo tools, beat slicing and step-modulated effect rack.

**Individual acceptance:** Stereo phase behavior and tempo-synced transitions remain stable.

**Source dependency note:** F1 basic DSP; F3 processor state/PDC

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A20, A21, A22, A23, A24, R02, R08, R19

### FX-06 · Pitch correction, shifter and vocal transformation

**Original stage/status:** F3 / Not found · **Execution:** W19 (F3) · **Current:** partial

**Required build outcome:** Explicit scale/formant controls and quality/latency modes; distinguish correction from generated vocals.

**Individual acceptance:** Audition bypass with level matching; display latency and detection confidence.

**Source dependency note:** F1 basic DSP; F3 processor state/PDC

**Integration tests:** T72, T73, T74

**Source/evidence IDs:** A20, A21, A22, A23, A24, R02, R08, R19

### FX-07 · MIDI effects and sandboxed scripting

**Original stage/status:** F3 / Not found · **Execution:** W28 (F5) · **Current:** partial

**Required build outcome:** Arpeggiation, chord triggering, note-repeat, transformation and a bounded MIDI scripting API.

**Individual acceptance:** A bad script cannot stall audio, access private files or emit unbounded events.

**Source dependency note:** F1 basic DSP; F3 processor state/PDC

**Integration tests:** T97, T98

**Source/evidence IDs:** A20, A21, A22, A23, A24, R02, R08, R19

### FX-08 · Meters, tuner, gain, test signal and utility processors

**Original stage/status:** F1 / Stub · **Execution:** W10 (F1) · **Current:** verified

**Required build outcome:** Real peak/RMS meters, spectrum/tuning analysis and gain utility first; specialist multichannel tools follow.

**Individual acceptance:** Known sine/noise/impulse fixtures yield documented readings; no fabricated zero meters.

**Source dependency note:** F1 basic DSP; F3 processor state/PDC

**Integration tests:** T40, T41, T42, T43

**Source/evidence IDs:** A20, A21, A22, A23, A24, R02, R08, R19

## Third-party devices and extensibility

### HOST-01 · Audio Units v2/v3 instruments and effects

**Original stage/status:** F3 / Not found · **Execution:** W06, W20 (F0, F3) · **Current:** partial

**Required build outcome:** Add a native macOS host, discovery, state restore, automation, editor embedding and compatibility diagnostics.

**Individual acceptance:** Validated third-party effects/instruments survive save/reopen and offline bounce.

**Source dependency note:** F0 licence/API spike; F3 isolated host

**Integration tests:** T22, T23, T24, T75, T76, T77

**Source/evidence IDs:** A13, A27, R08, R12, R20

### HOST-02 · Plugin scan, validation and crash recovery

**Original stage/status:** F3 / Not found · **Execution:** W06, W20 (F0, F3) · **Current:** partial

**Required build outcome:** Scan in disposable processes; timeouts, quarantine list and safe re-scan. Full runtime isolation is separate work.

**Individual acceptance:** A deliberately crashing plugin is quarantined and cannot prevent the next studio startup.

**Source dependency note:** F0 licence/API spike; F3 isolated host

**Integration tests:** T22, T23, T24, T75, T76, T77

**Source/evidence IDs:** A13, A27, R08, R12, R20

### HOST-03 · ARA/editor integration and instrument multi-output

**Original stage/status:** F5 / Not found · **Execution:** W20, W28 (F3, F5) · **Current:** partial

**Required build outcome:** Assess supported native SDK capabilities and licensing; map edit state and all output buses.

**Individual acceptance:** Document supported plugin/OS combinations and unsupported combinations explicitly.

**Source dependency note:** F0 licence/API spike; F3 isolated host

**Integration tests:** T75, T76, T77, T97, T98

**Source/evidence IDs:** A13, A27, R08, R12, R20

### HOST-04 · WASM devices and controlled live reload

**Original stage/status:** F4 / Contract only · **Execution:** W28 (F5) · **Current:** partial

**Required build outcome:** Versioned capability manifest, bounded compute/memory, state migration and rollback to last known good module.

**Individual acceptance:** Malformed modules or parameter schemas are rejected without changing the project.

**Source dependency note:** F0 licence/API spike; F3 isolated host

**Integration tests:** T97, T98

**Source/evidence IDs:** A13, A27, R08, R12, R20

### HOST-05 · VST3 and CLAP hosting — VOID extension, not Logic feature

**Original stage/status:** F3 / Not found · **Execution:** W06, W20 (F0, F3) · **Current:** partial

**Required build outcome:** Consider cross-platform native hosts after the core plugin abstraction; license/SDK support must be verified.

**Individual acceptance:** Platform test matrix distinguishes hosted formats; no claim that Logic hosts VST3.

**Source dependency note:** F0 licence/API spike; F3 isolated host

**Integration tests:** T22, T23, T24, T75, T76, T77

**Source/evidence IDs:** A13, A27, R08, R12, R20

## Logic intelligent tools and VOID AI

### INTEL-01 · Session Players: drummer, bass, keyboard and synth styles

**Original stage/status:** F3 / Not found · **Execution:** W21 (F3) · **Current:** verified

**Required build outcome:** Controllable accompaniment that follows chords, groove and sections; store editable MIDI plus generation recipe.

**Individual acceptance:** Lock a phrase, regenerate elsewhere, and preserve the locked notes exactly.

**Source dependency note:** F1 editable notes/assets; F2 command/proposal gate

**Integration tests:** T78, T79, T80

**Source/evidence IDs:** A03, A11, A12, A27, R06, R20

### INTEL-02 · Chord ID and harmonic analysis

**Original stage/status:** F2 / Not found · **Execution:** W13, W14 (F2) · **Current:** verified

**Required build outcome:** Analyze selected audio/MIDI into tentative chord objects with confidence, manual correction and region scope.

**Individual acceptance:** Low-confidence chords remain suggestions; no automatic project-wide reharmonization.

**Source dependency note:** F1 editable notes/assets; F2 command/proposal gate

**Integration tests:** T53, T54, T55, T56, T57, T58, T59

**Source/evidence IDs:** A03, A11, A12, A27, R06, R20

### INTEL-03 · Mastering Assistant

**Original stage/status:** F3 / Not found · **Execution:** W21 (F3) · **Current:** verified

**Required build outcome:** Analysis-driven editable mastering-chain proposal, target loudness/true peak and level-matched audition.

**Individual acceptance:** Export meets the user-selected delivery target without hiding destructive processing.

**Source dependency note:** F1 editable notes/assets; F2 command/proposal gate

**Integration tests:** T78, T79, T80

**Source/evidence IDs:** A03, A11, A12, A27, R06, R20

### INTEL-04 · Predictive notes, drums, chords and phrase completion

**Original stage/status:** F2 / Not found · **Execution:** W13 (F2) · **Current:** verified

**Required build outcome:** Inline ghost material, alternative candidates, contextual audition, accept/dismiss and one-transaction commit.

**Individual acceptance:** Rejecting a suggestion changes nothing; accepted output is normal editable project data.

**Source dependency note:** F1 editable notes/assets; F2 command/proposal gate

**Integration tests:** T53, T54, T55, T56

**Source/evidence IDs:** A03, A11, A12, A27, R06, R20

### INTEL-05 · Arrangement and orchestration copilot

**Original stage/status:** F3 / Contract only · **Execution:** W21 (F3) · **Current:** verified

**Required build outcome:** Propose structured edits to selected tracks/ranges; explain scope and preview A/B against a saved revision.

**Individual acceptance:** Model output cannot mutate a newer revision without revalidation.

**Source dependency note:** F1 editable notes/assets; F2 command/proposal gate

**Integration tests:** T78, T79, T80

**Source/evidence IDs:** A03, A11, A12, A27, R06, R20

### INTEL-06 · Text/reference-to-audio: loops, one-shots and stems

**Original stage/status:** F2 / Contract only · **Execution:** W15 (F2) · **Current:** verified

**Required build outcome:** Async generation jobs with duration/key/tempo requirements, preview, crop/alignment and asset provenance.

**Individual acceptance:** Job completion never auto-replaces the selected clip; failed/cancelled jobs leave no project mutation.

**Source dependency note:** F1 editable notes/assets; F2 command/proposal gate

**Integration tests:** T60, T61, T62

**Source/evidence IDs:** A03, A11, A12, A27, R06, R20

### INTEL-07 · Audio continuation, inpainting and variations

**Original stage/status:** F3 / Not found · **Execution:** W21 (F3) · **Current:** verified

**Required build outcome:** Preserve protected regions; create alternative takes around defined musical boundaries and approved reference assets.

**Individual acceptance:** Only the approved selection changes after acceptance; original media remains intact.

**Source dependency note:** F1 editable notes/assets; F2 command/proposal gate

**Integration tests:** T78, T79, T80

**Source/evidence IDs:** A03, A11, A12, A27, R06, R20

### INTEL-08 · Local memory, preferences and reusable recipes

**Original stage/status:** F2 / Contract only · **Execution:** W12 (F2) · **Current:** verified

**Required build outcome:** Opt-in project-scoped musical preferences with source/revision provenance, editable memory and delete controls.

**Individual acceptance:** Private project context is never sent to a provider without explicit permission.

**Source dependency note:** F1 editable notes/assets; F2 command/proposal gate

**Integration tests:** T49, T50, T51, T52

**Source/evidence IDs:** A03, A11, A12, A27, R06, R20

### INTEL-09 · Model routing, downloads, cancellation and resource budgets

**Original stage/status:** F2 / Not found · **Execution:** W12 (F2) · **Current:** verified

**Required build outcome:** Capability registry for MIDI/audio/image/video; local-first policy, optional approved cloud and cached results.

**Individual acceptance:** Missing model, OOM, timeout and offline states have working manual fallbacks.

**Source dependency note:** F1 editable notes/assets; F2 command/proposal gate

**Integration tests:** T49, T50, T51, T52

**Source/evidence IDs:** A03, A11, A12, A27, R06, R20

## Gesture-first composition — VOID additions

### GEST-01 · Draw a melodic contour and tap a rhythm

**Original stage/status:** F2 / Not found · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Convert pointer/touch paths into proposed pitch/time/velocity events; optional scale and grid constraints.

**Individual acceptance:** User can inspect the raw gesture, edit the resulting notes and disable snapping.

**Source dependency note:** F1 parameter/event IDs; F2 gesture mapping

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A02, R01, R10, R11, R20, R25

### GEST-02 · Touch/trackpad macros and harmonic gestures

**Original stage/status:** F2 / Not found · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Pinch/rotate/swipe mappings for timbre, tension, voicing and density; explicit arm/clutch and reset.

**Individual acceptance:** Navigation gestures cannot accidentally record destructive automation.

**Source dependency note:** F1 parameter/event IDs; F2 gesture mapping

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A02, R01, R10, R11, R20, R25

### GEST-03 · MIDI/MPE expressive performance

**Original stage/status:** F3 / Contract only · **Execution:** W18 (F3) · **Current:** verified

**Required build outcome:** Use the same mapping targets as touch input, with configurable deadbands, smoothing and bounded output.

**Individual acceptance:** Hardware and on-screen controls produce equivalent parameter/event changes.

**Source dependency note:** F1 parameter/event IDs; F2 gesture mapping

**Integration tests:** T69, T70, T71

**Source/evidence IDs:** A02, R01, R10, R11, R20, R25

### GEST-04 · Camera-hand conducting, optional and local

**Original stage/status:** F4 / Not found · **Execution:** W23 (F4) · **Current:** partial

**Required build outcome:** On-device pose capture, calibration, confidence threshold and tracking-loss release; record music events, not video, by default.

**Individual acceptance:** Denied camera permission still leaves a fully usable studio; tracking loss causes no surprise cue.

**Source dependency note:** F1 parameter/event IDs; F2 gesture mapping

**Integration tests:** T84, T85, T86

**Source/evidence IDs:** A02, R01, R10, R11, R20, R25

### GEST-05 · Gesture recording, quantized launches and undo

**Original stage/status:** F2 / Not found · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Preserve raw input timestamps and optionally quantized musical results; macro changes may be recorded as automation.

**Individual acceptance:** One undo reverses a committed gesture phrase without deleting an earlier take.

**Source dependency note:** F1 parameter/event IDs; F2 gesture mapping

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A02, R01, R10, R11, R20, R25

### GEST-06 · Keyboard/accessibility equivalents for every gesture

**Original stage/status:** F2 / Not found · **Execution:** W14 (F2) · **Current:** verified

**Required build outcome:** Provide focusable controls, numeric entry, screen-reader labels and discoverable shortcuts.

**Individual acceptance:** A keyboard-only user completes the same composition and export workflow.

**Source dependency note:** F1 parameter/event IDs; F2 gesture mapping

**Integration tests:** T57, T58, T59

**Source/evidence IDs:** A02, R01, R10, R11, R20, R25

## Visual timeline and stage — VOID additions

### VIS-01 · Imported images, video and conventional visual presets

**Original stage/status:** F4 / Not found · **Execution:** W22 (F4) · **Current:** verified

**Required build outcome:** Visual tracks with layers, transforms, opacity, blend modes and licensed presets; all work without AI.

**Individual acceptance:** Offline project reopening restores source media, layering and timing.

**Source dependency note:** F1 clock/assets; F2 jobs; F4 renderer

**Integration tests:** T81, T82, T83

**Source/evidence IDs:** A16, R07, R06, R20, R26

### VIS-02 · Generated artwork, visual loops and video

**Original stage/status:** F4 / Not found · **Execution:** W23 (F4) · **Current:** partial

**Required build outcome:** Provider jobs create immutable assets; compare candidates and choose exact timeline ranges before insertion.

**Individual acceptance:** Generation failure cannot replace a working stage scene.

**Source dependency note:** F1 clock/assets; F2 jobs; F4 renderer

**Integration tests:** T84, T85, T86

**Source/evidence IDs:** A16, R07, R06, R20, R26

### VIS-03 · Audio-reactive visuals and beat-synced changes

**Original stage/status:** F4 / Contract only · **Execution:** W22, W23 (F4) · **Current:** partial

**Required build outcome:** Decimated audio features and musical-clock snapshots drive bounded render parameters; bake expensive material.

**Individual acceptance:** Visual load cannot increase audio xruns in the certified stress test.

**Source dependency note:** F1 clock/assets; F2 jobs; F4 renderer

**Integration tests:** T81, T82, T83, T84, T85, T86

**Source/evidence IDs:** A16, R07, R06, R20, R26

### VIS-04 · Scene graphs, shader presets and safe generated shaders

**Original stage/status:** F4 / Contract only · **Execution:** W23 (F4) · **Current:** partial

**Required build outcome:** Prefer validated declarative scenes; compile untrusted shaders in an isolated renderer with watchdog and rollback.

**Individual acceptance:** Invalid or expensive shader falls back to the last good scene without privileged access.

**Source dependency note:** F1 clock/assets; F2 jobs; F4 renderer

**Integration tests:** T84, T85, T86

**Source/evidence IDs:** A16, R07, R06, R20, R26

### VIS-05 · Preview, full-screen and multi-display outputs

**Original stage/status:** F4 / Contract only · **Execution:** W22 (F4) · **Current:** verified

**Required build outcome:** Independent preview/program output, monitor selection, output blackout and latency calibration.

**Individual acceptance:** Losing a display never stops the audio engine or exposes private UI on the stage output.

**Source dependency note:** F1 clock/assets; F2 jobs; F4 renderer

**Integration tests:** T81, T82, T83

**Source/evidence IDs:** A16, R07, R06, R20, R26

### VIS-06 · Projection mapping and show cues

**Original stage/status:** F5 / Contract only · **Execution:** W27 (F5) · **Current:** verified

**Required build outcome:** Surface calibration, per-output transforms, cue timing, rehearsable transitions and safe rollback.

**Individual acceptance:** Rehearsal exports/plays the same cue timeline; bad mappings can be bypassed immediately.

**Source dependency note:** F1 clock/assets; F2 jobs; F4 renderer

**Integration tests:** T94, T95, T96

**Source/evidence IDs:** A16, R07, R06, R20, R26

### VIS-07 · OSC/DMX integration and external show control

**Original stage/status:** F5 / Contract only · **Execution:** W27 (F5) · **Current:** verified

**Required build outcome:** Capability-scoped local adapters, explicit network/device pairing, rate limits and a safe-state policy.

**Individual acceptance:** Untrusted content cannot send network/hardware commands; fixtures fail safe on disconnect.

**Source dependency note:** F1 clock/assets; F2 jobs; F4 renderer

**Integration tests:** T94, T95, T96

**Source/evidence IDs:** A16, R07, R06, R20, R26

### VIS-08 · Audio-plus-video export and reproducible rendering

**Original stage/status:** F4 / Not found · **Execution:** W24 (F4) · **Current:** verified

**Required build outcome:** Freeze visual/audio assets, render using one timeline, mux with timebase and encoder validation.

**Individual acceptance:** Audio and video stay within the declared sync tolerance from start to end.

**Source dependency note:** F1 clock/assets; F2 jobs; F4 renderer

**Integration tests:** T87, T88, T89

**Source/evidence IDs:** A16, R07, R06, R20, R26

## Delivery and interoperability

### OUT-01 · Stereo/mono audio bounce and export

**Original stage/status:** F1 / Contract only · **Execution:** W10 (F1) · **Current:** verified

**Required build outcome:** PCM WAV/AIFF first; region/project range, sample rate, bit depth and tail options. Add compressed formats after codec review.

**Individual acceptance:** Exported files contain real non-silent audio and match project duration plus selected tail.

**Source dependency note:** F1 offline audio render; F3 interchange

**Integration tests:** T40, T41, T42, T43

**Source/evidence IDs:** A02, A14, A16, R09, R02, R20

### OUT-02 · Offline/realtime bounce, dither and normalization

**Original stage/status:** F3 / Not found · **Execution:** W21 (F3) · **Current:** verified

**Required build outcome:** Distinguish hardware-dependent real-time render from offline render; explicit dither/normalize and clipping report.

**Individual acceptance:** Repeated built-in renders match within the declared deterministic tolerance.

**Source dependency note:** F1 offline audio render; F3 interchange

**Integration tests:** T78, T79, T80

**Source/evidence IDs:** A02, A14, A16, R09, R02, R20

### OUT-03 · Batch stems, regions and instrument renders

**Original stage/status:** F3 / Not found · **Execution:** W21 (F3) · **Current:** verified

**Required build outcome:** Export aligned stems with routing/tail policy, filenames, channel metadata and a manifest.

**Individual acceptance:** Reassembled stems follow documented bus-effect semantics and remain sample-aligned.

**Source dependency note:** F1 offline audio render; F3 interchange

**Integration tests:** T78, T79, T80

**Source/evidence IDs:** A02, A14, A16, R09, R02, R20

### OUT-04 · AAF, Final Cut XML and MusicXML interchange

**Original stage/status:** F5 / Not found · **Execution:** W25 (F5) · **Current:** verified

**Required build outcome:** Implement separately tested import/export subsets with a conversion report and retained source files.

**Individual acceptance:** Unsupported items are reported, never silently dropped.

**Source dependency note:** F1 offline audio render; F3 interchange

**Integration tests:** T90, T91

**Source/evidence IDs:** A02, A14, A16, R09, R02, R20

### OUT-05 · Logic/GarageBand project migration

**Original stage/status:** F5 / Not found · **Execution:** W25 (F5) · **Current:** verified

**Required build outcome:** Start with stems, MIDI and tempo/chord metadata. Treat native .logicx/.band conversion as a separate researched compatibility project.

**Individual acceptance:** Do not claim exact Apple plugin/sound-library or project-state round-tripping.

**Source dependency note:** F1 offline audio render; F3 interchange

**Integration tests:** T90, T91

**Source/evidence IDs:** A02, A14, A16, R09, R02, R20

### OUT-06 · Surround routing and binaural monitoring

**Original stage/status:** F5 / Not found · **Execution:** W26 (F5) · **Current:** partial

**Required build outcome:** Multichannel buses, speaker maps, monitor downmixes, spatial panners and calibrated output.

**Individual acceptance:** Speaker/channel mapping and gain fixtures pass on documented layouts.

**Source dependency note:** F1 offline audio render; F3 interchange

**Integration tests:** T92, T93

**Source/evidence IDs:** A02, A14, A16, R09, R02, R20

### OUT-07 · Dolby Atmos beds, objects and ADM BWF

**Original stage/status:** F5 / Not found · **Execution:** W26 (F5) · **Current:** partial

**Required build outcome:** Evaluate licensed renderer/toolchain, object metadata, bed routing and ADM import/export as a dedicated programme.

**Individual acceptance:** Validate files with the selected toolchain; binaural stereo alone is not Atmos parity.

**Source dependency note:** F1 offline audio render; F3 interchange

**Integration tests:** T92, T93

**Source/evidence IDs:** A02, A14, A16, R09, R02, R20

### OUT-08 · Atmos MP4 QC, head tracking and spatial formats

**Original stage/status:** F5 / Not found · **Execution:** W26 (F5) · **Current:** partial

**Required build outcome:** Separate codec/rendering permissions, supported headphones/hardware and QC exports from basic surround.

**Individual acceptance:** Accepted format/device matrix is explicit; encoding and metadata survive validation.

**Source dependency note:** F1 offline audio render; F3 interchange

**Integration tests:** T92, T93

**Source/evidence IDs:** A02, A14, A16, R09, R02, R20

## Specialist workflows and product completeness

### PRO-01 · Notation editor, parts, lyrics and tablature

**Original stage/status:** F5 / Not found · **Execution:** W25 (F5) · **Current:** verified

**Required build outcome:** Notation derived from MIDI but with independent engraving data; staff styles, chord grids and print/export.

**Individual acceptance:** Visual quantization does not change playback; extracted parts retain intended notation.

**Source dependency note:** F3 stable studio; F5 specialist integrations

**Integration tests:** T90, T91

**Source/evidence IDs:** A15, A17, A02, R19, R20, R22

### PRO-02 · Movie scoring and absolute timecode

**Original stage/status:** F5 / Not found · **Execution:** W25 (F5) · **Current:** verified

**Required build outcome:** Reference video, frame-rate-aware ruler, offset, cues and soundtrack export with decoder isolation.

**Individual acceptance:** Frame/time conversion fixtures pass, including explicitly supported drop-frame modes.

**Source dependency note:** F3 stable studio; F5 specialist integrations

**Integration tests:** T90, T91

**Source/evidence IDs:** A15, A17, A02, R19, R20, R22

### PRO-03 · MTC, MIDI Clock output, MMC and Ableton Link

**Original stage/status:** F5 / Not found · **Execution:** W27 (F5) · **Current:** verified

**Required build outcome:** Explicit sync ownership and protocol adapters. Incoming MIDI Clock would be an extra capability, not claimed Logic parity.

**Individual acceptance:** Prevent conflicting clock sources; recover after external transport jumps.

**Source dependency note:** F3 stable studio; F5 specialist integrations

**Integration tests:** T94, T95, T96

**Source/evidence IDs:** A15, A17, A02, R19, R20, R22

### PRO-04 · Logic Remote-style companion and control surfaces

**Original stage/status:** F5 / Not found · **Execution:** W27 (F5) · **Current:** verified

**Required build outcome:** Paired remote controls with permissions, feedback and reconnect; stage macOS keyboard/MIDI support first.

**Individual acceptance:** A disconnected/stale controller cannot replay old destructive commands.

**Source dependency note:** F3 stable studio; F5 specialist integrations

**Integration tests:** T94, T95, T96

**Source/evidence IDs:** A15, A17, A02, R19, R20, R22

### PRO-05 · Custom shortcuts, screensets and flexible workspaces

**Original stage/status:** F3 / Not found · **Execution:** W21 (F3) · **Current:** verified

**Required build outcome:** Command palette, imported/merged keymaps, saved panel layouts and instrument/editor popouts.

**Individual acceptance:** Shortcut conflicts are reported; layouts restore on smaller displays without hidden controls.

**Source dependency note:** F3 stable studio; F5 specialist integrations

**Integration tests:** T78, T79, T80

**Source/evidence IDs:** A15, A17, A02, R19, R20, R22

### PRO-06 · Accessibility, help, onboarding and localization

**Original stage/status:** F1 / Not found · **Execution:** W09, W11 (F1) · **Current:** partial

**Required build outcome:** Readable meters, labelled controls, keyboard transport, contextual help and actionable missing-device/media states.

**Individual acceptance:** Complete record/edit/save/export journey passes keyboard and screen-reader checks.

**Source dependency note:** F3 stable studio; F5 specialist integrations

**Integration tests:** T36, T37, T38, T39, T44, T45, T46, T47, T48

**Source/evidence IDs:** A15, A17, A02, R19, R20, R22

### PRO-07 · Environment-style MIDI/modular routing

**Original stage/status:** F5 / Not found · **Execution:** W27 (F5) · **Current:** verified

**Required build outcome:** Consider a bounded declarative MIDI/device graph; no unrestricted scripting in the audio callback.

**Individual acceptance:** Cycles and unbounded event multiplication are rejected; the graph is editable without code.

**Source dependency note:** F3 stable studio; F5 specialist integrations

**Integration tests:** T94, T95, T96

**Source/evidence IDs:** A15, A17, A02, R19, R20, R22

### PRO-08 · Installer, updates, diagnostics and production QA

**Original stage/status:** F0 / Not found · **Execution:** W07, W29 (F0, F5) · **Current:** partial

**Required build outcome:** Pinned builds, real tests, CI, signed releases, crash recovery and privacy-preserving diagnostics; ship installers only after platform validation.

**Individual acceptance:** Fresh-machine installation and update/rollback preserve projects and never require development tools.

**Source dependency note:** F3 stable studio; F5 specialist integrations

**Integration tests:** T25, T26, T27, T28, T29, T30, T99, T100

**Source/evidence IDs:** A15, A17, A02, R19, R20, R22

## Named stock-tool coverage

Names below describe the original comparison inventory. Use original/licensed implementations and content, not copied Apple assets.

| ID | Family | Comparison inventory | Required musical outcome | Tasks |
| --- | --- | --- | --- | --- |
| STOCK-01 | Core synthesis | Alchemy; Retro Synth; ES1; ES2; EFM1; ES E; ES M; ES P | Cover subtractive, wavetable, FM, additive/spectral/granular and modulation outcomes with original engines; a useful compact synth ships before the advanced set. | W10, W19, W28 |
| STOCK-02 | Sampling | Quick Sampler; Sampler; Sample Alchemy | One-shot/loop/slice playback first; then streaming multisampling and sample resynthesis. Preserve user-sample licences and source paths. | W14, W19, W28 |
| STOCK-03 | Drums | Drum Kit Designer; Drum Machine Designer; Drum Synth; Ultrabeat | Acoustic kits, pad racks, synthesized percussion and pattern-oriented drums; add multi-output kits and velocity layers. | W10, W14, W19 |
| STOCK-04 | Acoustic and orchestral | Studio Bass; Studio Piano; Studio Strings; Studio Horns | Commission or license sample sets, expressive articulations, round robins and realistic performance controls. Code alone does not provide this content. | W19 |
| STOCK-05 | Vintage and modeled | Vintage B3 Organ; Vintage Clav; Vintage Electric Piano; Vintage Mellotron; Sculpture | Original/licensed tonewheel, keyboard/tape and physical-model synthesis. Match musical use cases, not Apple binaries or preset signatures. | W19, W28 |
| STOCK-06 | Vocoder and infrastructure | EVOC 20 PolySynth; External Instrument; Klopfgeist | Vocoder, external-hardware timing/routing and a controllable click instrument. | W08, W18, W19 |
| STOCK-07 | Legacy sound coverage | Bass; Church Organ; Drum Kits; Electric Clav(inet); Electric Piano; Guitar; Horns; Piano; Sound Effects; Strings; Tuned Percussion; Voice; Woodwind; Tonewheel Organ; Analog Basic/Mono/Pad/Swirl/Sync; Digital Basic/Mono/Stepper; Hybrid Basic/Morph | Cover legacy timbres through original presets/sample packs; no promise of legacy Apple project compatibility. | W19, W28 |
| STOCK-08 | Amps and pedal rack | Amp Designer; Bass Amp Designer; Pedalboard | Original amplifier/cabinet models, microphone placement abstractions and routable pedal effects. | W19 |
| STOCK-09 | Delays | Delay Designer; Echo; Sample Delay; Stereo Delay; Tape Delay | Short alignment delays, tempo-sync stereo/tape and multitap design with feedback limits and safe parameter smoothing. | W19 |
| STOCK-10 | Saturation and distortion | Bitcrusher; ChromaGlow; Clip Distortion; Distortion; Distortion II; Overdrive; Phase Distortion | Waveshaping and modeled saturation with oversampling, gain matching and clear quality modes. | W19 |
| STOCK-11 | Dynamics | Adaptive Limiter; Compressor; DeEsser 2; Enveloper; Expander; Limiter; Multipressor; Noise Gate; Surround Compressor | Dynamics family, sidechain filters, envelope/transient control, multiband and multichannel variants. | W19, W26 |
| STOCK-12 | Equalization | Channel EQ; Linear Phase EQ; Match EQ; Single Band EQ; Vintage Console EQ; Vintage Graphic EQ; Vintage Tube EQ | Parametric, phase-linear, spectral-match and character EQ with stable automation and response fixtures. | W19 |
| STOCK-13 | Filters and vocoding | AutoFilter; EVOC 20 Filterbank; EVOC 20 TrackOscillator; Fuzz-Wah; Spectral Gate | Resonant/formant filters, envelope/LFO modulation, audio tracking and spectral selection. | W19, W28 |
| STOCK-14 | Modulation | Chorus; Ensemble; Flanger; Microphaser; Modulation Delay; Phaser; RingShifter; Rotor Cabinet; Scanner Vibrato; Spreader; Tremolo | Delay-, phase-, ring- and rotary-based movement processors. Verify stereo correlation and gain as part of DSP acceptance. | W19 |
| STOCK-15 | Rhythmic multi-effects | Beat Breaker; Phat FX; Remix FX; Step FX | Time slicing, sequenced effects and touch performance macros, with editable automation and deterministic export. | W17, W19 |
| STOCK-16 | Pitch and voice | Pitch Correction; Pitch Shifter; Vocal Transformer | Pitch detection/correction, independent shift and formant transformation; display resulting latency. | W19 |
| STOCK-17 | Reverbs | ChromaVerb; Space Designer; Quantec Room Simulator; EnVerb; SilverVerb | Original algorithmic reverb, licensed impulse-response convolution and high-quality spaces. Do not copy Quantec/Apple implementation code. | W19 |
| STOCK-18 | Imaging and spatial | Binaural Post-Processing; Spatial Audio Monitoring; Direction Mixer; Stereo Spread; Dolby Atmos | Stereo and binaural utilities first; licensed object-based spatial work is a separate scope. | W26 |
| STOCK-19 | Meters and utilities | BPM Counter; Correlation Meter; Level Meter; Loudness Meter; MultiMeter; Surround MultiMeter; Tuner; Auto Sampler; Down Mixer; Gain; Multichannel Gain; I/O; Test Oscillator | Measurement, metering, capture and calibration tools; every reading must come from real audio, with documented units. | W10, W18, W19, W26 |
| STOCK-20 | MIDI processors | Arpeggiator; Chord Trigger; Modifier; Modulator; Note Repeater; Randomizer; Scripter; Transposer; Velocity Processor | Composable, deterministic event processors and bounded scripting with previewable output. | W14, W18, W28 |
| STOCK-21 | Mastering | Mastering Assistant | Propose an inspectable mastering chain and let the user set delivery targets; retain bypass and level-matched comparison. | W21 |
| STOCK-22 | Legacy effects | AVerb; Bass Amp; DeEsser; Denoiser; Ducker; DJ EQ; Fat EQ; Single-Band EQ; Silver EQ; GoldVerb; Grooveshifter; Guitar Amp Pro; PlatinumVerb | Retain a low-priority outcome-coverage backlog; prioritize current equivalents over matching old interfaces. | W19 |
