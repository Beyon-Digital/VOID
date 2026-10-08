// Persistent-op implementations for EngineSession.
// Every op runs on the message thread (single serialized mutation lane).

#include "EngineSession.h"
#include "../persistence/SaveBridge.h"
#include "../recording/TakeJournal.h"

namespace voidengine
{

namespace
{
inline CommandResult ok()     { return { vp::AckStatus_APPLIED, vp::ErrorCode_NONE, 0, {} }; }
inline CommandResult notFound (const char* what) { return { vp::AckStatus_REJECTED, vp::ErrorCode_NOT_FOUND, 0, juce::String (what) + " not found" }; }
inline CommandResult badReq (const char* m) { return { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, 0, m }; }
inline bool finite (float v) { return std::isfinite (v); }
} // namespace

// ---------------------------------------------------------------------------
// Project lifecycle
// ---------------------------------------------------------------------------
CommandResult EngineSession::opCreateProject (const vp::CreateProjectOp& op)
{
    if (edit_ != nullptr)
        return badReq ("a project is already open");

    const auto dir = juce::String (op.container_dir() ? op.container_dir()->c_str() : "");
    if (dir.isEmpty() || ! juce::File (dir).createDirectory().wasOk())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, 0, "cannot create container dir" };

    containerDirStr_ = dir;
    projectName_ = juce::String (op.name() ? op.name()->c_str() : "Untitled");
    projectId_   = juce::Uuid().toString();
    revision_    = 0;
    receipts_.clear();

    auto state = te::createEmptyEdit (*engine_);
    te::Edit::Options options { *engine_, state, {} };
    options.editFileRetriever = [this] { return containerDir().getChildFile ("checkpoints")
                                                                .getChildFile ("live")
                                                                .getChildFile ("engine.tracktionedit"); };
    options.role = te::Edit::forEditing;
    options.numUndoLevelsToStore = 500;
    edit_ = std::make_unique<te::Edit> (options);
    edit_->state.setProperty (te::IDs::name, projectName_, nullptr);
    recording_ = std::make_unique<RecordingManager> (*this);

    edit_->tempoSequence.getTempoAt (te::BeatPosition::fromBeats (0.0))
        .setBpm ((double) op.initial_bpm());

    // project.json at container root (CONTRACTS.md §4)
    auto* root = new juce::DynamicObject();
    root->setProperty ("format", "void-project/1");
    root->setProperty ("projectId", projectId_);
    root->setProperty ("name", projectName_);
    root->setProperty ("sampleRate", (int) op.sample_rate());
    root->setProperty ("createdEpoch", (juce::int64) epoch_);
    containerDir().getChildFile ("project.json")
        .replaceWithText (juce::JSON::toString (juce::var (root), true));

    return ok();
}

CommandResult EngineSession::opOpenProject (const vp::OpenProjectOp& op)
{
    if (edit_ != nullptr)
    {
        edit_->getTransport().stop (true, false);
        recording_.reset();
        edit_.reset();
        tracks_.clear(); clips_.clear(); notes_.clear(); plugins_.clear();
        clipToTrack_.clear(); noteToClip_.clear(); meterClients_.clear();
        assets_.clear();
    }

    const auto dir = juce::String (op.container_dir() ? op.container_dir()->c_str() : "");
    const auto container = juce::File (dir);
    const auto projectJson = container.getChildFile ("project.json");
    if (! container.isDirectory() || ! projectJson.existsAsFile())
        return notFound ("project container");

    // Recording journal recovery is container-level: orphaned takes must be
    // labeled `incomplete` on reopen even if the edit file is absent/corrupt.
    voidengine::TakeJournalFile::recoverIncomplete (container);

    const auto meta = juce::JSON::parse (projectJson.loadFileAsString());
    projectId_   = meta["projectId"].toString();
    projectName_ = meta["name"].toString();
    containerDirStr_ = dir;

    // Resolve CURRENT → checkpoint dir; fall back to container-root edit file.
    juce::File editFile;
    const auto currentFile = container.getChildFile ("CURRENT");
    if (currentFile.existsAsFile())
    {
        const auto ptr = juce::JSON::parse (currentFile.loadFileAsString());
        const auto cpId = ptr["checkpointId"].toString();
        if (cpId.isNotEmpty())
            editFile = container.getChildFile ("checkpoints").getChildFile (cpId)
                                .getChildFile ("engine.tracktionedit");
    }
    if (! editFile.existsAsFile())
        editFile = container.getChildFile ("engine.tracktionedit");
    if (! editFile.existsAsFile())
        return notFound ("engine edit file in container");

    auto state = juce::ValueTree::fromXml (editFile.loadFileAsString());
    if (! state.isValid())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, 0, "edit file unreadable" };

    te::Edit::Options options { *engine_, state, {} };
    options.role = te::Edit::forEditing;
    options.editFileRetriever = [editFile] { return editFile; };
    edit_ = std::make_unique<te::Edit> (options);

    const auto storedRev = (juce::int64) edit_->state.getProperty ("voidRevision", 0);
    revision_ = (uint64_t) storedRev;
    edit_->state.removeProperty ("voidRevision", nullptr);

    recording_ = std::make_unique<RecordingManager> (*this);
    recording_->recoverIncompleteTakes();
    rebuildIndexes();
    return ok();
}

