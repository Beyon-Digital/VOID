# VOID

> **Dark. Infinite. Spacious.**
> A local-first, AI-powered creative production environment for music, sound design, live visuals, and full concert/rave/festival media compositions.

-----

## Implementation status (honest snapshot)

> Updated during the `devin/void-implementation` work. The canonical tracker is
> `docs/void-handoff/tracking/` (`TASKS.json`, `TESTS.json`,
> `FEATURE_TRACEABILITY.json`) with generated views in `WORK_PACKAGES.md` /
> `TEST_MATRIX.md` / `FEATURE_MAP.md`.

**The rest of this README describes the target product, not current capability.**

What exists in the repository today:

- An Electron + React scaffold: window lifecycle, a mostly-empty `WebAudioAdapter`
  (stub methods — it does **not** play, record, mix or render), domain/port
  interfaces in `void-core`, a `SessionView` clip-matrix component, and `Knob`/`Fader`
  style primitives.
- A tracked implementation plan (`docs/void-handoff/`) to migrate to
  Tauri/Rust + a native Tracktion Engine/JUCE worker per `docs/void-handoff/HANDOFF.md`.

What does **not** exist yet (despite sections below describing them):

- No audio/MIDI engine, playback, recording, mixing, plugin hosting or rendering.
- No working Session/Ableton-parity feature set, no stock devices, no WASM/M4L
  plugin runtime, no AI copilot, no visual engine, no show/cue system, no OSC/DMX.
- No Tauri shell or native C++ worker wired into the app yet (in progress on this branch).

Treat every "feature" section below as a design goal until its requirement IDs
show evidence in `docs/void-handoff/tracking/`.

-----

## What Is VOID?

VOID is an Electron-based desktop application that replaces Logic Pro X for power users who want:

- A **professional multi-track DAW** with absolute "Ableton Live" parity (Session View/Clip Matrix, advanced Audio Warping, Groove Pool, and stock devices)
- A **modular node-graph synthesizer** and **Max for Live equivalent** (Eurorack-style, in software, powered by WASM)
- An **AI copilot** that learns your musical identity and co-creates with you
- A **live visual engine** with WebGL shaders, projection mapping, and a live coding environment
- A **show cue-list** for designing full festival/concert sets from start to finish
- **MIDI/OSC/DMX** connectivity to any hardware in your studio or on stage
- **Stem separation, music generation, and AI sound design** — all running locally
- **Generative plugins and dynamic pipelines** — chain AI models, WASM modules, and Python sidecar tasks for endless generative music

Everything runs on your machine. No subscriptions. No cloud lock-in. Cross-platform: macOS, Windows, Linux.

-----

## Architecture Philosophy

VOID is built on **Hexagonal (Ports & Adapters) Architecture**. This means:

- The **Core Domain** (`packages/void-core`) contains all business logic and defines abstract **Port interfaces** — it has zero knowledge of how those interfaces are implemented
- Every external dependency (Web Audio API, MIDI devices, OSC, Python AI sidecar, Three.js, SQLite) is an **Adapter** that implements a Port
- Modules are **plug-and-play**: you can swap the audio adapter, the AI model adapter, or the visual renderer without touching any business logic
- The **Event Bus** is the nervous system — all inter-module communication flows through typed events

This architecture makes VOID infinitely extensible. New hardware, new AI models, new visual backends — all snap in via new adapters implementing existing ports.

-----

## Repository Structure

