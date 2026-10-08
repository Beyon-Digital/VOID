// VOID engine session: owns the Tracktion Engine + authoritative Edit,
// maps VOID stable IDs to engine objects, and applies serialized
// persistent commands on the message thread (the single mutation lane).
#pragma once

#include <JuceHeader.h>
#include <tracktion_engine/tracktion_engine.h>
#include "void_control_generated.h"
#include "../recording/RecordingManager.h"
#include <functional>
#include <memory>
#include <unordered_map>

namespace te = tracktion;

namespace voidengine
{

namespace vp = voidproto;

/// Ticks per quarter note on the wire (CONTRACTS.md §1).
inline constexpr double kTicksPerQuarter = 960000.0;

struct CommandResult
{
    vp::AckStatus status   = vp::AckStatus_APPLIED;
    vp::ErrorCode error    = vp::ErrorCode_NONE;
    uint64_t      revision = 0;
    juce::String  message;
};

/// Receipt retention for duplicate detection (per CONTRACTS.md §3).
struct ReceiptRecord
{
    juce::String payloadHash;
    vp::AckStatus status;
    vp::ErrorCode error;
    uint64_t      revision;
    juce::String  transactionId;
    juce::String  message;
};

class EngineSession
{
public:
    explicit EngineSession (uint64_t engineEpoch);
    ~EngineSession();

    /// Apply one decoded PersistentCommand on the message thread.
    CommandResult applyPersistentCommand (const vp::PersistentCommand& cmd,
                                          const juce::String& payloadHash);

    /// Apply a TransportRequest (scoped, non-persistent).
    void applyTransportRequest (const vp::TransportRequest& req);

    /// Answer a ReadRequest; fills bounded JSON items + next cursor.
    void handleRead (const vp::ReadRequest& req,
                     std::vector<std::pair<juce::String, juce::String>>& items,
                     juce::String& nextCursor, bool& done,
                     vp::ErrorCode& error);

    uint64_t revision() const noexcept { return revision_; }
    uint64_t epoch()   const noexcept { return epoch_; }
    juce::String projectId() const noexcept { return projectId_; }
    bool hasProject() const noexcept { return edit_ != nullptr; }

    te::Engine& engine() { return *engine_; }
    te::Edit*   edit()   { return edit_.get(); }

    /// Device-bridge hooks (audio/DeviceBridge.cpp installs callbacks).
    std::function<void (const juce::String& trackId, float peakL, float peakR,
                        float rmsL, float rmsR, bool clipped)> onMeterFrame;

    /// Stress-fixture switch (T16): log a warning if an Edit mutation is
    /// attempted on the audio callback thread.
    static thread_local bool rtCallbackActive;

    /// Crash fixture (T24): deliberate in-process fault on next audio block
    /// after a crashing plugin instance has been inserted.
    void armCrashFixture() noexcept { crashArmed_ = true; }
    bool crashArmed() const noexcept { return crashArmed_; }

    /// Recording subsystem (W08). Valid while a project is open.
    RecordingManager* recording() noexcept { return recording_.get(); }
    /// Public track lookup for subsystems (recording, monitor routing).
    te::AudioTrack* findTrackById (const juce::String& trackId) const { return findTrack (trackId); }
    /// Absolute container dir as a string ("" when no project is open).
    juce::String containerDirPath() const { return containerDirStr_; }

    /// Record that a plugin failed to load during project open (missing /
    /// incompatible): keeps the slot marked, never throws.
    void markPluginMissing (const juce::String& instanceId, const juce::String& reason);

    /// JSON blob describing recording subsystem state for PROJECT_SUMMARY.
    juce::String recordingSummaryJson() const;

private:
    // -- persistent ops -----------------------------------------------------
    CommandResult opCreateProject (const vp::CreateProjectOp&);
    CommandResult opOpenProject   (const vp::OpenProjectOp&);
    CommandResult opCloseProject  (const vp::CloseProjectOp&);
    CommandResult opAddTrack      (const vp::AddTrackOp&);
    CommandResult opRemoveTrack   (const vp::RemoveTrackOp&);
    CommandResult opSetTrackName  (const vp::SetTrackNameOp&);
    CommandResult opSetTrackGain  (const vp::SetTrackGainOp&);
    CommandResult opSetTrackPan   (const vp::SetTrackPanOp&);
    CommandResult opSetTrackMute  (const vp::SetTrackMuteOp&);
    CommandResult opSetTrackSolo  (const vp::SetTrackSoloOp&);
    CommandResult opInsertAudioClip (const vp::InsertAudioClipOp&);
    CommandResult opInsertMidiClip  (const vp::InsertMidiClipOp&);
    CommandResult opRemoveClip    (const vp::RemoveClipOp&);
    CommandResult opMoveClip      (const vp::MoveClipOp&);
    CommandResult opTrimClip      (const vp::TrimClipOp&);
    CommandResult opSplitClip     (const vp::SplitClipOp&);
    CommandResult opInsertNote    (const vp::InsertNoteOp&);
    CommandResult opRemoveNote    (const vp::RemoveNoteOp&);
    CommandResult opSetNote       (const vp::SetNoteOp&);
    CommandResult opSetTempo      (const vp::SetTempoOp&);
    CommandResult opSetTimeSignature (const vp::SetTimeSignatureOp&);
    CommandResult opSetLoopRange  (const vp::SetLoopRangeOp&);
    CommandResult opSaveProject   (const vp::SaveProjectOp&);
    CommandResult opCreateCheckpoint (const vp::CreateCheckpointOp&);
    CommandResult opUndo          (const vp::UndoOp&);
    CommandResult opRedo          (const vp::RedoOp&);
    CommandResult opAttachAsset   (const vp::AttachAssetOp&);
    CommandResult opInsertPlugin  (const vp::InsertPluginOp&);
    CommandResult opRemovePlugin  (const vp::RemovePluginOp&);
    CommandResult opSetPluginParam (const vp::SetPluginParamOp&);
    CommandResult opOpenPluginEditor  (const vp::OpenPluginEditorOp&);
    CommandResult opClosePluginEditor (const vp::ClosePluginEditorOp&);

