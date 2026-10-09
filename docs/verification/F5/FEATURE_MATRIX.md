# VOID — Supported feature & device matrix (F5 reconciliation)

Generated as part of Lane Y (W29) on `devin/void-lane-w29` from `tracking/TESTS.json` + `tracking/FEATURE_TRACEABILITY.json` @ `devin/void-implementation` `9099089`. Machine-readable source: `TRACE_AUDIT.json`.

## Legend — verdict classes (derived, never asserted by the ledger)

| Class | Meaning |
|---|---|
| `pass_linux` | every mapped acceptance test passed on Linux with recorded evidence |
| `pass_macos_engine` | every mapped test passed on the macOS engine lane (device-qualified) |
| `mixed_pass` | all mapped tests pass but split across Linux and macOS evidence |
| `model_side` | the lane shipped the honest model/policy/op layer; engine/GUI/hardware half is an open NEEDS entry |
| `partial_or_open` | at least one mapped test is partial, blocked or not_run — see open test ids |
| `not_run` | no mapped test has run |

**Ledger caveat:** `implementation_status` in FEATURE_TRACEABILITY.json is `not_verified` for all 116 features and 22 stock groups — sub-test ledgers were never maintained (F5-N01). The `derived` column below is computed from the mapped test rows and is the honest current state.

## Feature rows (116)

### Projects and recovery

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| DOC-01 | Project create, open, save and templates | not_verified | partial_or_open | T45, T47, T48 |
| DOC-02 | Project alternatives and track alternatives | not_verified | pass_linux | — |
| DOC-03 | Backups, undo and redo | not_verified | pass_linux | — |
| DOC-04 | Consolidate, relink and clean media | not_verified | pass_linux | — |
| DOC-05 | Project/track notes and project information | not_verified | partial_or_open | T45, T47, T48 |
| DOC-06 | Import tracks/settings from another project | not_verified | pass_linux | — |

### Audio engine and hardware

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| ENG-01 | Audio device, input/output and buffer setup | not_verified | partial_or_open | T13, T15, T16 |
| ENG-02 | Sample-accurate transport, seek, pause and stop | not_verified | partial_or_open | T13, T15, T16 |
| ENG-03 | Low-latency monitoring and recording alignment | not_verified | pass_macos_engine | — |
| ENG-04 | Plugin delay compensation and sidechain timing | not_verified | pass_linux | — |
| ENG-05 | Disk streaming, CPU meters, freeze and bounce-in-place | not_verified | pass_linux | — |
| ENG-06 | Mono, stereo, dual-mono and multichannel processing | not_verified | pass_linux | — |

### Transport and global musical structure

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| TIME-01 | Tempo, meter, key and cycle loop | not_verified | partial_or_open | T13, T15, T16 |
| TIME-02 | Metronome, count-in and punch boundaries | not_verified | pass_macos_engine | — |
| TIME-03 | Tempo/signature maps and tempo curves | not_verified | pass_linux | — |
| TIME-04 | Markers, arrangement sections and global edits | not_verified | pass_linux | — |
| TIME-05 | Chord track, region chords and harmonic follow | not_verified | pass_linux | — |
| TIME-06 | Smart Tempo, beat mapping and groove track | not_verified | pass_linux | — |

### Recording and takes

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| REC-01 | Audio recording: mono/stereo and multitrack | not_verified | pass_macos_engine | — |
| REC-02 | MIDI and software-instrument recording | not_verified | pass_macos_engine | — |
| REC-03 | Overdub, replace, step input and note repeat | not_verified | pass_linux | — |
| REC-04 | Loop takes, take folders and punch recording | not_verified | pass_linux | — |
| REC-05 | Quick-swipe-style comping and take editing | not_verified | pass_linux | — |
| REC-06 | Flashback capture: recent MIDI and audio | not_verified | pass_linux | — |

### Tracks and linear arrangement

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| ARR-01 | Track list, timeline, waveform and region editing | not_verified | pass_linux | — |
| ARR-02 | Snap, marquee, alignment guides and drag modes | not_verified | pass_linux | — |
| ARR-03 | Region loops, repeats, aliases and folders | not_verified | pass_linux | — |
| ARR-04 | Region gain, mute, solo, reverse and delay | not_verified | pass_linux | — |
| ARR-05 | Fades, crossfades and silence removal | not_verified | pass_linux | — |
| ARR-06 | Track stacks, groups, hide/protect and search | not_verified | pass_linux | — |
| ARR-07 | Batch region processing and render-in-place | not_verified | pass_linux | — |

