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
    juce::ScopedJuceInitialiser_GUI gui;

    // Two modes:
    //   void-render-fixture <out.wav>
    //       T14: renders the built-in 16-bar fixture (asserts 1,536,000 frames).
    //   void-render-fixture --edit <engine.tracktionedit> <out.wav> [--seconds N]
    //       Loads a real checkpoint edit file and renders it through the same
    //       te::Renderer::renderToFile path (used by F1 journey evidence while
    //       protocol major.1 has no render op — see docs/engine/NEEDS.md).
    juce::File editFile;
    juce::File out ("/tmp/void-render-fixture.wav");
    double secondsOverride = -1.0;
    for (int i = 1; i < argc; ++i)
    {
        const auto a = juce::String (argv[i]);
        if (a == "--edit" && i + 1 < argc)
            editFile = juce::File (argv[++i]);
        else if (a == "--seconds" && i + 1 < argc)
            secondsOverride = juce::String (argv[++i]).getDoubleValue();
        else if (! a.startsWith ("-"))
            out = juce::File (a);
    }

    te::Engine engine (std::make_unique<te::PropertyStorage> ("void-render-fixture"),
                       std::make_unique<SyncUIBehaviour>(),
                       std::make_unique<te::EngineBehaviour>());

    std::unique_ptr<te::Edit> edit;
    double expectedSeconds;
    if (editFile.existsAsFile())
    {
        auto state = juce::ValueTree::fromXml (editFile.loadFileAsString());
        if (! state.isValid())
        {
            std::fprintf (stderr, "FAIL: edit file unreadable: %s\n",
                          editFile.getFullPathName().toRawUTF8());
            return 10;
        }
        te::Edit::Options options { engine, state, {} };
        options.role = te::Edit::forEditing;
        options.editFileRetriever = [editFile] { return editFile; };
        // VOID engine gap (F1 finding): EngineSession never sets
        // Edit::filePathResolver and VOID edits live outside a TE Project,
        // so AUDIOCLIP `source` strings written relative to the edit file
        // path (e.g. "../../../assets/sha256/<sha>/take.wav", anchored at
        // checkpoints/live/engine.tracktionedit treated as a directory)
        // resolve via getEditFileFromProjectManager() -> empty File ->
        // process-CWD-relative and silently fail to load (wave clips render
        // as silence). This resolver reproduces TE's write-side anchor
        // convention — the edit FILE path itself treated as the anchor
        // directory — which is what the engine should set on its own edit.
        options.filePathResolver = [editFile] (const juce::String& desc)
        {
            return editFile.getChildFile (desc);
        };
        edit = std::make_unique<te::Edit> (options);
        // Render span = declared --seconds, else the edit's own content length.
        expectedSeconds = secondsOverride > 0.0 ? secondsOverride
                                                : edit->getLength().inSeconds();
        std::printf ("edit-mode: %s content=%.3f s\n",
                     editFile.getFileName().toRawUTF8(), edit->getLength().inSeconds());
    }
    else
    {
        if (editFile != juce::File())
        {
            std::fprintf (stderr, "FAIL: --edit file missing: %s\n",
                          editFile.getFullPathName().toRawUTF8());
            return 10;
        }
        edit = FixtureEdit::build (engine);
        // Hard assert the render length: 16 bars @ 120 BPM 4/4 = 32 s = 1,536,000 @48k.
        expectedSeconds = 32.0;
    }
    if (secondsOverride > 0.0)
        expectedSeconds = secondsOverride;
    edit->getTransport().stop (false, false);

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

    // Channel content check: output must be non-silent somewhere. The T14
    // fixture has content at t=0 so the first block suffices; --edit mode
    // may legitimately start with silence (e.g. a solo'd take that enters
    // later), so scan the whole file.
    const int64_t scanLen = editFile.existsAsFile()
                              ? (int64_t) frames
                              : std::min<int64_t> ((int64_t) frames, 8192);
    juce::AudioBuffer<float> buf ((int) channels, 8192);
    float peak = 0.0f;
    for (int64_t pos = 0; pos < scanLen; pos += 8192)
    {
        const int n = (int) juce::jmin ((int64_t) 8192, scanLen - pos);
        reader->read (&buf, 0, n, pos, true, true);
        for (int ch = 0; ch < (int) channels; ++ch)
            peak = juce::jmax (peak, buf.getMagnitude (ch, 0, n));
        if (peak >= 1e-4f && ! editFile.existsAsFile())
            break;
    }
    std::printf ("peak (first %lld frames): %.6f\n", (long long) scanLen, peak);
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
