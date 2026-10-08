// RecordingManager: per-project recording subsystem for the VOID engine.
//
// Owns the arm/disarm/input-routing state, monitoring mode, count-in +
// metronome (edit click track), cycle/punch boundaries and the take journal.
// Maps onto Tracktion's InputDeviceInstance / TransportControl machinery —
// never invents its own audio path.
//
// Threading: every method runs on the JUCE message thread (same serialized
// lane as EngineSession ops). Journal writes happen from timerCallback /
// stop paths on that thread — the audio callback is never touched.
//
// Missing-wire note: no arm/record ops exist in void_control.fbs protocol
// minor 0; the gaps are recorded in docs/engine/NEEDS.md. This class is driven
// by the session wiring (STOP/PANIC/close) today and by the recording fixture
// binary for tests.
#pragma once

#include <JuceHeader.h>
#include <tracktion_engine/tracktion_engine.h>
#include "TakeJournal.h"
#include "MidiCapture.h"

namespace te = tracktion;

namespace voidengine
{

class EngineSession;

class RecordingManager final : private te::TransportControl::Listener,
                               private juce::ChangeListener,
                               private juce::Timer
{
public:
    enum class Monitor { off, automatic, on };
    enum class Phase { idle, armed, recording, stopping, failed };

    explicit RecordingManager (EngineSession& session);
    ~RecordingManager() override;

    // -- setup (message thread) ------------------------------------------
    /// Arm a track onto an input. inputName: "" = first wave input (audio) /
    /// first MIDI input (midi tracks); "track:<voidId>" = another track's
    /// output (trackWaveDevice/trackMidiDevice); "midi-hosted" = hosted/
    /// virtual midi input. Returns CommandResult-compatible error text.
    juce::Result arm (te::AudioTrack& dest, const juce::String& inputName,
                      bool midi, Monitor monitor);
    juce::Result disarm (te::AudioTrack& dest);
    juce::Result setMonitor (te::AudioTrack& dest, Monitor);

    void setCountIn (te::Edit::CountIn);
    void setMetronome (bool enabled, float gain, bool recordingOnly);
    /// Punch-in/out = TE recordingPunchInOut + loop range (SET_CYCLE owns it).
    void setPunchInOut (bool enabled);

    // -- lifecycle ---------------------------------------------------------
    /// Starts TE recording (tc.record). Checks disk headroom first —
    /// DEVICE_UNAVAILABLE / DISK_FULL never report success.
    juce::Result startRecording();
    /// Stops recording; discard=true drops takes (journal: discarded).
    juce::Result stopRecording (bool discard);
    /// Live punch: enable/disable record on an armed target mid-take.
    juce::Result punchIn (te::AudioTrack& dest);
    juce::Result punchOut (te::AudioTrack& dest);

    /// Reserved high-priority path: all-notes-off + stop. Never queues behind
    /// edits — call directly on the control lane.
    void panic();
    /// Transport stopped externally: finalize pending journal entries.
    void transportStopped();
    /// Device list changed: recording inputs lost -> stop + mark failed.
    void deviceListChanged();

    // -- journal / queries ---------------------------------------------------
    Phase phase() const noexcept { return phase_; }
    bool  isRecording() const noexcept;
    const TakeJournal* currentTake() const noexcept { return journal_ ? &*journal_ : nullptr; }
    juce::String lastError() const { return lastError_; }
    juce::Array<juce::String> armedTrackItemIds() const;

    /// Scan recordings/ for crashed takes; called on project open.
    void recoverIncompleteTakes();
    /// Tear-down: notes off, close journal, detach consumers.
    void shutdown();

private:
    struct ArmedTarget
    {
        te::AudioTrack* track = nullptr;
        te::InputDeviceInstance* instance = nullptr;
        te::InputDeviceInstance::Destination* destination = nullptr;
        bool midi = false;
        std::unique_ptr<MidiCapture> capture;
    };

    // te::TransportControl::Listener
    void recordingStarted (te::SyncPoint, std::optional<te::TimeRange>) override;
    void recordingStopped (te::SyncPoint, bool discardRecordings) override;
    void recordingFinished (te::InputDeviceInstance&, te::EditItemID,
                            const juce::ReferenceCountedArray<te::Clip>&) override;
    // juce::ChangeListener — device list changes mid-take.
    void changeListenerCallback (juce::ChangeBroadcaster*) override;
    // juce::Timer — journal chunk-length updates while recording.
    void timerCallback() override;

    ArmedTarget* armedFor (te::AudioTrack&) const;
    ArmedTarget* armedForItemId (te::EditItemID) const;
    te::EditPlaybackContext* contextOrNull() const;
    juce::Result fail (const juce::String& msg, bool stopTake = true);
    juce::File container() const;
    void updateJournalChunks();
    void scanTakeDirChunks();

    EngineSession& session_;
    std::vector<std::unique_ptr<ArmedTarget>> armed_;
    std::optional<TakeJournal> journal_;
    juce::File takeDir_;
    Phase phase_ = Phase::idle;
    juce::String lastError_;
    bool shuttingDown_ = false;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (RecordingManager)
};

/// EngineBehaviour that routes take files into recordings/<takeId>/ while a
/// take is live (TE asks it for the destination filename per target).
class VoidEngineBehaviour final : public te::EngineBehaviour
{
public:
    /// Set/cleared around tc.record() by RecordingManager.
    std::function<juce::File (te::Track&)> takeFileProvider;
    juce::File getFileForNewAudioRecording (te::Track&, const juce::String& ext) override;
};

} // namespace voidengine