### Audio correction and sample editing

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| EDIT-01 | Flex Time: transient and sustained-material stretching | not_verified | partial_or_open | T72, T74 |
| EDIT-02 | Flex Pitch: note-level audio pitch editing | not_verified | partial_or_open | T72, T74 |
| EDIT-03 | Varispeed and linked/unlinked time-pitch changes | not_verified | partial_or_open | T72, T74 |
| EDIT-04 | Audio-file editing and repair tools | not_verified | partial_or_open | T72, T74 |
| EDIT-05 | Transient detection, drum replacement and audio-to-MIDI | not_verified | partial_or_open | T72, T74 |
| EDIT-06 | Stem Splitter: vocals, drums, bass, guitar, piano, other | not_verified | partial_or_open | T72, T74 |

### MIDI editing and performance data

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| MIDI-01 | Piano roll: pitch, duration and velocity | not_verified | pass_linux | — |
| MIDI-02 | Timing/pitch quantization, swing and humanize | not_verified | pass_linux | — |
| MIDI-03 | CC lanes, sustain, pitch bend and aftertouch | not_verified | mixed_pass | — |
| MIDI-04 | Event List, Step Editor and MIDI Transform | not_verified | pass_linux | — |
| MIDI-05 | MPE and MIDI 2.0 data handling | not_verified | pass_linux | — |
| MIDI-06 | Articulation sets, key switches and external MIDI | not_verified | pass_linux | — |
| MIDI-07 | MIDI import/export and track/channel demix | not_verified | mixed_pass | — |

### Patterns and Live Loops

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| PAT-01 | Step Sequencer: note and automation patterns | not_verified | pass_linux | — |
| PAT-02 | Pattern recording, variation and MIDI conversion | not_verified | pass_linux | — |
| PAT-03 | Live Loops: track-by-scene cell grid | not_verified | pass_linux | — |
| PAT-04 | Quantized clip/scene launch and live capture | not_verified | pass_linux | — |
| PAT-05 | Remix performance and controller triggering | not_verified | pass_linux | — |

### Mixing and automation

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| MIX-01 | Mixer strips: gain, pan, mute, solo and real meters | not_verified | pass_linux | — |
| MIX-02 | Insert chains, bypass and channel-strip presets | not_verified | pass_linux | — |
| MIX-03 | Sends, aux returns, subgroups and sidechains | not_verified | pass_linux | — |
| MIX-04 | VCA, mixer groups, multi-output instruments and external I/O | not_verified | pass_linux | — |
| MIX-05 | Automation lanes and read/touch/latch/write modes | not_verified | pass_linux | — |
| MIX-06 | Automation trim, relative edits and region moves | not_verified | pass_linux | — |
| MIX-07 | Smart Controls, MIDI learn and performance macros | not_verified | pass_linux | — |
| MIX-08 | Mixer undo and loudness-aware A/B | not_verified | pass_linux | — |

### Instruments, sampling and sound library

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| SND-01 | Playable stock instruments and synthesizer patches | not_verified | pass_linux | — |
| SND-02 | Quick sampling, slicing and drum-pad mapping | not_verified | pass_linux | — |
| SND-03 | Multisampling, zones, layers and round robins | not_verified | partial_or_open | T72, T74 |
| SND-04 | Granular, additive, spectral, wavetable and physical synthesis | not_verified | partial_or_open | T98 |
| SND-05 | Loop/sample browser and musical preview | not_verified | pass_linux | — |
| SND-06 | Downloadable sound packs and storage management | not_verified | partial_or_open | T72, T74 |
| SND-07 | User patches, Auto Sampler-style capture and import | not_verified | partial_or_open | T72, T74 |

### Audio and MIDI effect capabilities

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| FX-01 | EQ, filtering and spectrum matching | not_verified | partial_or_open | T72, T74 |
| FX-02 | Compression, limiting, gating, de-essing and dynamics | not_verified | partial_or_open | T72, T74 |
| FX-03 | Reverb, delay and convolution | not_verified | partial_or_open | T72, T74 |
| FX-04 | Distortion, saturation, amps and pedals | not_verified | partial_or_open | T72, T74 |
| FX-05 | Modulation, imaging and rhythmic multi-effects | not_verified | partial_or_open | T72, T74 |
| FX-06 | Pitch correction, shifter and vocal transformation | not_verified | partial_or_open | T72, T74 |
| FX-07 | MIDI effects and sandboxed scripting | not_verified | partial_or_open | T98 |
| FX-08 | Meters, tuner, gain, test signal and utility processors | not_verified | pass_linux | — |

