// EngineSession: authoritative Tracktion Edit + serialized command lane.
//
// Threading contract (CONTRACTS.md §3): every persistent mutation runs on
// the JUCE message thread via applyPersistentCommand; the audio callback is
// never touched. Preparation work (file existence checks, hash reads) may
// run there too at F0 — bounded and measured by the DeviceBridge probe.
//
// Set JUCE string_encoding=UTF8 throughout (project names, safe messages).

#include "EngineSession.h"
#include <flatbuffers/flatbuffers.h>
#include <fstream>
#include <sstream>

namespace voidengine
{

thread_local bool EngineSession::rtCallbackActive = false;

namespace
{
/// Minimal JSON string escaping for read-view summaries (" \\ + controls).
juce::String esc (const juce::String& s)
{
    juce::String out;
    for (auto c : s)
        switch (c)
        {
            case '"':  out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n"; break;
            case '\r': out += "\\r"; break;
            case '\t': out += "\\t"; break;
            default:
                if ((unsigned) c < 0x20)
                    out += juce::String::formatted ("\\u%04x", (int) c);
                else
                    out += c;
        }
    return out;
}
} // namespace

// ---------------------------------------------------------------------------
// Minimal PropertyStorage / UIBehaviour / EngineBehaviour implementations.
// PropertyStorage persists to <appSupport>/VOID/Engine/settings.xml.
// ---------------------------------------------------------------------------
namespace
{
class VoidPropertyStorage : public te::PropertyStorage
{
public:
    VoidPropertyStorage() : te::PropertyStorage ("VOID") {}
};

class VoidUIBehaviour : public te::UIBehaviour
{
public:
    // Headless-safe: warnings are logged, never dialogued.
    void showWarningAlert (const juce::String& title,
                           const juce::String& message) override
    {
        juce::Logger::writeToLog ("[warn] " + title + ": " + message);
    }
};

// Default EngineBehaviour: stock plugin/device/preset behaviour.
} // namespace

// ---------------------------------------------------------------------------

EngineSession::EngineSession (uint64_t engineEpoch)
    : epoch_ (engineEpoch)
{
    engine_ = std::make_unique<te::Engine> (std::make_unique<VoidPropertyStorage>(),
                                            std::make_unique<VoidUIBehaviour>(),
                                            std::make_unique<te::EngineBehaviour>());
    engine_->getTemporaryFileManager().getTempDirectory().createDirectory();
}

EngineSession::~EngineSession()
{
    if (edit_ != nullptr)
        edit_->getTransport().stop (true, false);
    edit_.reset();
    engine_.reset();
}

// -- conversions -------------------------------------------------------------
te::BeatPosition EngineSession::beatsOf (int64_t ticks) const
{
    return te::BeatPosition::fromBeats (static_cast<double> (ticks) / kTicksPerQuarter);
}

te::BeatDuration EngineSession::beatLenOf (int64_t ticks) const
{
    return te::BeatDuration::fromBeats (static_cast<double> (ticks) / kTicksPerQuarter);
}

te::TimePosition EngineSession::timeOf (int64_t ticks) const
{
    jassert (edit_ != nullptr);
    return te::toTime (beatsOf (ticks), edit_->tempoSequence);
}

te::TimeRange EngineSession::timeRangeOf (int64_t startTicks, int64_t lenTicks) const
{
    return { timeOf (startTicks), timeOf (startTicks + lenTicks) };
}

int64_t EngineSession::ticksOf (te::BeatPosition p) const
{
    return static_cast<int64_t> (std::llround (p.inBeats() * kTicksPerQuarter));
}

int64_t EngineSession::ticksLenOf (te::BeatDuration d) const
{
    return static_cast<int64_t> (std::llround (d.inBeats() * kTicksPerQuarter));
}

// -- lookups -----------------------------------------------------------------
te::AudioTrack* EngineSession::findTrack (const juce::String& trackId) const
{
    auto it = tracks_.find (trackId.toStdString());
    return it != tracks_.end() ? it->second : nullptr;
}

te::Clip* EngineSession::findClip (const juce::String& clipId) const
{
    auto it = clips_.find (clipId.toStdString());
    return it != clips_.end() ? it->second : nullptr;
}

te::Plugin* EngineSession::findPlugin (const juce::String& instanceId) const
{
    auto it = plugins_.find (instanceId.toStdString());
    return it != plugins_.end() ? it->second : nullptr;
}

juce::File EngineSession::containerDir() const
{
    return juce::File (containerDirStr_);
}

juce::File EngineSession::assetFile (const juce::String& relPath) const
{
    // Never trust traversal — validate relPath is a plain assets-relative name.
    if (relPath.contains ("..") || relPath.startsWith ("/"))
        return {};
    return containerDir().getChildFile ("assets").getChildFile ("sha256")
                         .getChildFile (relPath);
}

juce::String EngineSession::trackKindString (const te::Track& t)
{
    if (dynamic_cast<const te::AudioTrack*> (&t) != nullptr)
        return "AUDIO";
    if (dynamic_cast<const te::MarkerTrack*> (&t) != nullptr)
        return "MARKER";
    if (dynamic_cast<const te::TempoTrack*> (&t) != nullptr)
        return "TEMPO";
    if (dynamic_cast<const te::ChordTrack*> (&t) != nullptr)
        return "CHORD";
    return "OTHER";
}

// -- index maintenance -------------------------------------------------------
void EngineSession::rebuildIndexes()
{
    tracks_.clear();
    clips_.clear();
    notes_.clear();
    plugins_.clear();
    clipToTrack_.clear();
    noteToClip_.clear();
    meterClients_.clear();

    if (edit_ == nullptr)
        return;

    for (auto* t : te::getAllTracks (*edit_))
    {
        if (auto* at = dynamic_cast<te::AudioTrack*> (t))
        {
            const auto voidId = at->state.getProperty ("voidId").toString();
            if (voidId.isNotEmpty())
                tracks_[voidId.toStdString()] = at;

            for (auto* c : at->getClips())
                indexClipTree (*c, voidId);

            for (auto* p : at->pluginList)
            {
                const auto pid = p->state.getProperty ("voidInstanceId").toString();
                if (pid.isNotEmpty())
                    plugins_[pid.toStdString()] = p;
            }
        }
    }
}

void EngineSession::indexClipTree (te::Clip& c, const juce::String& trackId)
{
    const auto voidId = c.state.getProperty ("voidId").toString();
    if (voidId.isNotEmpty())
    {
        clips_[voidId.toStdString()] = &c;
        clipToTrack_[voidId.toStdString()] = trackId;
        if (auto* mc = dynamic_cast<te::MidiClip*> (&c))
            for (auto* n : mc->getSequence().getNotes())
            {
                const auto nid = n->state.getProperty ("voidId").toString();
                if (nid.isNotEmpty())
                {
                    notes_[nid.toStdString()] = n;
                    noteToClip_[nid.toStdString()] = voidId;
                }
            }
    }
}

bool EngineSession::attachLevelMeter (te::AudioTrack& t)
{
    if (t.getLevelMeterPlugin() != nullptr)
        return true;
    auto meter = edit_->getPluginCache().createNewPlugin (te::LevelMeterPlugin::xmlTypeName, {});
    if (meter == nullptr)
        return false;
    t.pluginList.insertPlugin (*meter, -1, nullptr);
    auto* lm = t.getLevelMeterPlugin();
    if (lm == nullptr)
        return false;
    // Register a private measurer client keyed by the track's voidId.
    const auto voidId = t.state.getProperty ("voidId").toString();
    if (voidId.isNotEmpty())
    {
        auto client = std::make_unique<te::LevelMeasurer::Client>();
        lm->measurer.addClient (*client);
        meterClients_[voidId.toStdString()] = std::move (client);
    }
    return true;
}

// -- dispatch ----------------------------------------------------------------
CommandResult EngineSession::applyPersistentCommand (const vp::PersistentCommand& cmd,
                                                     const juce::String& payloadHash)
{
    if (rtCallbackActive)
    {
        // T16 tripwire: mutations must never land on the audio thread.
        juce::Logger::writeToLog ("[rt-safety] persistent command reached audio thread - BUG");
        jassertfalse;
    }

    const auto commandId = juce::String (cmd.command_id() ? cmd.command_id()->c_str() : "");
    const auto txnId     = juce::String (cmd.transaction_id() ? cmd.transaction_id()->c_str() : "");

    if (commandId.isEmpty())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, revision_, "missing command_id" };

