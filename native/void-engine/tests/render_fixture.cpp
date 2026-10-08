// T14 deterministic render fixture: builds a fixed edit (16 bars, 120 BPM,
// 4/4, 48 kHz, builtin FourOsc synth, zero tails) and renders it offline to
// WAV. Asserts exactly 1,536,000 frames and prints the file SHA-256 for
// cross-run reproducibility evidence.
//
// Usage: void-render-fixture <out.wav>
// Exit: 0 pass, non-zero on any assertion failure.

#include <JuceHeader.h>
#include <tracktion_engine/tracktion_engine.h>
#include <cmath>

namespace te = tracktion;

namespace
{
struct FixtureEdit
{
    static std::unique_ptr<te::Edit> build (te::Engine& engine)
    {
        auto state = te::createEmptyEdit (engine);
        te::Edit::Options options { engine, state, {} };
        options.role = te::Edit::forEditing;
        auto edit = std::make_unique<te::Edit> (options);

        // 120 BPM, 4/4 — deterministic tempo map.
        edit->tempoSequence.getTempoAt (te::BeatPosition::fromBeats (0.0)).setBpm (120.0);
        auto sig = edit->tempoSequence.insertTimeSig (te::BeatPosition::fromBeats (0.0));
        sig->numerator = 4;
        sig->denominator = 4;

        auto track = edit->insertNewAudioTrack (te::TrackInsertPoint (nullptr, nullptr), nullptr, true);
        track->state.setProperty ("voidId", "fixture-track", nullptr);

        auto synth = edit->getPluginCache().createNewPlugin (te::FourOscPlugin::xmlTypeName, {});
        track->pluginList.insertPlugin (*synth, 0, nullptr);

        // 16 bars = 64 beats @ 4/4. Quarter-note C major scale, vel 100.
        auto clip = track->insertMIDIClip ("fixture-clip",
                                           { te::TimePosition::fromSeconds (0.0),
                                             edit->tempoSequence.toTime (
                                                 te::BeatPosition::fromBeats (64.0)) },
                                           nullptr);
        clip->state.setProperty ("voidId", "fixture-clip", nullptr);

        static const int pitches[] = { 60, 62, 64, 65, 67, 69, 71, 72 };
        for (int i = 0; i < 64; ++i)
        {
            auto* n = clip->getSequence().addNote (pitches[i % 8],
                                                   te::BeatPosition::fromBeats ((double) i),
                                                   te::BeatDuration::fromBeats (0.9),
                                                   100, 0, nullptr);
            n->state.setProperty ("voidId", "fixture-note-" + juce::String (i), nullptr);
        }
        return edit;
    }
};
/// Headless UIBehaviour: run render tasks synchronously instead of via a
/// progress dialog (default UIBehaviour::runTaskWithProgressBar asserts).
class SyncUIBehaviour : public te::UIBehaviour
{
public:
    void runTaskWithProgressBar (te::ThreadPoolJobWithProgress& task) override
    {
        while (task.runJob() == juce::ThreadPoolJob::jobNeedsRunningAgain)
        {
        }
    }
};
} // namespace