### Third-party devices and extensibility

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| HOST-01 | Audio Units v2/v3 instruments and effects | not_verified | partial_or_open | T23, T75, T76 |
| HOST-02 | Plugin scan, validation and crash recovery | not_verified | partial_or_open | T23, T75, T76 |
| HOST-03 | ARA/editor integration and instrument multi-output | not_verified | partial_or_open | T75, T76, T98 |
| HOST-04 | WASM devices and controlled live reload | not_verified | partial_or_open | T98 |
| HOST-05 | VST3 and CLAP hosting — VOID extension, not Logic feature | not_verified | partial_or_open | T23, T75, T76 |

### Logic intelligent tools and VOID AI

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| INTEL-01 | Session Players: drummer, bass, keyboard and synth styles | not_verified | pass_linux | — |
| INTEL-02 | Chord ID and harmonic analysis | not_verified | pass_linux | — |
| INTEL-03 | Mastering Assistant | not_verified | pass_linux | — |
| INTEL-04 | Predictive notes, drums, chords and phrase completion | not_verified | pass_linux | — |
| INTEL-05 | Arrangement and orchestration copilot | not_verified | pass_linux | — |
| INTEL-06 | Text/reference-to-audio: loops, one-shots and stems | not_verified | pass_linux | — |
| INTEL-07 | Audio continuation, inpainting and variations | not_verified | pass_linux | — |
| INTEL-08 | Local memory, preferences and reusable recipes | not_verified | partial_or_open | T49, T50, T51 |
| INTEL-09 | Model routing, downloads, cancellation and resource budgets | not_verified | partial_or_open | T49, T50, T51 |

### Gesture-first composition — VOID additions

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| GEST-01 | Draw a melodic contour and tap a rhythm | not_verified | pass_linux | — |
| GEST-02 | Touch/trackpad macros and harmonic gestures | not_verified | pass_linux | — |
| GEST-03 | MIDI/MPE expressive performance | not_verified | pass_linux | — |
| GEST-04 | Camera-hand conducting, optional and local | not_verified | partial_or_open | T85 |
| GEST-05 | Gesture recording, quantized launches and undo | not_verified | pass_linux | — |
| GEST-06 | Keyboard/accessibility equivalents for every gesture | not_verified | pass_linux | — |

### Visual timeline and stage — VOID additions

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| VIS-01 | Imported images, video and conventional visual presets | not_verified | pass_linux | — |
| VIS-02 | Generated artwork, visual loops and video | not_verified | partial_or_open | T85 |
| VIS-03 | Audio-reactive visuals and beat-synced changes | not_verified | partial_or_open | T85 |
| VIS-04 | Scene graphs, shader presets and safe generated shaders | not_verified | partial_or_open | T85 |
| VIS-05 | Preview, full-screen and multi-display outputs | not_verified | pass_linux | — |
| VIS-06 | Projection mapping and show cues | not_verified | pass_linux | — |
| VIS-07 | OSC/DMX integration and external show control | not_verified | pass_linux | — |
| VIS-08 | Audio-plus-video export and reproducible rendering | not_verified | pass_linux | — |

### Delivery and interoperability

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| OUT-01 | Stereo/mono audio bounce and export | not_verified | pass_linux | — |
| OUT-02 | Offline/realtime bounce, dither and normalization | not_verified | pass_linux | — |
| OUT-03 | Batch stems, regions and instrument renders | not_verified | pass_linux | — |
| OUT-04 | AAF, Final Cut XML and MusicXML interchange | not_verified | pass_linux | — |
| OUT-05 | Logic/GarageBand project migration | not_verified | pass_linux | — |
| OUT-06 | Surround routing and binaural monitoring | not_verified | partial_or_open | T92, T93 |
| OUT-07 | Dolby Atmos beds, objects and ADM BWF | not_verified | partial_or_open | T92, T93 |
| OUT-08 | Atmos MP4 QC, head tracking and spatial formats | not_verified | partial_or_open | T92, T93 |