```
void/
├── apps/
│   └── void-desktop/               # Electron shell — the app container
│       ├── main/                   # Node.js main process
│       │   ├── index.ts            # App lifecycle, window management, sidecar spawn
│       │   ├── ipc/                # IPC handlers (bridge between main and renderer)
│       │   ├── sidecar/            # Python sidecar process manager
│       │   │   ├── SidecarManager.ts   # Spawn, health-check, restart, kill
│       │   │   ├── PythonChecker.ts    # Validates Python env, pip, GPU availability
│       │   │   └── SidecarError.ts     # Typed error hierarchy for sidecar failures
│       │   └── updater/            # Auto-updater (electron-updater)
│       ├── preload/
│       │   └── index.ts            # contextBridge — exposes void-api to renderer
│       └── renderer/
│           └── index.tsx           # React 18 root, router, global providers
│
├── packages/
│   │
│   ├── void-core/                  # ★ THE HEART — domain models + port interfaces
│   │   ├── ports/                  # Abstract interfaces (TypeScript interfaces only)
│   │   │   ├── AudioPort.ts        # Audio engine: tracks, nodes, transport
│   │   │   ├── MIDIPort.ts         # MIDI: note on/off, CC, clock, mapping
│   │   │   ├── OSCPort.ts          # OSC: send/receive, universe exposure
│   │   │   ├── AIPort.ts           # AI: generate, separate, chat, shader, arrange
│   │   │   ├── VisualPort.ts       # Visuals: shaders, surfaces, projection, livecode
│   │   │   ├── PluginPort.ts       # Plugin: register, instantiate, hot-reload
│   │   │   ├── ProjectPort.ts      # Project: save, load, export, history
│   │   │   └── ShowPort.ts         # Show: cues, triggers, stage-view control
│   │   ├── domain/                 # Pure domain entities
│   │   │   ├── Project.ts          # Root aggregate: tracks, scenes, settings
│   │   │   ├── Track.ts            # Audio/MIDI/Instrument/Bus track
│   │   │   ├── Clip.ts             # Audio clip, MIDI clip, automation clip
│   │   │   ├── Node.ts             # Synth node (for modular graph)
│   │   │   ├── Connection.ts       # Cable between synth nodes
│   │   │   ├── CueList.ts          # Show cue list aggregate
│   │   │   ├── Cue.ts              # Individual cue: type, trigger, duration
│   │   │   ├── SessionMemory.ts    # AI session memory model
│   │   │   └── PluginManifest.ts   # WASM plugin descriptor
│   │   └── events/
│   │       ├── VoidEventBus.ts     # EventEmitter3-based typed event bus
│   │       └── events.ts           # All event type definitions (discriminated unions)
│   │
│   ├── void-daw/                   # Multi-track DAW: timeline, mixer, automation
│   │   ├── adapters/
│   │   │   └── WebAudioAdapter.ts  # Implements AudioPort via Web Audio API + Tone.js
│   │   ├── engine/
│   │   │   ├── Sequencer.ts        # Timeline playback, loop, punch-in/out
│   │   │   ├── Transport.ts        # BPM, time signature, play/stop/record
│   │   │   ├── Automation.ts       # Automation lane engine
│   │   │   ├── AudioWarp.ts        # Advanced audio warping algorithms
│   │   │   ├── GroovePool.ts       # Groove extraction and quantization
│   │   │   └── AudioWorklets/      # Custom DSP processors (pitch, time-stretch, etc.)
│   │   └── components/
│   │       ├── SessionView/        # Ableton-style Clip Matrix for non-linear performance
│   │       ├── Timeline/           # Scrollable multi-track Arrangement View
│   │       ├── TrackLane/          # Individual track row
│   │       ├── ClipBlock/          # Draggable audio/MIDI clip
│   │       ├── Mixer/              # Vertical channel strip layout
│   │       ├── ChannelStrip/       # Fader, pan, sends, inserts, EQ
│   │       └── AutomationLane/     # Automation curve editor
│   │
│   ├── void-synth/                 # Modular synthesizer — node-graph signal routing
│   │   ├── nodes/                  # Individual module definitions
│   │   │   ├── OscillatorNode.ts
│   │   │   ├── FilterNode.ts
│   │   │   ├── EnvelopeNode.ts
│   │   │   ├── LFONode.ts
│   │   │   ├── DelayNode.ts
│   │   │   ├── ReverbNode.ts
│   │   │   ├── CompressorNode.ts
│   │   │   ├── SamplerNode.ts
│   │   │   ├── SequencerNode.ts    # Step sequencer module
│   │   │   └── MIDIInNode.ts       # MIDI input into graph
│   │   ├── graph/
│   │   │   ├── SynthCanvas.tsx     # React Flow canvas with custom nodes
│   │   │   ├── PatchCable.tsx      # Animated SVG cables between nodes
│   │   │   └── NodeLibrary.tsx     # Drag-from panel of available modules
│   │   └── engine/
│   │       └── GraphBuilder.ts     # Builds Web Audio graph from node definitions
│   │
│   ├── void-visual/                # Visual engine: WebGL, shaders, projection mapping
│   │   ├── adapters/
│   │   │   └── ThreeJSAdapter.ts   # Implements VisualPort via Three.js / R3F
│   │   ├── renderer/
│   │   │   ├── VoidCanvas.tsx      # R3F canvas, audio-reactive scene
│   │   │   ├── SceneGraph.ts       # Scene management, layer system
│   │   │   └── AudioReactor.ts     # Binds analyser data to visual parameters
│   │   ├── shaders/
│   │   │   ├── ShaderEditor.tsx    # Monaco-based GLSL editor with live preview
│   │   │   ├── ShaderLibrary.ts    # Built-in shader presets
│   │   │   └── hot-reload.ts       # GLSL hot-reload without scene restart
│   │   ├── projection/
│   │   │   ├── ProjectionMapper.tsx # Multi-surface warp editor (drag corners)
│   │   │   ├── Surface.ts          # Projection surface model
│   │   │   └── OutputManager.ts    # Multi-window output (Electron BrowserWindow)
│   │   └── livecode/
│   │       ├── LiveCodeEditor.tsx  # Hydra-style live coding panel
│   │       └── LiveCodeRuntime.ts  # Sandboxed GLSL/JS execution environment
│   │
│   ├── void-ai/                    # AI subsystem — all intelligence lives here
│   │   ├── adapters/
│   │   │   ├── OllamaAdapter.ts    # Local LLM via Ollama REST API
│   │   │   ├── OpenAIAdapter.ts    # Cloud fallback: OpenAI
│   │   │   ├── AnthropicAdapter.ts # Cloud fallback: Anthropic
│   │   │   └── SidecarAdapter.ts   # Audio models via Python FastAPI sidecar
│   │   ├── router/
│   │   │   └── AIRouter.ts         # Routes requests: local first → cloud fallback
│   │   ├── copilot/
│   │   │   ├── CopilotPanel.tsx    # Chat UI (floating panel)
│   │   │   ├── ContextBuilder.ts   # Builds rich context from current project state
│   │   │   └── ActionParser.ts     # Parses LLM responses into DAW actions
│   │   ├── memory/
│   │   │   ├── SessionMemory.ts    # Stores style, BPM, key, genre preferences
│   │   │   ├── MemoryStore.ts      # SQLite-backed memory with vector search
│   │   │   └── IdentityProfile.ts  # Your musical identity (built over sessions)
│   │   ├── generation/
│   │   │   ├── MusicGenerator.ts   # ACE-Step bridge: text → audio
│   │   │   └── GenerationPanel.tsx # UI: prompt, duration, style, generate button
│   │   ├── separation/
│   │   │   ├── StemSeparator.ts    # Demucs bridge: audio → stems
│   │   │   └── StemPanel.tsx       # UI: drag-in audio → get stems
│   │   ├── arrangement/
│   │   │   ├── ArrangementEngine.ts  # NL → arrangement skeleton
│   │   │   └── ArrangementPanel.tsx  # UI: describe your track → scaffold
│   │   └── shader-copilot/
│   │       ├── ShaderCopilot.ts    # NL description → GLSL shader code
│   │       └── ShaderPromptPanel.tsx # UI: type visual description → get shader
│   │
│   ├── void-midi/                  # MIDI + OSC hub
│   │   ├── midi/
│   │   │   ├── MIDIAdapter.ts      # Implements MIDIPort via WEBMIDI.js v3
│   │   │   ├── DeviceManager.tsx   # UI: list, connect, name devices
│   │   │   ├── CCMapper.ts         # Map any CC to any parameter address
│   │   │   └── MIDIMonitor.tsx     # Real-time MIDI event inspector
│   │   └── osc/
│   │       ├── OSCAdapter.ts       # Implements OSCPort via node-osc
│   │       ├── OSCUniverse.ts      # Exposes ALL VOID params as OSC addresses
│   │       ├── OSCRouter.tsx       # UI: view/filter/test OSC routing
│   │       └── DMXBridge.ts        # OSC → DMX (via sACN/ArtNet bridge)
│   │
│   ├── void-plugins/               # WASM plugin system + marketplace + dynamic pipelines
│   │   ├── sandbox/
│   │   │   ├── WASMSandbox.ts      # Secure WASM execution environment
│   │   │   └── PluginRuntime.ts    # Lifecycle: init, process, destroy
│   │   ├── pipelines/
│   │   │   ├── GenerativePipeline.ts # Chaining AI sidecar + WASM modules for generative audio/MIDI
│   │   │   └── PipelineNode.ts     # Individual node in a generative pipeline
│   │   ├── registry/
│   │   │   ├── PluginRegistry.ts   # Local manifest store (JSON/SQLite)
│   │   │   └── PluginLoader.ts     # Load, validate, hot-reload plugins
│   │   └── marketplace/
│   │       ├── MarketplacePanel.tsx # Browse/install local plugin packages
│   │       └── PluginCard.tsx      # Plugin display: name, version, controls
│   │
│   ├── void-show/                  # Concert/show cue-list + stage view
│   │   ├── cuelist/
│   │   │   ├── CueListEditor.tsx   # Visual cue list editor (drag, reorder, edit)
│   │   │   ├── CueItem.tsx         # Single cue: name, type, trigger, duration
│   │   │   └── CueEngine.ts        # Executes cues: fires events at correct times
│   │   ├── stage-view/
│   │   │   ├── StageWindow.ts      # Electron second BrowserWindow (to projector)
│   │   │   ├── StageView.tsx       # Full-screen performance UI
│   │   │   └── PanicButton.ts      # Immediate blackout + audio stop
│   │   └── triggers/
│   │       ├── BeatTrigger.ts      # Fire cue on beat N
│   │       ├── TimeTrigger.ts      # Fire cue at elapsed time
│   │       └── ManualTrigger.ts    # Fire cue on keyboard/MIDI/OSC input
│   │
│   └── void-ui/                    # Shared design system (dark/industrial)
│       ├── components/
│       │   ├── Knob.tsx            # Rotary control with MIDI learn
│       │   ├── Fader.tsx           # Linear fader
│       │   ├── VUMeter.tsx         # Level meter (peak + RMS)
│       │   ├── Waveform.tsx        # Waveform display (canvas)
│       │   ├── PianoRoll.tsx       # MIDI piano roll editor
│       │   └── Oscilloscope.tsx    # Real-time audio oscilloscope
│       └── theme/
│           ├── tokens.css          # CSS custom properties (colors, spacing, type)
│           └── global.css          # Base styles, scrollbars, selection
│
├── sidecar/                        # Python FastAPI AI sidecar process
│   ├── main.py                     # FastAPI app entry point
│   ├── requirements.txt            # Pin all versions
│   ├── routers/
│   │   ├── generate.py             # POST /generate — ACE-Step music generation
│   │   ├── separate.py             # POST /separate — Demucs stem separation
│   │   ├── copilot.py              # POST /copilot — LLM routing (Ollama/OpenAI/Anthropic)
│   │   ├── shader.py               # POST /shader — GLSL generation via LLM
│   │   └── arrangement.py          # POST /arrangement — arrangement skeleton
│   ├── models/
│   │   ├── loader.py               # Model loader: GPU/CPU detection, lazy loading
│   │   ├── acestep.py              # ACE-Step wrapper
│   │   └── demucs.py               # Demucs wrapper
│   ├── health/
│   │   ├── checker.py              # Health check logic: models, GPU, disk space
│   │   └── errors.py               # Typed error responses
│   └── utils/
│       ├── audio.py                # Audio I/O utilities (librosa, soundfile)
│       └── gpu.py                  # CUDA/MPS/CPU detection
│
├── docs/
│   ├── README.md                   # ← YOU ARE HERE
│   ├── ARCHITECTURE.md             # Deep-dive hexagonal arch + data flow diagrams
│   ├── PORTS.md                    # Every port interface with full TSDoc
│   ├── EVENTS.md                   # Complete event bus event catalogue
│   ├── PLUGINS.md                  # WASM plugin authoring guide
│   ├── OSC_UNIVERSE.md             # Full OSC address space reference
│   └── SIDECAR.md                  # Python sidecar: setup, models, API reference
│
├── scripts/
│   ├── setup.ts                    # First-run setup: Python check, pip install
│   ├── check-python.ts             # Python environment validator
│   └── build-sidecar.ts            # Package sidecar for distribution
│
├── package.json                    # Root workspace
├── pnpm-workspace.yaml
├── turbo.json
└── electron-builder.config.ts      # Cross-platform packaging config
```