    if (cmd.engine_epoch() != 0 && cmd.engine_epoch() != epoch_)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_STALE_EPOCH, revision_, "stale engine epoch" };

    // Exactly-once dedup (CONTRACTS.md §3).
    if (auto it = receipts_.find (commandId.toStdString()); it != receipts_.end())
    {
        const auto& rec = it->second;
        if (rec.payloadHash != payloadHash)
            return { vp::AckStatus_REJECTED, vp::ErrorCode_COMMAND_ID_REUSE, rec.revision,
                     "command_id reuse with different payload" };
        return { vp::AckStatus_DUPLICATE, rec.error, rec.revision, rec.message };
    }

    // Serialized revision gate.
    if (cmd.expected_revision() != revision_)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_STALE_REVISION, revision_,
                 "expected_revision " + juce::String (cmd.expected_revision())
                     + " != current " + juce::String (revision_) };

    CommandResult result { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, revision_, "empty op" };

    if (edit_ != nullptr && txnId.isNotEmpty())
        edit_->getUndoManager().beginNewTransaction (txnId);

    switch (cmd.op_type())
    {
        case vp::PersistentOp_CreateProjectOp:       result = opCreateProject (*cmd.op_as_CreateProjectOp()); break;
        case vp::PersistentOp_OpenProjectOp:         result = opOpenProject (*cmd.op_as_OpenProjectOp()); break;
        case vp::PersistentOp_CloseProjectOp:        result = opCloseProject (*cmd.op_as_CloseProjectOp()); break;
        case vp::PersistentOp_AddTrackOp:            result = opAddTrack (*cmd.op_as_AddTrackOp()); break;
        case vp::PersistentOp_RemoveTrackOp:         result = opRemoveTrack (*cmd.op_as_RemoveTrackOp()); break;
        case vp::PersistentOp_SetTrackNameOp:        result = opSetTrackName (*cmd.op_as_SetTrackNameOp()); break;
        case vp::PersistentOp_SetTrackGainOp:        result = opSetTrackGain (*cmd.op_as_SetTrackGainOp()); break;
        case vp::PersistentOp_SetTrackPanOp:         result = opSetTrackPan (*cmd.op_as_SetTrackPanOp()); break;
        case vp::PersistentOp_SetTrackMuteOp:        result = opSetTrackMute (*cmd.op_as_SetTrackMuteOp()); break;
        case vp::PersistentOp_SetTrackSoloOp:        result = opSetTrackSolo (*cmd.op_as_SetTrackSoloOp()); break;
        case vp::PersistentOp_InsertAudioClipOp:     result = opInsertAudioClip (*cmd.op_as_InsertAudioClipOp()); break;
        case vp::PersistentOp_InsertMidiClipOp:      result = opInsertMidiClip (*cmd.op_as_InsertMidiClipOp()); break;
        case vp::PersistentOp_RemoveClipOp:          result = opRemoveClip (*cmd.op_as_RemoveClipOp()); break;
        case vp::PersistentOp_MoveClipOp:            result = opMoveClip (*cmd.op_as_MoveClipOp()); break;
        case vp::PersistentOp_TrimClipOp:            result = opTrimClip (*cmd.op_as_TrimClipOp()); break;
        case vp::PersistentOp_SplitClipOp:           result = opSplitClip (*cmd.op_as_SplitClipOp()); break;
        case vp::PersistentOp_InsertNoteOp:          result = opInsertNote (*cmd.op_as_InsertNoteOp()); break;
        case vp::PersistentOp_RemoveNoteOp:          result = opRemoveNote (*cmd.op_as_RemoveNoteOp()); break;
        case vp::PersistentOp_SetNoteOp:             result = opSetNote (*cmd.op_as_SetNoteOp()); break;
        case vp::PersistentOp_SetTempoOp:            result = opSetTempo (*cmd.op_as_SetTempoOp()); break;
        case vp::PersistentOp_SetTimeSignatureOp:    result = opSetTimeSignature (*cmd.op_as_SetTimeSignatureOp()); break;
        case vp::PersistentOp_SetLoopRangeOp:        result = opSetLoopRange (*cmd.op_as_SetLoopRangeOp()); break;
        case vp::PersistentOp_SaveProjectOp:         result = opSaveProject (*cmd.op_as_SaveProjectOp()); break;
        case vp::PersistentOp_CreateCheckpointOp:    result = opCreateCheckpoint (*cmd.op_as_CreateCheckpointOp()); break;
        case vp::PersistentOp_UndoOp:                result = opUndo (*cmd.op_as_UndoOp()); break;
        case vp::PersistentOp_RedoOp:                result = opRedo (*cmd.op_as_RedoOp()); break;
        case vp::PersistentOp_AttachAssetOp:         result = opAttachAsset (*cmd.op_as_AttachAssetOp()); break;
        case vp::PersistentOp_InsertPluginOp:        result = opInsertPlugin (*cmd.op_as_InsertPluginOp()); break;
        case vp::PersistentOp_RemovePluginOp:        result = opRemovePlugin (*cmd.op_as_RemovePluginOp()); break;
        case vp::PersistentOp_SetPluginParamOp:      result = opSetPluginParam (*cmd.op_as_SetPluginParamOp()); break;
        case vp::PersistentOp_OpenPluginEditorOp:    result = opOpenPluginEditor (*cmd.op_as_OpenPluginEditorOp()); break;
        case vp::PersistentOp_ClosePluginEditorOp:   result = opClosePluginEditor (*cmd.op_as_ClosePluginEditorOp()); break;
        default:
            result = { vp::AckStatus_REJECTED, vp::ErrorCode_UNSUPPORTED_CAPABILITY,
                       revision_, "unknown operation" };
            break;
    }

    if (result.status == vp::AckStatus_APPLIED)
        revision_ += 1;
    result.revision = revision_;

    receipts_[commandId.toStdString()] = ReceiptRecord {
        payloadHash, result.status, result.error, result.revision, txnId, result.message
    };
    return result;
}

