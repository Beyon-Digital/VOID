#include "DeviceBridge.h"
#include "../session/EngineSession.h"

namespace voidengine
{

void RtProbe::audioDeviceIOCallbackWithContext (const float* const* input,
                                              int numInputChannels,
                                              float* const* output,
                                              int numOutputChannels,
                                              int numSamples,
                                              const juce::AudioIODeviceCallbackContext& ctx)
{
    EngineSession::rtCallbackActive = true;
    const auto t0 = juce::Time::getHighResolutionTicks();
    callbacks.fetch_add (1, std::memory_order_relaxed);
    deviceSamples.fetch_add ((uint64_t) numSamples, std::memory_order_relaxed);

    // T24 crash fixture: the "void.crash" plugin arms this; the fault happens
    // inside the audio callback like a real crashing plugin would.
    if (armed_.load (std::memory_order_relaxed))
        juce::Process::terminate();

    for (int ch = 0; ch < numOutputChannels; ++ch)
        if (auto* b = output[ch])
            juce::FloatVectorOperations::clear (b, numSamples);
    juce::ignoreUnused (input, numInputChannels, ctx);

    const auto us = juce::Time::highResolutionTicksToSeconds (
                        juce::Time::getHighResolutionTicks() - t0) * 1e6;
    auto cur = maxBlockUs.load (std::memory_order_relaxed);
    while (us > cur && ! maxBlockUs.compare_exchange_weak (cur, us)) {}
    if (blockBudgetUs.load() > 0.0 && us > blockBudgetUs.load())
        overruns.fetch_add (1, std::memory_order_relaxed);
    EngineSession::rtCallbackActive = false;
}

void RtProbe::audioDeviceAboutToStart (juce::AudioIODevice* device)
{
    activeDevice = device;
    sampleRate = device ? device->getCurrentSampleRate() : 48000.0;
    blockSize = device ? device->getCurrentBufferSizeSamples() : 0;
    if (sampleRate.load() > 0.0 && blockSize.load() > 0)
        blockBudgetUs = 1e6 * blockSize.load() / sampleRate.load();
}

void RtProbe::audioDeviceStopped() { activeDevice = nullptr; }
void RtProbe::audioDeviceError (const juce::String&) {}

} // namespace voidengine