-----

## Port Interfaces (The Contracts)

> All adapters MUST implement these interfaces. The core domain ONLY calls these interfaces — never concrete implementations.

### AudioPort

```typescript
// packages/void-core/ports/AudioPort.ts
export interface AudioPort {
  // Transport
  play(): void
  stop(): void
  pause(): void
  record(): void
  setTransportBPM(bpm: number): void
  setTimeSignature(num: number, denom: number): void
  setLoopRegion(startBeat: number, endBeat: number): void
  getCurrentBeat(): number
  getCurrentTime(): number

  // Tracks
  createTrack(type: TrackType, id: string): AudioTrackRef
  deleteTrack(id: string): void
  setTrackVolume(id: string, volume: number): void       // 0.0 - 1.0
  setTrackPan(id: string, pan: number): void             // -1.0 to 1.0
  setTrackMute(id: string, muted: boolean): void
  setTrackSolo(id: string, soloed: boolean): void
  routeTrackTo(srcId: string, dstId: string): void       // Sends / bus routing

  // Clips
  placeClip(trackId: string, clip: ClipData, startBeat: number): ClipRef
  removeClip(clipId: string): void
  trimClip(clipId: string, startOffset: number, endOffset: number): void

  // Nodes (for synth graph integration)
  connectNode(src: AudioNodeRef, dst: AudioNodeRef, srcOutput?: number, dstInput?: number): void
  disconnectNode(src: AudioNodeRef, dst: AudioNodeRef): void
  createAudioNode(type: string, params: Record<string, number>): AudioNodeRef

  // Analysis
  getAnalyserData(trackId: string, fftSize: number): Float32Array
  getPeakLevel(trackId: string): { peak: number; rms: number }
}
```