// ===========================================================================
// Transport
// ===========================================================================
void EngineSession::applyTransportRequest (const vp::TransportRequest& req)
{
    if (edit_ == nullptr)
        return;
    auto& tc = edit_->getTransport();

    switch (req.op())
    {
        case vp::TransportOp_PLAY:
            tc.play (false);
            break;
        case vp::TransportOp_STOP:
            tc.stop (true, false);
            break;
        case vp::TransportOp_SEEK:
            tc.setPosition (timeOf (req.position_ticks()));
            break;
        case vp::TransportOp_PANIC:
            tc.stop (true, false);
            // All-notes-off to every enabled MIDI output — the reserved
            // high-priority path must not queue behind edits (it doesn't:
            // this runs on the control lane).
            {
                auto& dm = engine_->getDeviceManager();
                for (int i = 0; i < dm.getNumMidiOutDevices(); ++i)
                    if (auto* dev = dm.getMidiOutDevice (i); dev != nullptr && dev->isEnabled())
                        dev->sendNoteOffMessages();
            }
            break;
        case vp::TransportOp_SET_CYCLE:
        {
            const auto s = req.cycle_start_ticks();
            const auto e = req.cycle_end_ticks();
            if (e > s)
            {
                tc.setLoopRange (timeRangeOf (s, e - s));
                tc.looping = true;
            }
            else
            {
                tc.looping = false;
            }
            break;
        }
        case vp::TransportOp_SET_TEMPO_LIVE:
        {
            // Live preview only: adjusts the running tempo in place; a
            // committed tempo change goes through SetTempoOp.
            auto& ts = edit_->tempoSequence.getTempoAt (te::BeatPosition::fromBeats (0.0));
            if (req.value() > 1.0f && req.value() < 999.0f)
                ts.setBpm (req.value());
            break;
        }
        default: break;
    }
}