### Specialist workflows and product completeness

| ID | Feature | Ledger status | Derived | Open test ids |
|---|---|---|---|---|
| PRO-01 | Notation editor, parts, lyrics and tablature | not_verified | pass_linux | — |
| PRO-02 | Movie scoring and absolute timecode | not_verified | pass_linux | — |
| PRO-03 | MTC, MIDI Clock output, MMC and Ableton Link | not_verified | pass_linux | — |
| PRO-04 | Logic Remote-style companion and control surfaces | not_verified | pass_linux | — |
| PRO-05 | Custom shortcuts, screensets and flexible workspaces | not_verified | pass_linux | — |
| PRO-06 | Accessibility, help, onboarding and localization | not_verified | partial_or_open | T45, T47, T48 |
| PRO-07 | Environment-style MIDI/modular routing | not_verified | pass_linux | — |
| PRO-08 | Installer, updates, diagnostics and production QA | not_verified | not_run | T25, T26, T27, T28, T29, T30, T99, T100 |

## Stock-tool outcome groups (22)

| ID | Family | Inventory | Ledger | Derived (via owner-task tests) | Open test ids |
|---|---|---|---|---|---|
| STOCK-01 | Core synthesis | Alchemy; Retro Synth; ES1; ES2; EFM1; ES E; ES M; ES P | not_verified | partial_or_open | T72, T74, T98 |
| STOCK-02 | Sampling | Quick Sampler; Sampler; Sample Alchemy | not_verified | partial_or_open | T72, T74, T98 |
| STOCK-03 | Drums | Drum Kit Designer; Drum Machine Designer; Drum Synth; Ultrabeat | not_verified | partial_or_open | T72, T74 |
| STOCK-04 | Acoustic and orchestral | Studio Bass; Studio Piano; Studio Strings; Studio Horns | not_verified | partial_or_open | T72, T74 |
| STOCK-05 | Vintage and modeled | Vintage B3 Organ; Vintage Clav; Vintage Electric Piano; Vintage Mellotron; Sculp | not_verified | partial_or_open | T72, T74, T98 |
| STOCK-06 | Vocoder and infrastructure | EVOC 20 PolySynth; External Instrument; Klopfgeist | not_verified | partial_or_open | T72, T74 |
| STOCK-07 | Legacy sound coverage | Bass; Church Organ; Drum Kits; Electric Clav(inet); Electric Piano; Guitar; Horn | not_verified | partial_or_open | T72, T74, T98 |
| STOCK-08 | Amps and pedal rack | Amp Designer; Bass Amp Designer; Pedalboard | not_verified | partial_or_open | T72, T74 |
| STOCK-09 | Delays | Delay Designer; Echo; Sample Delay; Stereo Delay; Tape Delay | not_verified | partial_or_open | T72, T74 |
| STOCK-10 | Saturation and distortion | Bitcrusher; ChromaGlow; Clip Distortion; Distortion; Distortion II; Overdrive; P | not_verified | partial_or_open | T72, T74 |
| STOCK-11 | Dynamics | Adaptive Limiter; Compressor; DeEsser 2; Enveloper; Expander; Limiter; Multipres | not_verified | partial_or_open | T72, T74, T92, T93 |
| STOCK-12 | Equalization | Channel EQ; Linear Phase EQ; Match EQ; Single Band EQ; Vintage Console EQ; Vinta | not_verified | partial_or_open | T72, T74 |
| STOCK-13 | Filters and vocoding | AutoFilter; EVOC 20 Filterbank; EVOC 20 TrackOscillator; Fuzz-Wah; Spectral Gate | not_verified | partial_or_open | T72, T74, T98 |
| STOCK-14 | Modulation | Chorus; Ensemble; Flanger; Microphaser; Modulation Delay; Phaser; RingShifter; R | not_verified | partial_or_open | T72, T74 |
| STOCK-15 | Rhythmic multi-effects | Beat Breaker; Phat FX; Remix FX; Step FX | not_verified | partial_or_open | T72, T74 |
| STOCK-16 | Pitch and voice | Pitch Correction; Pitch Shifter; Vocal Transformer | not_verified | partial_or_open | T72, T74 |
| STOCK-17 | Reverbs | ChromaVerb; Space Designer; Quantec Room Simulator; EnVerb; SilverVerb | not_verified | partial_or_open | T72, T74 |
| STOCK-18 | Imaging and spatial | Binaural Post-Processing; Spatial Audio Monitoring; Direction Mixer; Stereo Spre | not_verified | partial_or_open | T92, T93 |
| STOCK-19 | Meters and utilities | BPM Counter; Correlation Meter; Level Meter; Loudness Meter; MultiMeter; Surroun | not_verified | partial_or_open | T72, T74, T92, T93 |
| STOCK-20 | MIDI processors | Arpeggiator; Chord Trigger; Modifier; Modulator; Note Repeater; Randomizer; Scri | not_verified | partial_or_open | T98 |
| STOCK-21 | Mastering | Mastering Assistant | not_verified | pass_linux | — |
| STOCK-22 | Legacy effects | AVerb; Bass Amp; DeEsser; Denoiser; Ducker; DJ EQ; Fat EQ; Single-Band EQ; Silve | not_verified | partial_or_open | T72, T74 |
## Device / platform matrix

