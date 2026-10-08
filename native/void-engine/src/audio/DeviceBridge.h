// RtProbe: real-time instrumentation attached to the AudioDeviceManager
// callback chain. Counts blocks, deadline overruns and max block duration —
// the T16 evidence channel. Lock-free; the audio thread only writes atomics.
#pragma once

#include <JuceHeader.h>
#include <atomic>

namespace voidengine
{

class RtProbe : public juce::AudioIODeviceCallback
{
public:
    void audioDeviceIOCallbackWithContext (const float* const* input,
                                           int numInputChannels,
                                           float* const* output,
                                           int numOutputChannels,
                                           int numSamples,
                                           const juce::AudioIODeviceCallbackContext& ctx) override;

    void audioDeviceAboutToStart (juce::AudioIODevice* device) override;
    void audioDeviceStopped() override;
    void audioDeviceError (const juce::String& error) override;

    /// T24 fixture: armed by the "void.crash" plugin insert; next callback
    /// terminates the process inside the audio path (evidenced, not hidden).
    std::atomic<bool> armed_ { false };
    void armCrash() noexcept { armed_ = true; }

    std::atomic<uint64_t> callbacks { 0 };
    std::atomic<uint64_t> deviceSamples { 0 };
    std::atomic<double>   maxBlockUs { 0.0 };
    std::atomic<uint64_t> overruns { 0 };
    std::atomic<double>   blockBudgetUs { 0.0 };
    std::atomic<double>   sampleRate { 48000.0 };
    std::atomic<int>      blockSize { 0 };
    std::atomic<juce::AudioIODevice*> activeDevice { nullptr };
};

} // namespace voidengine
