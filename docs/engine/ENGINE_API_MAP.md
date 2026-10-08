# ENGINE_API_MAP — VOID ops → Tracktion Engine symbols

Worker: `native/void-engine` (JUCE message-loop process, no `[NSApp run]` — see EVIDENCE).
All ops execute on the **JUCE message thread** inside `EngineSession::dispatch`
(`src/session/EngineSession.cpp`), never on the audio callback. Audio-thread work is
limited to `DeviceBridge::audioDeviceIOCallbackWithContext` (`src/audio/DeviceBridge.cpp`).

VOID IDs → engine objects: `session/` maps via `voidId` metadata stored on each
engine object (`t.state[voidId]` / clip/note set at insert time) + `tracks_`/`clips_`/`notes_`
index maps rebuilt by `rebuildIndexes()`. No ID-prefix inference anywhere.

| Protocol op (`vp::PersistentOp_*`) | Handler | Engine call(s) | File |
|---|---|---|---|
| CreateProjectOp | `opCreateProject` | `te::createEmptyEdit`, `te::Edit::Options{role=forEditing,numUndoLevelsToStore=500}`, `editFileRetriever`→`container/checkpoints/live/engine.tracktionedit`, `tempoSequence.getTempoAt(BP(0)).setBpm` | `Ops.cpp` |
| OpenProjectOp | `opOpenProject` | `te::loadEditFromFile`/`te::Edit` on checkpoint `engine.tracktionedit`, restores `voidRevision` | `Ops.cpp` |
| CloseProjectOp | `opCloseProject` | `edit_.reset()` (TE destructor unwinds graph) | `Ops.cpp` |
| AddTrackOp | `opAddTrack` | `te::Edit::insertNewAudioTrack` (AUDIO/MIDI), `insertNewBusTrack`/folder for BUS; `voidId` on `track->state`; INSTRUMENT = audio track + `getPluginCache().createNewPlugin(FourOscPlugin::xmlTypeName)` + `pluginList.insertPlugin` | `Ops.cpp` |
| RemoveTrackOp | `opRemoveTrack` | `te::Edit::deleteTrack` | `Ops.cpp` |
| SetTrackNameOp | `opSetTrackName` | `track->setName` | `Ops.cpp` |
| SetTrackGainOp | `opSetTrackGain` | `te::VolumeAndPanPlugin` on `track->pluginList` (gainLinear → `setVolumeDb` conversion) | `Ops.cpp` |
| SetTrackPanOp | `opSetTrackPan` | `te::VolumeAndPanPlugin::setPan` | `Ops.cpp` |
| SetTrackMuteOp / SetTrackSoloOp | `opSetTrackMute`/`opSetTrackSolo` | `te::Track::setMute(bool)` / `setSolo(bool)` — audio-thread safe mute flags | `Ops.cpp` |
| InsertAudioClipOp | `opInsertAudioClip` | `track->insertWaveClip(name, file, {start,len}, true)` via `assetFile(rel)` container path | `Ops.cpp` |
| InsertMidiClipOp | `opInsertMidiClip` | `te::MidiTrack`/`AudioTrack::insertMIDIClip(name, {start,len}, um)`; `voidId` on clip state | `Ops.cpp` |
| RemoveClipOp / MoveClipOp / TrimClipOp / SplitClipOp | `opRemoveClip`…`opSplitClip` | `clip->removeFromParent`, `clip->setStart`/`setLength`, `te::Clip::splitAt` | `Ops.cpp` |
| InsertNoteOp | `opInsertNote` | `te::MidiClip::getSequence().addNote(pitch, startBeat, lengthBeat, velocity, colour, um)` | `Ops.cpp` |
| RemoveNoteOp / SetNoteOp | `opRemoveNote`/`opSetNote` | `te::MidiNote` mutation + `removeNote` on sequence | `Ops.cpp` |
| SetTempoOp | `opSetTempo` | `edit_->tempoSequence.getTempoAt(...).setBpm` / insertTempo | `Ops.cpp` |
| SetTimeSignatureOp | `opSetTimeSignature` | `tempoSequence.insertTimeSig(BP(0))` → `numerator`/`denominator` | `Ops.cpp` |
| SetLoopRangeOp | `opSetLoopRange` | `te::Edit::loopPoint1/loopPoint2` + `TransportControl::setLoopRange` | `Ops.cpp` |
| SaveProjectOp / CreateCheckpointOp | `opSaveProject`/`opCreateCheckpoint` → `writeCheckpoint` | staging dir → `edit_->state.toXmlString()` → `checkpoints/<uuid>/` + `manifest.json` (sha256 inventory) + `CURRENT` pointer | `persistence/SaveBridge.cpp` |
| UndoOp / RedoOp | `opUndo`/`opRedo` | `edit_->getUndoManager().undo()/redo()` + `rebuildIndexes()` | `Ops.cpp` |
| AttachAssetOp | `opAttachAsset` | registry only: `assets_[assetId]=relPath` under `container/assets/sha256/` (path validated, no `..`) | `Ops.cpp` |
| InsertPluginOp / RemovePluginOp / SetPluginParamOp | `opInsertPlugin`/`opRemovePlugin`/`opSetPluginParam` | `te::PluginCache::createNewPlugin` by type name, `pluginList.insertPlugin`, `AutomatableParameter` by UID | `Ops.cpp` + `plugins/PluginHost.cpp` |
| OpenPluginEditorOp / ClosePluginEditorOp | `openPluginEditor`/`closePluginEditor` | `te::Plugin::showWindowExplicitly` + `WindowState` NSWindow (native editor proof) | `plugins/PluginHost.cpp` |

