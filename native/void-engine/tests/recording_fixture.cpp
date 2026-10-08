// W08 recording fixture: drives EngineSession + RecordingManager in-process
// through the hosted audio device (test_utilities::EnginePlayer) so real takes
// — audio WAV chunks, MIDI events, journals — are produced headless with no
// hardware. Commands:
//
//   void-recording-fixture take <container> [--stereo] [--midi]
//                                     [--kill-after-blocks N] [--punch]
//   void-recording-fixture recover <container>
//   void-recording-fixture latency [inLatencySamples outLatencySamples]
//   void-recording-fixture errors
//   void-recording-fixture panic <container>
//
// Exit: 0 = assertions passed; non-zero on first failure. Prints PASS/FAIL
// lines plus a JSON evidence object on stdout.

#include <JuceHeader.h>
#include <tracktion_engine/tracktion_engine.h>
#include <tracktion_engine/testing/tracktion_EnginePlayer.h>
#include "../src/session/EngineSession.h"
#include "../src/recording/RecordingManager.h"
#include "void_control_generated.h"
#include <cmath>

namespace te = tracktion;
namespace vp = voidproto;
using namespace voidengine;

namespace
{

juce::uint64 gRev = 0;
juce::uint64 gCmd = 0;

/// Serialize + apply a persistent command through the real dispatch lane
/// (epoch, revision and dedup gates all apply).
CommandResult send (EngineSession& s, flatbuffers::FlatBufferBuilder& b)
{
    auto* cmd = flatbuffers::GetRoot<vp::PersistentCommand> (b.GetBufferPointer());
    const auto hash = juce::SHA256 (juce::MemoryBlock (b.GetBufferPointer(),
                                                     (size_t) b.GetSize())).toHexString();
    return s.applyPersistentCommand (*cmd, hash);
}

/// Stable, monotonically-increasing command id for the session.
static juce::String nextCmdId()
{
    return "cmd-" + juce::String (++gCmd);
}

CommandResult createProject (EngineSession& s, const juce::File& dir)
{
    flatbuffers::FlatBufferBuilder b;
    auto op = vp::CreateCreateProjectOpDirect (b, "rec-fixture",
                                             dir.getFullPathName().toRawUTF8(),
                                             48000, 120.0f);
    const auto cid = nextCmdId();
    auto cmd = vp::CreatePersistentCommandDirect (b, cid.toRawUTF8(),
                                                  "txn-rec", "", 42, gRev,
                                                  vp::PersistentOp_CreateProjectOp, op.Union());
    b.Finish (cmd);
    auto r = send (s, b);
    if (r.status == vp::AckStatus_APPLIED) gRev = r.revision;
    return r;
}

CommandResult openProject (EngineSession& s, const juce::File& dir)
{
    flatbuffers::FlatBufferBuilder b;
    auto op = vp::CreateOpenProjectOpDirect (b, dir.getFullPathName().toRawUTF8());
    const auto cid = nextCmdId();
    auto cmd = vp::CreatePersistentCommandDirect (b, cid.toRawUTF8(),
                                                  "txn-rec", "", 42, gRev,
                                                  vp::PersistentOp_OpenProjectOp, op.Union());
    b.Finish (cmd);
    auto r = send (s, b);
    if (r.status == vp::AckStatus_APPLIED) gRev = r.revision;
    return r;
}

CommandResult addTrack (EngineSession& s, const char* id, vp::TrackKind kind)
{
    flatbuffers::FlatBufferBuilder b;
    auto op = vp::CreateAddTrackOpDirect (b, id, kind, id, -1);
    const auto cid = nextCmdId();
    auto cmd = vp::CreatePersistentCommandDirect (b, cid.toRawUTF8(),
                                                  "txn-rec", "", 42, gRev,
                                                  vp::PersistentOp_AddTrackOp, op.Union());
    b.Finish (cmd);
    auto r = send (s, b);
    if (r.status == vp::AckStatus_APPLIED) gRev = r.revision;
    return r;
}

bool check (bool cond, const char* name)
{
    std::printf ("%s %s\n", cond ? "PASS" : "FAIL", name);
    return cond;
}

/// Fill `buf` with a deterministic 440Hz sine at `phase` (advances by ref).
void fillSine (juce::AudioBuffer<float>& buf, double& phase, double rate)
{
    for (int ch = 0; ch < buf.getNumChannels(); ++ch)
    {
        auto* d = buf.getWritePointer (ch);
        double p = phase;
        for (int i = 0; i < buf.getNumSamples(); ++i)
        {
            d[i] = (float) (0.4 * std::sin (p) * (ch == 0 ? 1.0 : 0.5));
            p += 2.0 * juce::MathConstants<double>::pi * 440.0 / rate;
        }
        if (ch == 0) phase = p;
    }
}

te::HostedAudioDeviceInterface::Parameters hostedParams (int inCh, int outCh,
                                                        double rate, int block,
                                                        int inLat = 0, int outLat = 0)
{
    te::HostedAudioDeviceInterface::Parameters p;
    p.sampleRate = rate;
    p.blockSize = block;
    p.inputChannels = inCh;
    p.outputChannels = outCh;
    p.inputLatencyNumSamples = inLat;
    p.outputLatencyNumSamples = outLat;
    return p;
}

te::AudioTrack* mustTrack (EngineSession& s, const char* id)
{
    return s.findTrackById (id);
}

/// List recordings/<takeId>/ dirs and their journals as JSON text.
juce::String recordingsJson (const juce::File& container)
{
    juce::String out = "[";
    const auto root = container.getChildFile ("recordings");
    bool first = true;
    if (root.isDirectory())
        for (auto& d : root.findChildFiles (juce::File::findDirectories, false))
        {
            TakeJournal j;
            juce::File td (d);
            if (! TakeJournalFile::read (td, j))
                continue;
            if (! first) out += ",";
            first = false;
            out += "{\"takeId\":\"" + j.takeId + "\",\"status\":\""
                 + takeStatusString (j.status) + "\",\"chunks\":"
                 + juce::String ((int) j.chunks.size()) + ",\"clips\":"
                 + juce::String (j.clipIds.size()) + ",\"trackIds\":\""
                 + j.trackIds + "\"}";
        }
    out += "]";
    return out;
}

int cmdTake (const juce::File& container, bool stereo, bool midi,
             int killBlocks, bool punch)
{
    EngineSession s (42);
    te::test_utilities::EnginePlayer player (s.engine(),
        hostedParams (2, 2, 48000.0, 512));
    std::printf ("hosted-device: rate=%.0f block=%d waveIns=%d\n",
                 s.engine().getDeviceManager().getSampleRate(),
                 s.engine().getDeviceManager().getBlockSize(),
                 (int) s.engine().getDeviceManager().getWaveInputDevices().size());

    if (! check (createProject (s, container).status == vp::AckStatus_APPLIED, "create-project"))
        return 2;
    if (! check (addTrack (s, "dest-a", vp::TrackKind_AUDIO).status == vp::AckStatus_APPLIED, "add-track-a"))
        return 2;
    if (! check (addTrack (s, "dest-b", vp::TrackKind_AUDIO).status == vp::AckStatus_APPLIED, "add-track-b"))
        return 2;
    if (midi && ! check (addTrack (s, "dest-midi", vp::TrackKind_MIDI).status == vp::AckStatus_APPLIED, "add-track-midi"))
        return 2;

    auto* rec = s.recording();
    if (! check (rec != nullptr, "recording-manager-exists"))
        return 3;

    // Hosted wave input: stereo pairs the two channels onto one destination;
    // mono leaves them as two destinations for the multitrack take.
    auto& dm = s.engine().getDeviceManager();
    auto waveIns = dm.getWaveInputDevices();
    if (! check (! waveIns.empty(), "hosted-wave-input-present"))
        return 4;
    for (auto* d : waveIns)
        if (auto* wid = dynamic_cast<te::WaveInputDevice*> (d))
            std::printf ("wavein: %s ch=%d\n", wid->getName().toRawUTF8(),
                         (int) wid->getChannels().getNumChannels());
    if (auto* wid = dynamic_cast<te::WaveInputDevice*> (waveIns[0]))
        wid->setChannelConfiguration (stereo ? te::ChannelConfiguration::stereo (0)
                                             : te::ChannelConfiguration::mono (0));

    rec->setCountIn (te::Edit::CountIn::none);
    rec->setMetronome (true, -12.0f, true); // click on during recording only
    rec->setPunchInOut (false);

    auto* ta = mustTrack (s, "dest-a");
    auto* tb = mustTrack (s, "dest-b");
    if (! check (rec->arm (*ta, "wave:0", false, RecordingManager::Monitor::automatic).wasOk(),
                 "arm-a"))
        return 5;
    if (! stereo)
    {
        if (! check (rec->arm (*tb, "wave:1", false, RecordingManager::Monitor::off).wasOk(),
                     "arm-b-multitrack"))
            return 5;
    }
    if (midi)
    {
        auto* tm = mustTrack (s, "dest-midi");
        if (! check (rec->arm (*tm, "", true, RecordingManager::Monitor::automatic).wasOk(),
                     "arm-midi"))
            return 5;
    }
    check ((int) rec->armedTrackItemIds().size() >= (stereo ? 1 : 2), "armed-count");

    if (! check (rec->startRecording().wasOk(), "start-recording"))
        return 6;
    if (! check (rec->isRecording(), "is-recording"))
        return 6;

    // Pump ~0.7s of audio + a scripted MIDI phrase.
    double phase = 0.0;
    const int totalBlocks = 66; // ~0.7s @ 512/48k
    bool punched = false;
    for (int blk = 0; blk < totalBlocks; ++blk)
    {
        juce::AudioBuffer<float> in (2, 512);
        fillSine (in, phase, 48000.0);
        juce::MidiBuffer mb;
        if (midi)
        {
            const int t0 = blk * 512;
            auto at = [&] (int frame, const juce::MidiMessage& m) {
                const int off = frame - t0;
                if (off >= 0 && off < 512)
                    mb.addEvent (m, off);
            };
            at (3 * 512 + 64,  juce::MidiMessage::noteOn (1, 60, (juce::uint8) 100));
            at (5 * 512 + 32,  juce::MidiMessage::controllerEvent (1, 64, 127)); // sustain down
            at (7 * 512,       juce::MidiMessage::pitchWheel (1, 8192 + 1000));  // bend up
            at (9 * 512,       juce::MidiMessage::aftertouchChange (1, 60, 88)); // poly AT
            at (10 * 512,      juce::MidiMessage::channelPressureChange (1, 55));// channel AT
            at (12 * 512,      juce::MidiMessage::controllerEvent (1, 64, 0));   // sustain up
            at (14 * 512,      juce::MidiMessage::noteOff (1, 60, (juce::uint8) 40));
        }
        player.process (in, mb);

        if (punch && ! punched && blk == 30)
        {
            punched = true;
            check (rec->punchIn (*tb).wasOk(), "punch-in-b");
        }
        if (killBlocks > 0 && blk == killBlocks)
        {
            std::fprintf (stderr, "[fixture] simulating crash mid-take at block %d\n", blk);
            std::fflush (stdout);
            std::abort(); // real abnormal exit — journal stays "recording"
        }
    }

    if (! check (rec->stopRecording (false).wasOk(), "stop-recording"))
        return 7;

    s.edit()->dispatchPendingUpdatesSynchronously();

    // Verify clips landed on dest tracks.
    const auto clipsA = ta->getClips().size();
    const auto clipsB = tb->getClips().size();
    check (clipsA == 1, "clip-on-a");
    check (clipsB == (stereo ? 0 : 1), "clip-on-b");
    int midiNotes = -1;
    if (midi)
    {
        auto* tm = mustTrack (s, "dest-midi");
        midiNotes = tm->getClips().isEmpty() ? 0
                  : dynamic_cast<te::MidiClip*> (tm->getClips()[0])->getSequence().getNumNotes();
        check (midiNotes == 1, "midi-clip-one-note");
    }

    // Journal must be finalized with chunk entries + sha256.
    const auto* j = rec->currentTake();
    check (j != nullptr && j->status == TakeStatus::finalized, "journal-finalized");
    check (j != nullptr && ! j->chunks.empty(), "journal-chunks");

    std::printf ("{\"takeId\":\"%s\",\"status\":\"%s\",\"chunks\":%d,\"clips\":%d,"
                 "\"midiNotes\":%d,\"stereo\":%s,\"recordings\":%s}\n",
                 j ? j->takeId.toRawUTF8() : "", j ? takeStatusString (j->status).toRawUTF8() : "",
                 j ? (int) j->chunks.size() : 0, j ? j->clipIds.size() : 0,
                 midiNotes, stereo ? "true" : "false",
                 recordingsJson (container).toRawUTF8());
    std::printf ("PASS take\n");
    return 0;
}

int cmdRecover (const juce::File& container)
{
    EngineSession s (42);
    auto r = openProject (s, container);
    std::printf ("open-status=%d err=%d rev=%lld\n", (int) r.status, (int) r.error,
                 (long long) r.revision);
    const auto rec = recordingsJson (container);
    std::printf ("recordings: %s\n", rec.toRawUTF8());
    // The crash-recovery contract: the orphaned take must be listed and
    // labeled `incomplete`, never mistaken for a finalized source.
    if (! check (rec.contains ("incomplete"), "recover:incomplete-labeled"))
        return 9;
    check (! rec.contains ("\"status\":\"recording\""), "recover:no-phantom-recording");
    std::printf ("%s recover\n", rec.contains ("incomplete") ? "PASS" : "FAIL");
    return rec.contains ("incomplete") ? 0 : 9;
}

int cmdLatency (int inLat, int outLat)
{
    EngineSession s (42);
    te::test_utilities::EnginePlayer player (s.engine(),
        hostedParams (1, 1, 48000.0, 512, inLat, outLat));
    auto& dm = s.engine().getDeviceManager();
    auto waveIns = dm.getWaveInputDevices();
    if (waveIns.empty())
    {
        std::fprintf (stderr, "FAIL: no wave input\n");
        return 4;
    }
    auto* wid = dynamic_cast<te::WaveInputDevice*> (waveIns[0]);
    const auto adj = wid != nullptr ? wid->getRecordAdjustment() : te::TimeDuration();
    const auto line = "{\"sampleRate\":" + juce::String (dm.getSampleRate())
        + ",\"block\":" + juce::String (dm.getBlockSize())
        + ",\"configuredInLatSamples\":" + juce::String (inLat)
        + ",\"configuredOutLatSamples\":" + juce::String (outLat)
        + ",\"recordAdjustmentSamples\":" + juce::String (te::toSamples (adj, dm.getSampleRate()))
        + ",\"roundTripMs\":" + juce::String ((inLat + outLat) * 1000.0 / dm.getSampleRate(), 3)
        + "}";
    std::printf ("%s\n", line.toRawUTF8());
    std::printf ("PASS latency\n");
    return 0;
}

int cmdErrors()
{
    int fails = 0;
    // No hosted device: no playback context -> explicit failure, never success.
    {
        EngineSession s (42);
        const auto tmp = juce::File::getSpecialLocation (
            juce::File::tempDirectory).getChildFile ("void-rec-errors-" + juce::String (juce::Random().nextInt()));
        tmp.createDirectory();
        gRev = 0;
        if (createProject (s, tmp).status != vp::AckStatus_APPLIED)
            { std::printf ("FAIL errors:setup\n"); return 2; }
        addTrack (s, "t1", vp::TrackKind_AUDIO);
        auto* t = mustTrack (s, "t1");
        if (! check (t != nullptr, "errors:setup-track-1"))
            return 2;
        auto r = s.recording()->arm (*t, "", false, RecordingManager::Monitor::automatic);
        fails += ! check (! r.wasOk(), "errors:arm-no-device");
        auto r2 = s.recording()->startRecording();
        fails += ! check (! r2.wasOk(), "errors:record-no-device");
        tmp.deleteRecursively();
    }
    // Bad track / wrong-phase calls produce explicit errors.
    {
        EngineSession s (42);
        te::test_utilities::EnginePlayer player (s.engine(), hostedParams (1, 2, 48000.0, 512));
        const auto tmp = juce::File::getSpecialLocation (
            juce::File::tempDirectory).getChildFile ("void-rec-errors2-" + juce::String (juce::Random().nextInt()));
        tmp.createDirectory();
        gRev = 0;
        createProject (s, tmp);
        addTrack (s, "t1", vp::TrackKind_AUDIO);
        auto* t = mustTrack (s, "t1");
        if (! check (t != nullptr, "errors:setup-track"))
            return 2;
        fails += ! check (! s.recording()->punchIn (*t).wasOk(), "errors:punch-not-recording");
        fails += ! check (! s.recording()->stopRecording (false).wasOk(), "errors:stop-not-recording");
        fails += ! check (! s.recording()->disarm (*t).wasOk(), "errors:disarm-not-armed");
        fails += ! check (! s.recording()->arm (*t, "track:nosuch", false,
                                               RecordingManager::Monitor::off).wasOk(),
                          "errors:bad-source-track");
        tmp.deleteRecursively();
    }
    std::printf ("%s errors\n", fails == 0 ? "PASS" : "FAIL");
    return fails == 0 ? 0 : 9;
}

int cmdPanic (const juce::File& container)
{
    EngineSession s (42);
    te::test_utilities::EnginePlayer player (s.engine(), hostedParams (1, 2, 48000.0, 512));
    if (createProject (s, container).status != vp::AckStatus_APPLIED)
        return 2;
    addTrack (s, "dest-midi", vp::TrackKind_MIDI);
    auto* tm = mustTrack (s, "dest-midi");
    auto* rec = s.recording();
    if (! check (rec->arm (*tm, "", true, RecordingManager::Monitor::automatic).wasOk(),
                 "panic:arm-midi"))
        return 5;
    if (! check (rec->startRecording().wasOk(), "panic:start"))
        return 6;

    // Held note + sustain, no noteOff — then PANIC.
    for (int blk = 0; blk < 20; ++blk)
    {
        juce::AudioBuffer<float> in (1, 512);
        in.clear();
        juce::MidiBuffer mb;
        if (blk == 2) mb.addEvent (juce::MidiMessage::noteOn (1, 64, (juce::uint8) 90), 10);
        if (blk == 3) mb.addEvent (juce::MidiMessage::controllerEvent (1, 64, 127), 10);
        player.process (in, mb);
    }
    rec->panic();
    check (! rec->isRecording(), "panic:stopped");
    const auto* j = rec->currentTake();
    check (j != nullptr && j->status == TakeStatus::finalized, "panic:journal-finalized");

    // Evidence: the midi events chunk must contain allNotesOff rows.
    const auto dir = TakeJournalFile::takeDir (container, j ? j->takeId : "");
    int noteOffs = 0, allOff = 0;
    for (auto& f : dir.findChildFiles (juce::File::findFiles, false, "midi-*.jsonl"))
    {
        const auto lines = juce::StringArray::fromLines (f.loadFileAsString());
        for (auto& ln : lines)
        {
            if (ln.contains ("allNotesOff")) ++allOff;
            if (ln.contains ("noteOff")) ++noteOffs;
        }
    }
    check (allOff >= 16, "panic:all-notes-off-logged");
    std::printf ("{\"takeId\":\"%s\",\"allNotesOff\":%d,\"noteOffs\":%d,"
                 "\"recordings\":%s}\n",
                 j ? j->takeId.toRawUTF8() : "", allOff, noteOffs,
                 recordingsJson (container).toRawUTF8());
    std::printf ("PASS panic\n");
    return 0;
}

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
    juce::StringArray args;
    for (int i = 1; i < argc; ++i)
        args.add (argv[i]);
    if (args.isEmpty())
    {
        std::fprintf (stderr, "usage: void-recording-fixture <take|recover|latency|errors|panic> ...\n");
        return 64;
    }

    const auto cmd = args[0];
    const auto container = juce::File (args.size() > 1 ? args[1] : "/tmp/void-rec");

    if (cmd == "take")
    {
        const bool stereo  = args.contains ("--stereo");
        const bool midi    = args.contains ("--midi");
        const bool punch   = args.contains ("--punch");
        int killBlocks = -1;
        const auto ki = args.indexOf ("--kill-after-blocks");
        if (ki >= 0 && ki + 1 < args.size())
            killBlocks = args[ki + 1].getIntValue();
        return cmdTake (container, stereo, midi, killBlocks, punch);
    }
    if (cmd == "recover")
        return cmdRecover (container);
    if (cmd == "latency")
        return cmdLatency (args.size() > 1 ? args[1].getIntValue() : 97,
                           args.size() > 2 ? args[2].getIntValue() : 131);
    if (cmd == "errors")
        return cmdErrors();
    if (cmd == "panic")
        return cmdPanic (container);

    std::fprintf (stderr, "unknown command: %s\n", cmd.toRawUTF8());
    return 64;
}