### AIPort

```typescript
// packages/void-core/ports/AIPort.ts
export interface AIPort {
  // Music generation (routes to ACE-Step via sidecar)
  generateMusic(prompt: MusicGenerationPrompt): Promise<GeneratedAudioResult>

  // Stem separation (routes to Demucs via sidecar)
  separateStems(audioPath: string, stems: StemType[]): Promise<StemSeparationResult>

  // LLM copilot (routes to Ollama / OpenAI / Anthropic)
  chat(messages: ChatMessage[], context: ProjectContext, memory: SessionMemory): Promise<CopilotResponse>

  // GLSL generation
  generateShader(description: string, audioFeatures?: AudioFeatureHints): Promise<GLSLShaderResult>

  // Arrangement engine
  buildArrangement(spec: ArrangementSpec): Promise<ArrangementSkeleton>

  // Session memory
  saveMemory(key: string, value: MemoryValue): Promise<void>
  loadMemory(query: string): Promise<MemoryValue[]>
  buildIdentityProfile(): Promise<MusicalIdentityProfile>

  // Health / status
  getAvailableModels(): Promise<ModelAvailability>
  getActiveProvider(): AIProvider
}
```

### VisualPort

```typescript
// packages/void-core/ports/VisualPort.ts
export interface VisualPort {
  // Shader management
  loadShader(glsl: string, id: string): ShaderRef
  updateShader(id: string, glsl: string): void           // Hot-reload, no flicker
  removeShader(id: string): void

  // Surfaces (projection mapping)
  createSurface(config: SurfaceConfig): ProjectionSurfaceRef
  warpSurface(id: string, corners: Quad): void           // 4-corner warp
  assignShaderToSurface(shaderId: string, surfaceId: string): void

  // Audio reactivity
  bindAnalyser(analyser: AudioAnalyserRef): void
  setAudioBinding(shaderId: string, paramName: string, audioFeature: AudioFeature): void

  // Output
  setOutputWindow(windowId: number, display: DisplayInfo): void
  enterFullscreen(outputIndex: number): void
  exitFullscreen(): void
  previewInPanel(enabled: boolean): void

  // Live coding
  executeLiveCode(code: string): LiveCodeResult
  getLiveCodeError(): string | null
}
```

### MIDIPort

```typescript
// packages/void-core/ports/MIDIPort.ts
export interface MIDIPort {
  // Device management
  getInputDevices(): MIDIDeviceInfo[]
  getOutputDevices(): MIDIDeviceInfo[]
  connectInput(deviceId: string): void
  connectOutput(deviceId: string): void

  // Event listeners
  onNoteOn(handler: (event: NoteOnEvent) => void): Unsubscribe
  onNoteOff(handler: (event: NoteOffEvent) => void): Unsubscribe
  onCC(handler: (event: CCEvent) => void): Unsubscribe
  onPitchBend(handler: (event: PitchBendEvent) => void): Unsubscribe
  onClock(handler: (event: ClockEvent) => void): Unsubscribe

  // Send MIDI
  sendNoteOn(deviceId: string, channel: number, note: number, velocity: number): void
  sendNoteOff(deviceId: string, channel: number, note: number): void
  sendCC(deviceId: string, channel: number, cc: number, value: number): void

  // MIDI Learn
  startLearn(parameterAddress: string): void
  stopLearn(): void
  getMappings(): CCMapping[]
  clearMapping(parameterAddress: string): void
}
```

### OSCPort