Per-target evidence, honest to what ran where. "Verified" = the named suite ran
on that OS with recorded exit-0 output; "engine-qualified" = macOS lane evidence
against real Tracktion; "model-side" = Linux-verified model/policy only;
"NEEDS" = recorded gap with an owning entry.

| Capability surface | Linux x86_64 (this workspace) | macOS Apple Silicon | Windows x64 |
|---|---|---|---|
| App shell + coordinator + protocol | verified — workspace 146/146, void-worker lifecycle + wire roundtrip | qualified via engine lane commits | not_tested |
| Engine musical model + render | model-side (mock worker + fixtures) | engine-qualified — T14 1,536,000-frame WAV + T13/T15/T16 partials | not_tested |
| Recording takes/journal | spec + fixture model | engine-qualified — T31–T35 in-process fixtures | not_tested |
| Plugin scan/crash proof | descriptor model only | engine-qualified AU — T22/T24; VST3/CLAP descriptors NEEDS (engine #34) | not_tested |
| Runtime plugin isolation | NEEDS — engine #33 (policy spec only) | NEEDS — same entry | not_tested |
| Checkpoints/recovery | verified — tests/recovery 22/22 failpoint kills | inherited (crate is platform-neutral) | not_tested |
| AI job runtime + admission | verified — tests/journeys/ai 18/18; models suite provisioning-gated (F5-N10) | inherited | not_tested |
| Audio gen/stem/transcribe workers | verified when provisioned — T60–T62 pass_linux; musicgen CC-BY-NC gated | inherited | not_tested |
| Visual engine (wgpu) | model-side — void-visual determinism + drop-counted | needs GPU run (visfx NEEDS W23-01) | not_tested |
| Camera conducting | model-side — landmark-only policy by construction | NEEDS tracker backend (visfx W23-02) | not_tested |
| AV export (ffmpeg) | verified — tests/av 29/29 argv-only + rights flags | inherited | not_tested |
| Notation model + MusicXML | verified — tests/notation 18/18; Verovio engraving NEEDS | inherited | not_tested |
| Spatial delivery | model-side — layout/ADM gates; licensed validators NEEDS (show #1/#2) | needs licensed validator | not_tested |
| Sync/show (MTC/MMC/OSC/DMX) | model-side — byte-exact codecs; wire transports NEEDS (show #5/#7/#8) | needs MIDI/DMX/OSC endpoints | not_tested |
| WASM extensions | verified — tests/wasm 31/31 real wasmtime host | inherited | not_tested |
| Packaging/signing/updater | unsigned dev bundles only (b3de97e) | unsigned | not_tested — NEEDS F5-N03 |

## Summary counts

- Tests (100): 55 pass_linux · 9 pass_macos · 9 partial_linux · 3 partial_macos · 3 blocked · 21 not_run
- Features (116, derived): 72 pass_linux · 4 pass_macos_engine · 2 mixed_pass · 37 partial_or_open · 1 not_run (PRO-08)
- Stock groups (22, derived): 1 pass_linux · 21 partial_or_open (stock-DSP conformance T72 is the dominant blocker)
- Any row marked `partial_*`, `blocked` or `not_run` maps to a NEEDS entry — see `NEEDS_MAP.json` and `RESIDUAL_RISKS.md`.