## Transport (`ControlRequest → TransportRequest`)

| TransportOp | Call |
|---|---|
| PLAY | `edit_->getTransport().play(false)` |
| STOP | `stop(false, false)` |
| SEEK | `setCurrentPosition` + `TransportAck` event |
| PANIC | `stop` + `playHeadContextCancelled`/engine `getDeviceManager` flush |
| SET_CYCLE | `transport.setLoopRange` + `isLooping=true` |

## Telemetry (`telemetry_` UDS, lossy by contract, ≤30 Hz)

- `ClockSnapshot`: `edit_->getTransport().getCurrentPosition()` → ticks (`960000/quarter`), loop state. Built on `EngineWorker::timerCallback` (message thread) — never blocks.
- `MeterFrame`: `DeviceBridge` atomic peak/RMS per track, filled by the audio callback (lock-free atomics) and drained on the timer.
- `SaveResultEvent`: emitted by `writeCheckpoint` completion.
- `TransportAck`: emitted for SEEK/SET_CYCLE.

## Dispatch & receipt contract

`EngineSession::dispatch` enforces: launch-token already verified at hello;
`expected_revision == revision_` else STALE_REVISION(6); `engine_epoch==1` else
STALE_EPOCH(7); `command_id` dedup via `receipts_` map keyed by command_id storing
SHA-256 of the raw wire frame — same id+same hash → DUPLICATE(1), same id+different
hash → COMMAND_ID_REUSE(8). APPLIED bumps `revision_` and stores receipt.

## Recording (W08, engine-internal until protocol rev 2 — see NEEDS.md §7-8)

| Capability | Engine call | Tracktion/JUCE symbol (exact) | Site |
|---|---|---|---|
| Arm input on track | `RecordingManager::arm` | `te::InputDeviceInstance::setTarget(EditItemID, moveToTrack, undo, index)` → `tl::expected<Destination*>`; `Destination::recordEnabled=true` (via `te::assignTrackAsInput` for `track:` sources, `EditUtilities.cpp:1407`) | `recording/RecordingManager.cpp` |
| Input resolution | `resolveInput` | `WaveInputDevice`/`MidiInputDevice` from `DeviceManager` (`wave:N`/`midi:N` = Nth non-track device; `track:<voidId>`; `""` = first) | `recording/RecordingManager.cpp` |
| Monitor mode | `setMonitor` | `te::InputDevice::setMonitorMode(MonitorMode::{off,automatic,on})` | `recording/RecordingManager.cpp` |
| Count-in | `setCountIn` | `te::Edit::setCountInMode(Edit::CountIn)` (`tracktion_Edit.h:712`) | same |
| Metronome | `setMetronome` | `edit->clickTrackEnabled/clickTrackGain/clickTrackRecordingOnly` (CachedValues) | same |
| Punch | `punchIn`/`punchOut` | `te::InputDeviceInstance::setRecordingEnabled(EditItemID,bool)` live | same |
| Start/stop take | `startRecording`/`stopRecording` | `te::TransportControl::record(false)` / `stop(discard,false)`; `ensureContextAllocated()` required before `getCurrentPlaybackContext()` | same |
| Take file routing | `VoidEngineBehaviour::takeFileProvider` | `te::EngineBehaviour::getFileForNewAudioRecording(Track&, ext)` override (`tracktion_EngineBehaviour.h:149`) → `recordings/<takeId>/chunk-<id>-1.wav` | `recording/RecordingManager.h` |
| Stereo config | `arm` | `te::WaveInputDevice::setChannelConfiguration(ChannelConfiguration::stereo(0)/mono(0))` | `recording/RecordingManager.cpp` |
| Take lifecycle events | `recordingStarted/Stopped/Finished` | `te::TransportControl::Listener` (`tracktion_TransportControl.h`) | same |
| MIDI capture | `MidiCapture::handleIncomingMidiMessage` | `te::InputDeviceInstance::Consumer` realtime callback → lock-free `juce::AbstractFifo` → 20 Hz `juce::Timer` drain writes `midi-<id>.jsonl` (sustain/bend/polyAT/chanAT byte-exact) | `recording/MidiCapture.cpp` |
| All-notes-off | `emitAllNotesOff`, `panic` | `juce::MidiMessage::allNotesOff(ch)` ×16 → `MidiInputDevice::handleIncomingMessage` + `MidiOutputDevice::sendNoteOffMessages` | same |
| Journal | `TakeJournalFile` | atomic tmp+rename `journal.json`; `scanChunks` RIFF-walk (JUNK/bext-aware, unfinalized data-size → file-extent estimate); `recoverIncomplete` marks orphan takes `incomplete` | `recording/TakeJournal.cpp` |
| Recovery on open | `opOpenProject` | `TakeJournalFile::recoverIncomplete(container)` runs on every open incl. edit-file-missing containers | `session/Ops.cpp` |
| Device-loss error | `deviceListChanged` | `juce::ChangeListener` on `DeviceManager`; take fails explicitly when a live input disappears | `recording/RecordingManager.cpp` |
| Disk-space gate | `startRecording` | `juce::File::getBytesFreeOnVolume()` ≥ 64 MiB pre-check | same |
| Hosted latency | fixture `latency` | `te::test_utilities::EnginePlayer` `inputLatencyNumSamples`/`outputLatencyNumSamples` + `WaveInputDevice::getRecordAdjustment()` → `te::toSamples` | `tests/recording_fixture.cpp` |