```typescript
// packages/void-core/ports/OSCPort.ts
export interface OSCPort {
  // Send
  send(address: string, args: OSCArg[]): void
  sendToHost(host: string, port: number, address: string, args: OSCArg[]): void

  // Receive
  on(address: string, handler: (args: OSCArg[]) => void): Unsubscribe
  onPattern(pattern: string, handler: (address: string, args: OSCArg[]) => void): Unsubscribe

  // Universe: expose all VOID parameters as OSC addresses
  exposeParameter(address: string, opts: OSCParamOptions): void
  unexposeParameter(address: string): void
  getExposedAddresses(): OSCAddressEntry[]

  // Server control
  startServer(port: number): Promise<void>
  stopServer(): Promise<void>
  getServerStatus(): OSCServerStatus
}
```

-----

## Domain Events (Event Bus)

All inter-module communication happens via the `VoidEventBus`. No module imports another module’s internals directly.

```typescript
// packages/void-core/events/events.ts
export type VoidEvent =
  // Transport
  | { type: 'transport:play'; beat: number }
  | { type: 'transport:stop' }
  | { type: 'transport:bpmChange'; bpm: number }

  // Audio
  | { type: 'audio:clipPlaced'; trackId: string; clipId: string; startBeat: number }
  | { type: 'audio:trackCreated'; trackId: string; trackType: TrackType }
  | { type: 'audio:levelUpdate'; trackId: string; peak: number; rms: number }

  // MIDI
  | { type: 'midi:noteOn'; channel: number; note: number; velocity: number; deviceId: string }
  | { type: 'midi:cc'; channel: number; cc: number; value: number; deviceId: string }
  | { type: 'midi:deviceConnected'; device: MIDIDeviceInfo }

  // Visual
  | { type: 'visual:shaderLoaded'; shaderId: string }
  | { type: 'visual:shaderError'; shaderId: string; error: string }
  | { type: 'visual:fullscreenEntered'; outputIndex: number }

  // AI
  | { type: 'ai:generationStarted'; jobId: string }
  | { type: 'ai:generationComplete'; jobId: string; audioPath: string }
  | { type: 'ai:separationComplete'; jobId: string; stems: StemMap }
  | { type: 'ai:copilotResponse'; response: CopilotResponse }
  | { type: 'ai:sidecarReady' }
  | { type: 'ai:sidecarError'; error: SidecarError; recoverable: boolean }

  // Show
  | { type: 'show:cueTriggered'; cueId: string; triggerType: TriggerType }
  | { type: 'show:sceneStarted'; sceneId: string }
  | { type: 'show:panic' }

  // Plugin
  | { type: 'plugin:registered'; manifest: PluginManifest }
  | { type: 'plugin:hotReloaded'; pluginId: string }
  | { type: 'plugin:error'; pluginId: string; error: string }

  // Project
  | { type: 'project:loaded'; projectId: string }
  | { type: 'project:saved'; projectId: string }
  | { type: 'project:dirty' }
```

-----

## Python Sidecar

The sidecar is a **FastAPI process** spawned by Electron main on app start. It handles all heavy AI workloads that require Python/PyTorch (audio generation, stem separation, LLM routing to local models).

### Lifecycle

```
Electron starts
  → PythonChecker.validate()
      → checks python3/python, pip, virtualenv
      → checks GPU availability (CUDA/MPS/CPU)
      → checks disk space (models need ~10-40GB)
      → if checks fail → SidecarSetupWizard UI shown
  → SidecarManager.spawn()
      → starts sidecar/main.py on port 7842
      → polls GET /health every 2s
      → on ready → emits ai:sidecarReady event
  → if crash → SidecarManager.restart() with exponential backoff
  → on app quit → SidecarManager.kill()
```

### API Endpoints

|Method|Path          |Input                                        |Output                                   |Model                  |
|------|--------------|---------------------------------------------|-----------------------------------------|-----------------------|
|`POST`|`/generate`   |`{ prompt, lyrics?, duration, style, seed? }`|`{ jobId, audioPath }`                   |ACE-Step 1.5           |
|`POST`|`/separate`   |`{ audioPath, stems[] }`                     |`{ jobId, stemPaths }`                   |Demucs htdemucs        |
|`POST`|`/copilot`    |`{ messages, memory, provider, apiKey? }`    |`{ response, tokens }`                   |Ollama/OpenAI/Anthropic|
|`POST`|`/shader`     |`{ description, audioFeatures? }`            |`{ glsl }`                               |LLM (any)              |
|`POST`|`/arrangement`|`{ spec: ArrangementSpec }`                  |`{ skeleton }`                           |LLM (any)              |
|`GET` |`/health`     |—                                            |`{ status, models, gpu, disk }`          |—                      |
|`GET` |`/models`     |—                                            |`{ available[], loaded[], downloading? }`|—                      |
|`GET` |`/jobs/{id}`  |—                                            |`{ status, progress, result? }`          |—                      |

### Error Handling

All sidecar errors are typed and surfaced in the UI:

```
SidecarError
├── PythonNotFoundError       → Show setup wizard
├── DependencyMissingError    → Show pip install prompt
├── GPUUnavailableError       → Warn, fall back to CPU
├── ModelNotDownloadedError   → Show model download prompt
├── InsufficientDiskError     → Show disk space warning
├── ModelLoadError            → Show error, offer retry
├── InferenceError            → Show error, job failed
└── SidecarCrashError         → Auto-restart, show status
```

-----

## Plugin System (WASM)

VOID plugins are **WebAssembly modules** with a JSON manifest. They can implement any combination of: audio processing, visual generation, MIDI transformation, or UI panels.

### Plugin Manifest