CommandResult EngineSession::opCloseProject (const vp::CloseProjectOp&)
{
    if (edit_ == nullptr)
        return notFound ("open project");
    edit_->getTransport().stop (true, false);
    recording_.reset();
    edit_.reset();
    tracks_.clear(); clips_.clear(); notes_.clear(); plugins_.clear();
    clipToTrack_.clear(); noteToClip_.clear(); meterClients_.clear(); assets_.clear();
    revision_ = 0;
    projectId_ = {};
    return ok();
}

// ---------------------------------------------------------------------------
// Tracks
// ---------------------------------------------------------------------------
CommandResult EngineSession::opAddTrack (const vp::AddTrackOp& op)
{
    if (edit_ == nullptr)
        return notFound ("open project");
    const auto trackId = juce::String (op.track_id() ? op.track_id()->c_str() : "");
    if (trackId.isEmpty())
        return badReq ("missing track_id");
    if (findTrack (trackId) != nullptr)
        return badReq ("track_id already exists");

    auto t = edit_->insertNewAudioTrack (te::TrackInsertPoint (nullptr, nullptr), nullptr, true);
    if (t == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_WORKER_FAILED, 0, "insertNewAudioTrack failed" };

    t->state.setProperty ("voidId", trackId, &edit_->getUndoManager());
    if (auto* nm = op.name())
        t->setName (nm->c_str());
    attachLevelMeter (*t);

    if (op.kind() == vp::TrackKind_INSTRUMENT || op.kind() == vp::TrackKind_MIDI)
    {
        auto synth = edit_->getPluginCache().createNewPlugin (te::FourOscPlugin::xmlTypeName, {});
        if (auto* p = synth.get())
        {
            p->state.setProperty ("voidInstanceId", trackId + ":synth", nullptr);
            t->pluginList.insertPlugin (*synth, 0, nullptr);
            plugins_[(trackId + ":synth").toStdString()] = p;
        }
        if (op.kind() == vp::TrackKind_MIDI)
            t->state.setProperty ("voidLaneKind", "MIDI", nullptr);
    }

    tracks_[trackId.toStdString()] = t.get();
    return ok();
}

CommandResult EngineSession::opRemoveTrack (const vp::RemoveTrackOp& op)
{
    if (edit_ == nullptr) return notFound ("open project");
    auto* t = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (t == nullptr) return notFound ("track");
    for (auto* c : t->getClips())
    {
        const auto cid = c->state.getProperty ("voidId").toString();
        if (cid.isNotEmpty()) { clips_.erase (cid.toStdString()); clipToTrack_.erase (cid.toStdString()); }
    }
    tracks_.erase (juce::String (op.track_id()->c_str()).toStdString());
    edit_->deleteTrack (t);
    return ok();
}

CommandResult EngineSession::opSetTrackName (const vp::SetTrackNameOp& op)
{
    auto* t = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (t == nullptr) return notFound ("track");
    t->setName (op.name() ? op.name()->c_str() : "");
    return ok();
}

CommandResult EngineSession::opSetTrackGain (const vp::SetTrackGainOp& op)
{
    auto* t = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (t == nullptr) return notFound ("track");
    const float g = op.gain_linear();
    if (! finite (g) || g < 0.0f || g > 8.0f)
        return badReq ("gain_linear out of range");
    auto* vpPlugin = t->getVolumePlugin();
    if (vpPlugin == nullptr) return { vp::AckStatus_REJECTED, vp::ErrorCode_PLUGIN_UNAVAILABLE, 0, "no volume plugin" };
    vpPlugin->volParam->setParameter (g, juce::sendNotificationSync);
    return ok();
}