// ===========================================================================
// Reads — bounded JSON projections with index cursor pagination.
// ===========================================================================
void EngineSession::handleRead (const vp::ReadRequest& req,
                                std::vector<std::pair<juce::String, juce::String>>& items,
                                juce::String& nextCursor, bool& done,
                                vp::ErrorCode& error)
{
    items.clear();
    nextCursor = {};
    done = true;
    error = vp::ErrorCode_NONE;

    const int limit = req.limit() > 0 ? static_cast<int> (req.limit()) : 200;
    const int startIndex = juce::String (req.cursor() ? req.cursor()->c_str() : "").getIntValue();

    if (edit_ == nullptr)
    {
        error = vp::ErrorCode_NOT_FOUND;
        return;
    }

    int index = 0;
    auto push = [&] (const juce::String& objectId, const juce::String& summary)
    {
        if (index >= startIndex && static_cast<int> (items.size()) < limit)
            items.emplace_back (objectId, summary);
        ++index;
    };

    switch (req.view())
    {
        case vp::ViewKind_PROJECT_SUMMARY:
        {
            auto& ts = edit_->tempoSequence;
            push (projectId_, "{"
                  "\"projectId\":\"" + projectId_ + "\","
                  "\"name\":\"" + esc (edit_->getName()) + "\","
                  "\"revision\":" + juce::String (revision_) + ","
                  "\"engineEpoch\":" + juce::String (epoch_) + ","
                  "\"bpm\":" + juce::String (ts.getBpmAt (te::TimePosition::fromSeconds (0))) + ","
                  "\"numTracks\":" + juce::String (te::getAllTracks (*edit_).size()) + "}");
            break;
        }
        case vp::ViewKind_TRACK_LIST:
        {
            for (auto* t : te::getAllTracks (*edit_))
            {
                auto* at = dynamic_cast<te::AudioTrack*> (t);
                const auto voidId = t->state.getProperty ("voidId").toString();
                juce::String kind = trackKindString (*t);
                if (at != nullptr)
                {
                    bool instrumented = false;
                    for (auto* p : at->pluginList)
                        if (p->isSynth())
                            instrumented = true;
                    if (instrumented) kind = "INSTRUMENT";
                }
                push (voidId,
                      "{\"trackId\":\"" + voidId + "\","
                      "\"kind\":\"" + kind + "\","
                      "\"name\":\"" + esc (t->getName()) + "\","
                      "\"index\":" + juce::String (index) + "}");
            }
            break;
        }
        case vp::ViewKind_CLIP_LIST:
        {
            const auto scope = juce::String (req.track_id() ? req.track_id()->c_str() : "");
            for (auto* t : te::getAllTracks (*edit_))
            {
                auto* at = dynamic_cast<te::AudioTrack*> (t);
                if (at == nullptr)
                    continue;
                const auto tid = at->state.getProperty ("voidId").toString();
                if (scope.isNotEmpty() && scope != tid)
                    continue;
                for (auto* c : at->getClips())
                {
                    const auto cid = c->state.getProperty ("voidId").toString();
                    const auto pos = c->getPosition();
                    const auto beats = te::toBeats (pos.time, edit_->tempoSequence);
                    juce::String clipKind = dynamic_cast<te::MidiClip*> (c) != nullptr ? "MIDI" : "AUDIO";
                    push (cid,
                          "{\"clipId\":\"" + cid + "\","
                          "\"trackId\":\"" + tid + "\","
                          "\"kind\":\"" + clipKind + "\","
                          "\"name\":\"" + esc (c->getName()) + "\","
                          "\"startTicks\":" + juce::String (ticksOf (beats.getStart())) + ","
                          "\"lengthTicks\":" + juce::String (ticksLenOf (beats.getLength())) + "}");
                }
            }
            break;
        }
        case vp::ViewKind_NOTE_RANGE:
        {
            const auto clipScope = juce::String (req.track_id() ? req.track_id()->c_str() : "");
            const auto winStart = req.start_ticks();
            const auto winEnd   = req.end_ticks();
            for (auto& kv : clips_)
            {
                if (clipScope.isNotEmpty() && kv.first != clipScope.toStdString())
                    continue;
                auto* mc = dynamic_cast<te::MidiClip*> (kv.second);
                if (mc == nullptr)
                    continue;
                for (auto* n : mc->getSequence().getNotes())
                {
                    const auto sb = n->getStartBeat();
                    const auto lb = n->getRangeBeats().getLength();
                    const auto st = ticksOf (sb);
                    const auto lt = ticksLenOf (lb);
                    if (winEnd > winStart && ((st + lt) < winStart || st > winEnd))
                        continue; // outside window — still counted for paging
                    const auto nid = n->state.getProperty ("voidId").toString();
                    push (nid,
                          "{\"noteId\":\"" + nid + "\","
                          "\"clipId\":\"" + juce::String (kv.first.c_str()) + "\","
                          "\"pitch\":" + juce::String (n->getNoteNumber()) + ","
                          "\"velocity\":" + juce::String (n->getVelocity()) + ","
                          "\"startTicks\":" + juce::String (st) + ","
                          "\"lengthTicks\":" + juce::String (lt) + "}");
                }
            }
            break;
        }
        case vp::ViewKind_ASSET_LIST:
        {
            for (auto& kv : assets_)
                push (juce::String (kv.first.c_str()),
                      "{\"assetId\":\"" + juce::String (kv.first.c_str()) + "\","
                      "\"relPath\":\"" + esc (kv.second) + "\"}");
            break;
        }
        case vp::ViewKind_PLUGIN_LIST:
        {
            for (auto& kv : plugins_)
            {
                auto* p = kv.second;
                const bool missing = missingPlugins_.count (kv.first) > 0;
                push (juce::String (kv.first.c_str()),
                      "{\"instanceId\":\"" + juce::String (kv.first.c_str()) + "\","
                      "\"name\":\"" + (p != nullptr ? esc (p->getName()) : "") + "\","
                      "\"status\":\"" + juce::String (missing ? "MISSING" : (p != nullptr && p->isEnabled() ? "ACTIVE" : "BYPASSED")) + "\"}");
            }
            for (auto& kv : missingPlugins_)
                if (plugins_.count (kv.first) == 0)
                    push (juce::String (kv.first.c_str()),
                          "{\"instanceId\":\"" + juce::String (kv.first.c_str()) + "\",\"status\":\"MISSING\","
                          "\"reason\":\"" + esc (kv.second) + "\"}");
            break;
        }
        case vp::ViewKind_RECEIPT_LIST:
        {
            for (auto& kv : receipts_)
                push (juce::String (kv.first.c_str()),
                      "{\"commandId\":\"" + juce::String (kv.first.c_str()) + "\","
                      "\"status\":" + juce::String ((int) kv.second.status) + ","
                      "\"revision\":" + juce::String (kv.second.revision) + "}");
            break;
        }
        default:
            error = vp::ErrorCode_BAD_REQUEST;
            return;
    }

    if (index > startIndex + static_cast<int> (items.size()))
    {
        nextCursor = juce::String (startIndex + items.size());
        done = false;
    }
}

void EngineSession::markPluginMissing (const juce::String& instanceId,
                                       const juce::String& reason)
{
    missingPlugins_[instanceId.toStdString()] = reason;
}

} // namespace voidengine