    // Implemented in plugins/PluginHost.cpp
    CommandResult insertExternalPlugin (const vp::InsertPluginOp&);
    CommandResult removePlugin (const vp::RemovePluginOp&);
    CommandResult setPluginParam (const vp::SetPluginParamOp&);
    CommandResult openPluginEditor (const vp::OpenPluginEditorOp&);
    CommandResult closePluginEditor (const vp::ClosePluginEditorOp&);

    // -- helpers ------------------------------------------------------------
    te::AudioTrack* findTrack (const juce::String& trackId) const;
    te::Clip*       findClip  (const juce::String& clipId) const;
    te::Plugin*     findPlugin (const juce::String& instanceId) const;
    static juce::String trackKindString (const te::Track&);

    juce::File containerDir() const;
    juce::File assetFile (const juce::String& relPath) const;

    /// Minimal checkpoint write (SaveBridge.cpp). Full W05 state machine is
    /// owned by the persistence lane; this writes a complete verified
    /// checkpoint dir + CURRENT pointer for F0 evidence.
    CommandResult writeCheckpoint (const juce::String& reason,
                                   juce::String& checkpointIdOut);

    void rebuildIndexes();
    void indexClipTree (te::Clip&, const juce::String& trackId);
    bool attachLevelMeter (te::AudioTrack&);

public:
    /// Registered LevelMeasurer::Client for a track's meter (nullptr if none).
    te::LevelMeasurer::Client* meterClientFor (const juce::String& trackId) const
    {
        auto it = meterClients_.find (trackId.toStdString());
        return it != meterClients_.end() ? it->second.get() : nullptr;
    }

private:

    // -- conversions --------------------------------------------------------
    tracktion::BeatPosition beatsOf (int64_t ticks) const;
    tracktion::BeatDuration beatLenOf (int64_t ticks) const;
    tracktion::TimePosition timeOf (int64_t ticks) const;
    tracktion::TimeRange    timeRangeOf (int64_t startTicks, int64_t lenTicks) const;
    int64_t ticksOf (tracktion::BeatPosition) const;
    int64_t ticksLenOf (tracktion::BeatDuration) const;

    std::unique_ptr<te::Engine> engine_;
    std::unique_ptr<te::Edit>   edit_;
    juce::UndoManager           undoHistory_ { 500, 64 * 1024 };

    uint64_t     epoch_    = 0;
    uint64_t     revision_ = 0;
    juce::String projectId_;
    juce::String containerDirStr_;
    juce::String projectName_;

    // Objects are owned by the Edit; entries are valid only while an Edit is
    // open and are cleared/rebuilt on open/close/undo/redo (WeakReference is
    // unusable on Clip/MidiNote: Selectable multiple-inheritance).
    std::unordered_map<std::string, te::AudioTrack*> tracks_;
    std::unordered_map<std::string, te::Clip*>       clips_;
    std::unordered_map<std::string, te::MidiNote*>   notes_;
    std::unordered_map<std::string, te::Plugin*>     plugins_;
    std::unordered_map<std::string, juce::String> clipToTrack_;
    std::unordered_map<std::string, juce::String> noteToClip_;
    std::unordered_map<std::string, std::unique_ptr<te::LevelMeasurer::Client>> meterClients_;
    std::unordered_map<std::string, juce::String> missingPlugins_;
    std::unordered_map<std::string, ReceiptRecord> receipts_;
    std::unordered_map<std::string, juce::String>  assets_; // asset_id -> rel path

    bool crashArmed_ = false;
    VoidEngineBehaviour* voidBehaviour_ = nullptr; // owned by engine_
    std::unique_ptr<RecordingManager> recording_;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (EngineSession)
};

} // namespace voidengine