CommandResult EngineSession::opSetTrackPan (const vp::SetTrackPanOp& op)
{
    auto* t = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (t == nullptr) return notFound ("track");
    const float p = op.pan();
    if (! finite (p) || p < -1.0f || p > 1.0f)
        return badReq ("pan out of range");
    auto* vpPlugin = t->getVolumePlugin();
    if (vpPlugin == nullptr) return { vp::AckStatus_REJECTED, vp::ErrorCode_PLUGIN_UNAVAILABLE, 0, "no volume plugin" };
    vpPlugin->panParam->setParameter (p, juce::sendNotificationSync);
    return ok();
}

CommandResult EngineSession::opSetTrackMute (const vp::SetTrackMuteOp& op)
{
    auto* t = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (t == nullptr) return notFound ("track");
    t->setMute (op.muted());
    return ok();
}

CommandResult EngineSession::opSetTrackSolo (const vp::SetTrackSoloOp& op)
{
    auto* t = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (t == nullptr) return notFound ("track");
    t->setSolo (op.soloed());
    return ok();
}

// ---------------------------------------------------------------------------
// Clips
// ---------------------------------------------------------------------------
CommandResult EngineSession::opInsertAudioClip (const vp::InsertAudioClipOp& op)
{
    if (edit_ == nullptr) return notFound ("open project");
    auto* t = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (t == nullptr) return notFound ("track");

    const auto assetId = juce::String (op.asset_id() ? op.asset_id()->c_str() : "");
    const auto relIt = assets_.find (assetId.toStdString());
    if (relIt == assets_.end())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_ASSET_MISSING, 0, "asset not attached" };

    const auto file = assetFile (relIt->second);
    if (! file.existsAsFile())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_ASSET_MISSING, 0, "asset blob missing" };

    const auto clipId = juce::String (op.clip_id() ? op.clip_id()->c_str() : "");
    if (clipId.isEmpty()) return badReq ("missing clip_id");

    auto clip = t->insertWaveClip (clipId, file,
                                   te::createClipPosition (edit_->tempoSequence,
                                                           timeRangeOf (op.start_ticks(), op.length_ticks()),
                                                           te::TimeDuration::fromSeconds (0.0)),
                                   false);
    if (clip == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_WORKER_FAILED, 0, "insertWaveClip failed" };

    auto* clipPtr = clip.get();
    clipPtr->state.setProperty ("voidId", clipId, &edit_->getUndoManager());
    clips_[clipId.toStdString()] = clipPtr;
    clipToTrack_[clipId.toStdString()] = juce::String (op.track_id()->c_str());
    return ok();
}

CommandResult EngineSession::opInsertMidiClip (const vp::InsertMidiClipOp& op)
{
    if (edit_ == nullptr) return notFound ("open project");
    auto* t = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (t == nullptr) return notFound ("track");

    const auto clipId = juce::String (op.clip_id() ? op.clip_id()->c_str() : "");
    if (clipId.isEmpty()) return badReq ("missing clip_id");

    auto clip = t->insertMIDIClip (clipId,
                                   timeRangeOf (op.start_ticks(), op.length_ticks()),
                                   nullptr);
    if (clip == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_WORKER_FAILED, 0, "insertMIDIClip failed" };

    auto* clipPtr = clip.get();
    clipPtr->state.setProperty ("voidId", clipId, &edit_->getUndoManager());
    clips_[clipId.toStdString()] = clipPtr;
    clipToTrack_[clipId.toStdString()] = juce::String (op.track_id()->c_str());
    return ok();
}

CommandResult EngineSession::opRemoveClip (const vp::RemoveClipOp& op)
{
    auto* c = findClip (op.clip_id() ? op.clip_id()->c_str() : "");
    if (c == nullptr) return notFound ("clip");
    c->removeFromParent();
    const auto cid = juce::String (op.clip_id()->c_str());
    clips_.erase (cid.toStdString());
    clipToTrack_.erase (cid.toStdString());
    return ok();
}

CommandResult EngineSession::opMoveClip (const vp::MoveClipOp& op)
{
    auto* c = findClip (op.clip_id() ? op.clip_id()->c_str() : "");
    if (c == nullptr) return notFound ("clip");
    auto* newTrack = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (auto* ct = dynamic_cast<te::ClipTrack*> (newTrack); ct != nullptr)
    {
        // Move across tracks via the engine's own move path.
        c->moveTo (*ct);
    }
    auto pos = c->getPosition();
    auto start = timeOf (op.start_ticks());
    pos.time = { start, start + pos.time.getLength() };
    c->setPosition (pos);
    clipToTrack_[juce::String (op.clip_id()->c_str()).toStdString()] =
        juce::String (op.track_id() ? op.track_id()->c_str() : "");
    return ok();
}

