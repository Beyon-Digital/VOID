// MidiCapture: taps an armed MIDI InputDeviceInstance through the engine's
// Consumer interface and appends timestamped events to the take journal's
// midi-events.jsonl chunk — off the audio callback.
//
// The callback-side path is push-only into a SPSC lock-free FIFO (no locks,
// no allocation). A message-thread drain writes events, preserving
// sustain/bend/aftertouch byte-exactly.
#pragma once

#include <JuceHeader.h>
#include <tracktion_engine/tracktion_engine.h>

namespace te = tracktion;

namespace voidengine
{

class MidiCapture final : public te::InputDeviceInstance::Consumer,
                          private juce::Timer
{
public:
    MidiCapture();
    ~MidiCapture() override;

    /// Attach to an input instance + start draining to <takeDir>/midi-events.jsonl.
    /// `takeStartTime` is the engine time position at which capture starts.
    bool begin (te::InputDeviceInstance& instance, const juce::File& takeDir,
                const juce::String& trackId);
    /// Stop draining and flush remaining events. Safe at any state.
    void end();

    bool active() const noexcept { return active_.load(); }
    uint64_t eventsCaptured() const noexcept { return eventsWritten_; }

    /// Called on panic/disconnect: synthesizes note-offs for every key the
    /// device still tracks, so no held note survives a drop. Returns count.
    int emitAllNotesOff (te::MidiInputDevice& device);

    // te::InputDeviceInstance::Consumer — realtime thread, push-only.
    void handleIncomingMidiMessage (const juce::MidiMessage&, te::MPESourceID) override;
    void discardRecordings (te::EditItemID) override;

private:
    struct Event
    {
        uint8_t bytes[3] { 0, 0, 0 };
        uint8_t size = 0;
        double stamp = 0.0;     // midi timestamp (device/host domain seconds)
        uint64_t hostNs = 0;    // steady-clock capture time
    };

    void timerCallback() override;
    void drain();

    juce::AbstractFifo fifo_ { 2048 };
    std::vector<Event> fifoBuf_ { (size_t) 2048 };
    te::InputDeviceInstance* instance_ = nullptr;
    juce::File outFile_;
    juce::String trackId_;
    std::unique_ptr<juce::FileOutputStream> out_;
    std::atomic<bool> active_ { false };
    uint64_t eventsWritten_ = 0;
    bool discarded_ = false;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (MidiCapture)
};

} // namespace voidengine