```json
{
  "id": "my-plugin@1.0.0",
  "name": "My Plugin",
  "version": "1.0.0",
  "author": "Artist Name",
  "type": ["audio-fx", "visual"],
  "wasm": "./my-plugin.wasm",
  "ui": "./my-plugin-ui.js",
  "ports": {
    "audio-in": 1,
    "audio-out": 1,
    "params": [
      { "id": "cutoff", "name": "Cutoff", "min": 20, "max": 20000, "default": 1000 }
    ]
  },
  "osc-prefix": "/void/plugin/my-plugin"
}
```

### Plugin API (available to WASM module)

```typescript
// Host functions injected into WASM memory
interface VoidPluginHostAPI {
  void_log(msgPtr: i32, len: i32): void
  void_emit_event(typePtr: i32, dataPtr: i32): void
  void_get_param(idPtr: i32): f32
  void_set_param(idPtr: i32, value: f32): void
  void_get_sample_rate(): f32
  void_get_bpm(): f32
  void_get_beat(): f32
}

// Required exports from WASM module
interface VoidPluginExports {
  init(): void
  process(inputPtr: i32, outputPtr: i32, numSamples: i32): void
  destroy(): void
}
```

-----

## OSC Universe

Every controllable parameter in VOID is exposed as an OSC address. This makes VOID instantly controllable from TouchOSC, Lemur, lighting desks, custom hardware, or any OSC-capable tool.

Address format: `/void/<module>/<target>/<parameter>`

Examples:

```
/void/daw/transport/bpm          f32   Set BPM
/void/daw/transport/play         i32   1=play, 0=stop
/void/daw/track/{id}/volume      f32   0.0-1.0
/void/daw/track/{id}/pan         f32   -1.0 to 1.0
/void/synth/node/{id}/{param}    f32   Any node parameter
/void/visual/shader/{id}/{param} f32   Any shader uniform
/void/show/cue/{id}/fire         i32   Trigger cue
/void/show/panic                 i32   Immediate blackout
/void/ai/generate                s     Trigger generation with prompt string
```

Full reference: `docs/OSC_UNIVERSE.md`

-----

## Show & Cue System

Designed for real festival use. A **cue** is any discrete action:

|Cue Type     |Examples                                 |
|-------------|-----------------------------------------|
|`AudioScene` |Load + arm a set of tracks for a section |
|`VisualScene`|Crossfade to a new shader/surface layout |
|`MIDISend`   |Send MIDI to external hardware at a cue  |
|`OSCSend`    |Trigger DMX, Resolume, lighting desk     |
|`AIGenerate` |Trigger AI music generation, queue result|
|`Tempo`      |Set new BPM at this cue                  |
|`Blackout`   |Hard-cut visuals to black                |
|`Custom`     |Execute JS snippet                       |

**Stage View** is a full-screen Electron `BrowserWindow` sent to a second display/projector, containing:

- Live visual output (Three.js renderer mirrored from main window)
- No UI chrome — pure output
- Controllable entirely from main window or MIDI/OSC

**Panic Button** is always accessible via:

- UI button in stage view and main window
- MIDI CC 119 on any channel (default)
- OSC `/void/show/panic`
- Keyboard: `Shift+Escape`

-----

## AI Copilot

The copilot understands the full state of your project and can:

- **Generate music**: “Make a 32-bar acid bassline at 138 BPM in F minor”
- **Build arrangements**: “Give me a 6-minute techno track: intro 2min → drop 1min → breakdown 90s → final drop 90s → outro”
- **Write shaders**: “Strobing acid fractal that pulses on kick hits and evolves with the bassline”
- **Mix advice**: “My kick sounds muddy in the low mids, what should I do?”
- **Sound design**: “What synthesis approach would give me that classic Roland Jupiter-8 pad?”
- **Remember context**: “Use the same style as the track I made last Tuesday”

### AI Routing

```
User request
  → AIRouter checks:
      1. Is Ollama running locally? → use Ollama
      2. Is BYOK key present? → use specified provider
      3. Default fallback → prompt user to set up local or BYOK
  → For audio tasks (generate/separate):
      → Always routes to Python sidecar (local models only)
      → No cloud fallback for audio (privacy + latency)
```

### Session Memory

After each session, the memory system extracts and stores:

- Preferred BPM range
- Key signatures used
- Genre/style descriptors
- Mixing decisions
- Preferred synth architectures
- Feedback on AI suggestions (thumbs up/down)

Stored in SQLite with vector embeddings for semantic recall. The copilot reads relevant memories to personalize every response.

-----

## Tech Stack Reference

|Layer       |Library                      |Version      |Why                                               |
|------------|-----------------------------|-------------|--------------------------------------------------|
|Shell       |Electron                     |`^32.0`      |Cross-platform, Chromium = native WebMIDI/WebAudio|
|Frontend    |React                        |`^18.3`      |Concurrent mode, Suspense for async AI states     |
|Build       |Vite + electron-vite         |`^5.0`       |Fast HMR, Electron-aware config                   |
|Monorepo    |pnpm workspaces + Turborepo  |`^9 / ^2`    |Fast installs, build caching                      |
|State       |Zustand + Immer              |`^4 / ^10`   |Minimal, fast, Electron IPC compatible            |
|Audio Engine|Tone.js + Web Audio API      |`^15`        |Transport, scheduling, synths                     |
|Node Graph  |React Flow (xyflow)          |`^11`        |Battle-tested, extensible, handles 100+ nodes     |
|Visuals     |Three.js + @react-three/fiber|`^0.169 / ^8`|WebGL, audio-reactive, projection                 |
|Code Editor |Monaco Editor                |`^0.52`      |VS Code engine, GLSL syntax, live errors          |
|MIDI        |WEBMIDI.js                   |`^3.1`       |Node.js + browser, full TypeScript                |
|OSC         |node-osc                     |`^5.0`       |UDP OSC, Node.js main process                     |
|AI Sidecar  |FastAPI + uvicorn            |`^0.115`     |Async Python, clean REST                          |
|Music Gen   |ACE-Step                     |`1.5`        |State-of-art local music generation               |
|Stem Sep    |Demucs                       |`^4.0`       |htdemucs model, best quality                      |
|Storage     |better-sqlite3               |`^11`        |Embedded, zero-config, fast                       |
|Packaging   |electron-builder             |`^25`        |cross-platform: .dmg / .exe / .AppImage           |