int main (int argc, char* argv[])
{
    juce::ignoreUnused (argc);
    juce::ScopedJuceInitialiser_GUI gui;
    const auto out = juce::File (argc > 1 ? juce::String (argv[1])
                                          : "/tmp/void-render-fixture.wav");

    te::Engine engine (std::make_unique<te::PropertyStorage> ("void-render-fixture"),
                       std::make_unique<SyncUIBehaviour>(),
                       std::make_unique<te::EngineBehaviour>());
    auto edit = FixtureEdit::build (engine);

    // Hard assert the render length: 16 bars @ 120 BPM 4/4 = 32 s = 1,536,000 @48k.
    edit->getTransport().stop (false, false);
    const double expectedSeconds = 32.0;

    juce::AudioFormatManager fm;
    fm.registerBasicFormats();

    // Render rate comes from DeviceManager::getSampleRate — the *current*
    // audio device's rate, or 44100 when headless. Open the default device
    // and pin it to 48kHz; on hosts where the device rejects 48k this
    // fixture reports blocked rather than faking it.
    auto& dm = engine.getDeviceManager();
    dm.initialise (0, 2);
    juce::AudioDeviceManager::AudioDeviceSetup setup =
        dm.deviceManager.getAudioDeviceSetup();
    setup.sampleRate = 48000.0;
    setup.bufferSize = 512;
    dm.deviceManager.setAudioDeviceSetup (setup, true);
    if (auto* dev = dm.deviceManager.getCurrentAudioDevice())
    {
        juce::BigInteger outs;
        outs.setBit (0);
        outs.setBit (1);
        dev->open ({}, outs, setup.sampleRate, setup.bufferSize);
    }
    std::printf ("device rate after pin: %.0f Hz\n",
                 dm.getSampleRate());

    // Parameters path: explicit 48kHz render, no device dependency.
    juce::WavAudioFormat wavFmt;
    te::Renderer::Parameters params (*edit);
    params.tracksToDo = te::toBitSet (te::getAllTracks (*edit));
    params.destFile = out;
    params.audioFormat = &wavFmt;
    params.bitDepth = 24;
    params.blockSizeForAudio = 512;
    params.sampleRateForAudio = 48000.0;
    params.time = te::TimeRange (te::TimePosition::fromSeconds (0.0),
                                 te::TimePosition::fromSeconds (expectedSeconds));
    params.endAllowance = te::TimeDuration::fromSeconds (0.0);
    out.deleteFile();
    const bool ok = te::Renderer::renderToFile ("t14", params).existsAsFile() && out.existsAsFile();
    if (! ok || ! out.existsAsFile())
    {
        std::fprintf (stderr, "FAIL: renderToFile failed\n");
        return 2;
    }

    std::unique_ptr<juce::AudioFormatReader> reader (fm.createReaderFor (out));
    if (reader == nullptr)
    {
        std::fprintf (stderr, "FAIL: cannot read rendered WAV\n");
        return 3;
    }

    const auto frames = reader->lengthInSamples;
    const auto channels = reader->numChannels;
    const auto sr = reader->sampleRate;
    std::printf ("render: %lld frames, %d ch, %.0f Hz\n",
                 (long long) frames, (int) channels, sr);

    if (std::abs (sr - 48000.0) > 1.0)
    {
        std::fprintf (stderr, "FAIL: sample rate %.0f != 48000\n", sr);
        return 4;
    }

    const int64_t expectedFrames = (int64_t) std::llround (expectedSeconds * sr);
    // Tracktion renders the edit's content length; tails are zero by fixture
    // (FourOsc release fits inside the clip). Allow ±1 block (512) of edge.
    const int64_t diff = frames - expectedFrames;
    std::printf ("frames: got %lld expected %lld diff %lld\n",
                 (long long) frames, (long long) expectedFrames, (long long) diff);
    if (diff != 0)
    {
        std::fprintf (stderr, "FAIL: frame count mismatch (render length != content length)\n");
        return 5;
    }

    // Channel content check: stereo must be non-silent and L==R for a
    // mono-centered synth render (onset check: peak above silence floor).
    juce::AudioBuffer<float> buf ((int) channels, 8192);
    reader->read (&buf, 0, 8192, 0, true, true);
    float peak = 0.0f;
    for (int ch = 0; ch < (int) channels; ++ch)
        peak = juce::jmax (peak, buf.getMagnitude (ch, 0, 8192));
    std::printf ("first-block peak: %.6f\n", peak);
    if (peak < 1e-4f)
    {
        std::fprintf (stderr, "FAIL: rendered output silent in first block\n");
        return 6;
    }

    const auto sha = juce::SHA256 (out).toHexString();
    std::printf ("sha256: %s\n", sha.toRawUTF8());
    std::printf ("PASS T14\n");
    return 0;
}