CommandResult EngineSession::opTrimClip (const vp::TrimClipOp& op)
{
    auto* c = findClip (op.clip_id() ? op.clip_id()->c_str() : "");
    if (c == nullptr) return notFound ("clip");
    c->setPosition (te::createClipPosition (edit_->tempoSequence,
                                          timeRangeOf (op.start_ticks(), op.length_ticks()),
                                          edit_->tempoSequence.toTime (beatsOf (op.offset_ticks()))
                                              - edit_->tempoSequence.toTime (te::BeatPosition::fromBeats (0.0))));
    return ok();
}

CommandResult EngineSession::opSplitClip (const vp::SplitClipOp& op)
{
    auto* c = findClip (op.clip_id() ? op.clip_id()->c_str() : "");
    if (c == nullptr) return notFound ("clip");
    auto* ownerTrack = dynamic_cast<te::ClipTrack*> (c->getTrack());
    if (ownerTrack == nullptr) return badReq ("clip has no clip track");
    auto* second = ownerTrack->splitClip (*c, timeOf (op.at_ticks()));
    if (second != nullptr)
    {
        const auto nid = juce::String (op.new_clip_id() ? op.new_clip_id()->c_str() : "");
        if (nid.isNotEmpty())
        {
            second->state.setProperty ("voidId", nid, &edit_->getUndoManager());
            clips_[nid.toStdString()] = second;
            clipToTrack_[nid.toStdString()] = clipToTrack_[juce::String (op.clip_id()->c_str()).toStdString()];
        }
    }
    return ok();
}

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------
CommandResult EngineSession::opInsertNote (const vp::InsertNoteOp& op)
{
    auto* c = findClip (op.clip_id() ? op.clip_id()->c_str() : "");
    auto* mc = dynamic_cast<te::MidiClip*> (c);
    if (mc == nullptr) return notFound ("midi clip");
    const auto nid = juce::String (op.note_id() ? op.note_id()->c_str() : "");
    if (nid.isEmpty()) return badReq ("missing note_id");
    if (op.velocity() < 1 || op.velocity() > 127 || op.pitch() > 127)
        return badReq ("pitch/velocity out of range");

    auto* note = mc->getSequence().addNote ((int) op.pitch(),
                                            beatsOf (op.start_ticks()),
                                            beatLenOf (op.length_ticks()),
                                            (int) op.velocity(), 0,
                                            &edit_->getUndoManager());
    if (note == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_WORKER_FAILED, 0, "addNote failed" };
    note->state.setProperty ("voidId", nid, &edit_->getUndoManager());
    notes_[nid.toStdString()] = note;
    noteToClip_[nid.toStdString()] = juce::String (op.clip_id()->c_str());
    return ok();
}

CommandResult EngineSession::opRemoveNote (const vp::RemoveNoteOp& op)
{
    const auto nid = juce::String (op.note_id() ? op.note_id()->c_str() : "");
    auto it = notes_.find (nid.toStdString());
    if (it == notes_.end() || it->second == nullptr) return notFound ("note");
    auto cIt = noteToClip_.find (nid.toStdString());
    auto* mc = cIt != noteToClip_.end()
               ? dynamic_cast<te::MidiClip*> (findClip (cIt->second)) : nullptr;
    if (mc != nullptr)
        mc->getSequence().removeNote (*it->second, &edit_->getUndoManager());
    notes_.erase (it);
    noteToClip_.erase (cIt);
    return ok();
}

CommandResult EngineSession::opSetNote (const vp::SetNoteOp& op)
{
    auto it = notes_.find (juce::String (op.note_id() ? op.note_id()->c_str() : "").toStdString());
    if (it == notes_.end() || it->second == nullptr) return notFound ("note");
    auto* n = it->second;
    auto& um = edit_->getUndoManager();
    if (op.pitch() >= 0)    n->setNoteNumber (op.pitch(), &um);
    if (op.velocity() >= 0) n->setVelocity (op.velocity(), &um);
    if (op.start_ticks() >= 0 || op.length_ticks() >= 0)
    {
        const auto s = op.start_ticks() >= 0 ? beatsOf (op.start_ticks()) : n->getStartBeat();
        const auto l = op.length_ticks() >= 0 ? beatLenOf (op.length_ticks())
                                              : n->getRangeBeats().getLength();
        n->setStartAndLength (s, l, &um);
    }
    return ok();
}