-----

## Build Phases (Implementation Order for LLM)

Follow this order strictly. Each phase is a complete, testable increment.

### Phase 1 — Foundation

**Goal:** Runnable Electron app with IPC bridge and event bus

Deliverables:

- `void-desktop` Electron shell (main + preload + renderer)
- `void-core` package: all port interfaces, domain models, event bus
- IPC bridge: typed channels between main process and renderer
- Basic React app shell with dark theme
- `void-ui` package: tokens, global styles, Knob, Fader stubs

Done when: App opens, main process communicates with renderer via typed IPC.

### Phase 2 — Audio Engine & Ableton Parity Foundation

**Goal:** Play audio, Session View (Clip Matrix), Arrangement View (timeline), basic mixer, and Groove Pool.

Deliverables:

- `void-daw` package: `WebAudioAdapter.ts` (implements `AudioPort`)
- Tone.js transport: play, stop, BPM, loop
- **Session View**: Ableton-style clip matrix for triggering loops non-linearly
- **Arrangement View**: Multi-track timeline UI for linear composition
- Clip placement and playback (with **advanced Audio Warping algorithms**)
- **Groove Pool**: extract and apply timing/velocity grooves across clips
- Mixer: volume, pan, mute, solo
- Level meters (VU)
- **Stock Devices**: Implement core EQ, Compressor, Reverb, and Delay modules.

Done when: Load a drum loop into a Session View clip, warp it to tempo, apply a groove from the Groove Pool, trigger it, and adjust the stock EQ.

### Phase 3 — MIDI + OSC Hub

**Goal:** Connect hardware, expose parameters

Deliverables:

- `void-midi` package: `MIDIAdapter` (WEBMIDI.js) + `OSCAdapter` (node-osc)
- Device manager UI
- CC mapper: click any knob → move hardware CC → mapped
- OSC server listening on configurable port
- First 20 OSC addresses exposed (transport, track volumes)
- MIDI monitor panel

Done when: MIDI keyboard triggers notes, TouchOSC controls BPM.

### Phase 4 — Modular Synth Graph

**Goal:** Build and play a complete synth patch from nodes

Deliverables:

- `void-synth` package: React Flow canvas
- 10 built-in nodes: Osc, Filter, Envelope, LFO, Delay, Reverb, Compressor, Sampler, Sequencer, MIDIIn
- Patch cable system (animated SVG)
- Node Library panel (drag to canvas)
- `GraphBuilder`: converts node graph → Web Audio graph
- Integration with DAW tracks (synth rack per instrument track)

Done when: Patch OSC → Filter → Reverb → Output and hear it triggered by MIDI.

### Phase 5 — Python Sidecar

**Goal:** Sidecar spawns, health check passes, models available

Deliverables:

- `sidecar/` FastAPI application
- `PythonChecker.ts`: validates Python env, shows setup wizard if needed
- `SidecarManager.ts`: spawn, monitor, restart, kill
- Full error hierarchy with UI surface
- `GET /health` endpoint working
- Demucs integration: drag audio → get stems (first AI feature)
- `void-ai/separation`: `StemSeparator.ts` + UI panel

Done when: Drag in a WAV, click Separate, get 4 stems back.

### Phase 6 — AI Copilot + Music Generation

**Goal:** Natural language controls VOID, music generates locally

Deliverables:

- `void-ai/copilot`: chat UI + `ContextBuilder` + `ActionParser`
- `void-ai/router`: Ollama → BYOK fallback chain
- `void-ai/memory`: SQLite memory store, identity profile
- `void-ai/generation`: ACE-Step bridge + generation panel
- `void-ai/arrangement`: arrangement engine + panel
- Copilot can: create tracks, set BPM, place clips, describe mix intent

Done when: “Create a 130 BPM minimal techno track” → arrangement scaffold appears.

### Phase 7 — Visual Engine

**Goal:** Audio-reactive visuals, shader editor, projection mapping

Deliverables:

- `void-visual/renderer`: Three.js/R3F canvas with audio binding
- `void-visual/shaders`: Monaco GLSL editor + hot-reload + preset library
- `void-visual/projection`: multi-surface warp editor (drag corners)
- `void-visual/livecode`: Hydra-style live coding panel
- `void-show/stage-view`: Electron second window → second display
- Shader copilot: describe → GLSL generated

Done when: GLSL shader reacts to kick drum, output sends to projector.

### Phase 8 — Plugins, Max for Live Equivalent & Generative Pipelines

**Goal:** Drop in WASM plugins, hot-reload without restart, and build generative pipelines.

Deliverables:

- `void-plugins/sandbox`: WASM execution environment (**Max for Live equivalent** for building custom devices)
- `void-plugins/pipelines`: Dynamic node-based pipelines bridging WASM and Python sidecar for **generative music pipelines**
- `void-plugins/registry`: manifest store + loader
- `void-plugins/marketplace`: browse/install UI
- Hot-reload: file watcher detects plugin changes, reloads in <1s
- Example plugin: a generative MIDI sequencer feeding a WASM synth
- OSC auto-exposure for plugin parameters

Done when: Drop a .wasm + manifest.json in the plugins folder, it appears and works. Chain it with a Python-based AI generative node in a dynamic pipeline.

### Phase 9 — Show & Cue System

**Goal:** Design and run a full concert set from VOID

Deliverables:

- `void-show/cuelist`: visual cue list editor
- All cue types implemented
- Beat-locked, time-locked, manual triggers
- Stage view full-screen output
- Panic button (UI + MIDI CC 119 + OSC + keyboard)
- Show timeline: set design with scene markers
- Export show file (JSON)

Done when: Design a 10-cue set, run it live with projector output.

### Phase 10 — Polish & Distribution

**Goal:** Ship it

Deliverables:

- Full OSC universe (all params exposed)
- electron-builder config: `.dmg` / `.exe` / `.AppImage`
- Auto-updater
- First-run setup wizard (Python, model downloads)
- Performance profiling + AudioWorklet optimization
- E2E tests for critical paths (audio playback, MIDI mapping, AI generation)
- User documentation

Done when: `pnpm build` produces installable app on all 3 platforms.

-----

## Development Setup

```bash
# Prerequisites
node >= 20
pnpm >= 9
python >= 3.10
git

# Clone and install
git clone https://github.com/your-org/void
cd void
pnpm install

# Setup Python sidecar (first time)
pnpm run setup:sidecar

# Start development
pnpm dev                    # Starts Electron + all packages in watch mode

# Individual packages
pnpm --filter void-daw dev
pnpm --filter void-visual dev

# Run sidecar standalone (for debugging)
cd sidecar && uvicorn main:app --port 7842 --reload

# Build for distribution
pnpm build                  # All platforms
pnpm build:mac
pnpm build:win
pnpm build:linux
```

-----

## Environment Configuration

```
# .env (root — not committed)
VOID_SIDECAR_PORT=7842
VOID_OSC_PORT=9000
VOID_OLLAMA_HOST=http://localhost:11434

# BYOK — user sets these in Settings UI, stored in electron-store
OPENAI_API_KEY=
ANTHROPIC_API_KEY=
```

-----

## Key Design Decisions

**Why not VST3/CLAP?** Requires a C++ plugin host (JUCE). This explodes build complexity across 3 platforms. WASM plugins + OSC bridge to external hosts gives 90% of the value with 10% of the pain. Users who need VST3 can route VOID’s audio/MIDI to Ableton/Bitwig via Rewire/JACK/BlackHole.

**Why Electron over Tauri?** Tauri 2 is excellent but the Rust layer adds complexity for audio-specific work. Electron’s Chromium gives native WebMIDI and WebAudio APIs — no polyfills, no bridging. The extra memory overhead is acceptable for a pro audio workstation app.

**Why Python sidecar over ONNX in Node.js?** ACE-Step and modern Demucs models are actively developed in PyTorch. ONNX exports lag model improvements by months. The FastAPI sidecar is isolated, easily upgradable, and standard in the AI tooling world. The sidecar can be replaced entirely without touching the Electron app — it just needs to implement the same REST API.

**Why SQLite over a cloud database?** VOID is local-first, always. Session memory, project files, plugin registry — all SQLite. Zero setup, zero subscription, works offline forever.

**Why React Flow for the synth graph?** Modular synth UIs are notoriously hard to build. React Flow handles the canvas, zoom, pan, node selection, and edge routing. Custom node components snap in cleanly. It handles 200+ nodes at 60fps.

-----

## LLM Instructions

> Read this entire README before writing any code. This is your single source of truth.

1. **Never import across module boundaries directly.** Inter-module communication is ONLY via `VoidEventBus` events or through a Port interface. Example: `void-visual` must never import from `void-daw`. It subscribes to `audio:levelUpdate` events from the bus.
1. **Always implement the Port interface.** When building an adapter, start with the Port interface in `void-core/ports/` and implement every method. The adapter file header should read: `// Adapter: implements AudioPort`.
1. **Follow the phase order.** Do not start Phase 5 (sidecar) until Phase 4 (synth) is complete. Each phase must be testable before proceeding.
1. **Error handling is not optional.** Every sidecar call must handle: network error (sidecar not running), model not loaded, inference failure, and timeout. Errors surface in the UI — never silently swallowed.
1. **Type everything.** No `any`. The shared types live in `void-core/domain/` and `void-core/ports/`. Add types there before using them.
1. **The dark theme is non-negotiable.** Background `#0a0a0a`, accent `#7c3aed` (violet), text `#e5e7eb`. All UI components must use CSS custom properties from `void-ui/theme/tokens.css`.
1. **OSC address registration is automatic.** When you build a component with a controllable parameter, call `oscPort.exposeParameter(address, opts)` in the component’s `useEffect`. Don’t manually maintain the OSC universe — it self-documents.
1. **Audio runs in the renderer process.** Web Audio API is not available in the main process. The `WebAudioAdapter` lives in renderer. IPC is only used to bridge hardware events (MIDI, OSC) from main to renderer.
1. **The sidecar port is 7842.** Always. Configurable via `VOID_SIDECAR_PORT` env var but default is 7842.
1. **When in doubt, emit an event.** If a module needs to tell another module something happened, it emits a typed event on `VoidEventBus`. It does not call a function on the other module.