// ---------------------------------------------------------------------------
// Tempo / meter / loop
// ---------------------------------------------------------------------------
CommandResult EngineSession::opSetTempo (const vp::SetTempoOp& op)
{
    if (edit_ == nullptr) return notFound ("open project");
    if (! finite (op.bpm()) || op.bpm() <= 0.0f || op.bpm() > 999.0f)
        return badReq ("bpm out of range");
    edit_->tempoSequence.insertTempo (beatsOf (op.at_ticks()), (double) op.bpm(), 0.0f);
    return ok();
}

CommandResult EngineSession::opSetTimeSignature (const vp::SetTimeSignatureOp& op)
{
    if (edit_ == nullptr) return notFound ("open project");
    if (op.numerator() == 0 || op.denominator() == 0)
        return badReq ("invalid time signature");
    auto sig = edit_->tempoSequence.insertTimeSig (beatsOf (op.at_ticks()));
    if (sig == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_WORKER_FAILED, 0, "insertTimeSig failed" };
    sig->numerator = op.numerator();
    sig->denominator = op.denominator();
    return ok();
}

CommandResult EngineSession::opSetLoopRange (const vp::SetLoopRangeOp& op)
{
    if (edit_ == nullptr) return notFound ("open project");
    auto& tc = edit_->getTransport();
    if (op.enabled())
        tc.setLoopRange (timeRangeOf (op.start_ticks(), op.end_ticks() - op.start_ticks()));
    tc.looping = op.enabled();
    return ok();
}

// ---------------------------------------------------------------------------
// Save / checkpoint (minimal — full W05 machinery is the persistence lane)
// ---------------------------------------------------------------------------
CommandResult EngineSession::opSaveProject (const vp::SaveProjectOp& op)
{
    juce::String cpId;
    return writeCheckpoint (op.reason() ? op.reason()->c_str() : "save", cpId);
}

CommandResult EngineSession::opCreateCheckpoint (const vp::CreateCheckpointOp& op)
{
    juce::String cpId;
    auto r = writeCheckpoint (op.reason() ? op.reason()->c_str() : "checkpoint", cpId);
    if (r.status == vp::AckStatus_APPLIED && cpId.isNotEmpty())
        r.message = cpId;
    return r;
}

// ---------------------------------------------------------------------------
// History — delegates to the engine's own undo (CONTRACTS.md §5)
// ---------------------------------------------------------------------------
CommandResult EngineSession::opUndo (const vp::UndoOp&)
{
    if (edit_ == nullptr) return notFound ("open project");
    if (! edit_->getUndoManager().canUndo())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, 0, "nothing to undo" };
    edit_->getUndoManager().undo();
    rebuildIndexes(); // object set changed — rebuild ID map
    return ok();
}

CommandResult EngineSession::opRedo (const vp::RedoOp&)
{
    if (edit_ == nullptr) return notFound ("open project");
    if (! edit_->getUndoManager().canRedo())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, 0, "nothing to redo" };
    edit_->getUndoManager().redo();
    rebuildIndexes();
    return ok();
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------
CommandResult EngineSession::opAttachAsset (const vp::AttachAssetOp& op)
{
    const auto assetId = juce::String (op.asset_id() ? op.asset_id()->c_str() : "");
    const auto rel = juce::String (op.rel_path() ? op.rel_path()->c_str() : "");
    if (assetId.isEmpty() || rel.isEmpty() || rel.contains ("..") || rel.startsWith ("/"))
        return badReq ("bad asset ref");
    const auto f = assetFile (rel);
    if (! f.existsAsFile())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_ASSET_MISSING, 0, "asset blob not in container" };
    assets_[assetId.toStdString()] = rel;
    return ok();
}

// ---------------------------------------------------------------------------
// Plugins (see plugins/PluginHost.cpp)
// ---------------------------------------------------------------------------
CommandResult EngineSession::opInsertPlugin (const vp::InsertPluginOp& op)
{
    return insertExternalPlugin (op);
}
CommandResult EngineSession::opRemovePlugin (const vp::RemovePluginOp& op)
{
    return removePlugin (op);
}
CommandResult EngineSession::opSetPluginParam (const vp::SetPluginParamOp& op)
{
    return setPluginParam (op);
}
CommandResult EngineSession::opOpenPluginEditor (const vp::OpenPluginEditorOp& op)
{
    return openPluginEditor (op);
}
CommandResult EngineSession::opClosePluginEditor (const vp::ClosePluginEditorOp& op)
{
    return closePluginEditor (op);
}

} // namespace voidengine
