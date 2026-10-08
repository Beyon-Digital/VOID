// void-engine worker entry point.
//
// Connects to the supervisor's Unix-domain sockets ($VOID_CONTROL_SOCK /
// $VOID_TELEMETRY_SOCK), presents WorkerHello with $VOID_WORKER_TOKEN, then
// serves framed ControlEnvelopes on the message thread and publishes
// ClockSnapshot/MeterFrame telemetry at <=30 Hz on a timer.
//
// Framing: u32 LE length prefix + flatbuffers payload, 1 MiB cap — byte
// parity with crates/void-worker/src/transport.rs.

#include <JuceHeader.h>
#include "bridge/UdsChannel.h"
#include "session/EngineSession.h"
#include "audio/DeviceBridge.h"
#include <flatbuffers/flatbuffers.h>
#include <chrono>
#include <unistd.h>

namespace vp = voidproto;
using voidengine::UdsChannel;

namespace
{

uint64_t hostClockNs()
{
    return (uint64_t) std::chrono::duration_cast<std::chrono::nanoseconds> (
               std::chrono::steady_clock::now().time_since_epoch()).count();
}

juce::String payloadHash (const std::vector<uint8_t>& frame)
{
    // Identity hash over the raw request frame for COMMAND_ID_REUSE
    // (CONTRACTS.md §3): same command_id + different bytes => reuse error.
    return juce::SHA256 (frame.data(), frame.size()).toHexString();
}

class EngineWorker final : public juce::Timer
{
public:
    EngineWorker (UdsChannel control, UdsChannel telemetry, const juce::String& token)
        : session_ (1), control_ (std::move (control)), telemetry_ (std::move (telemetry))
    {
        auto& dm = session_.engine().getDeviceManager();
        dm.initialise (0, 2);
        if (auto* dev = dm.deviceManager.getCurrentAudioDevice())
            juce::Logger::writeToLog (juce::String ("[void-engine] audio device: ")
                                      + dev->getName() + " @"
                                      + juce::String (dev->getCurrentSampleRate()) + "Hz");
        else
            juce::Logger::writeToLog ("[void-engine] no audio device — offline render still available");
        dm.deviceManager.addAudioCallback (&probe_);

        const auto* envId = std::getenv ("VOID_WORKER_INSTANCE_ID");
        instanceId_ = envId != nullptr ? juce::String (envId)
                                       : "engine-" + juce::String ((int64_t) ::getpid());
        sendHello (token);
        if (telemetry_.isOpen())
            telemetry_.setSendNonBlocking (true); // lossy by contract
        startTimerHz (30);
    }

    ~EngineWorker() override { stopTimer(); }

    // Blocking reader — run on its own thread.
    void runReader()
    {
        std::vector<uint8_t> frame;
        while (control_.recvFrame (frame))
        {
            auto copy = std::make_shared<std::vector<uint8_t>> (std::move (frame));
            frame.clear();
            juce::MessageManager::callAsync ([this, copy] { dispatch (*copy); });
        }
        juce::Logger::writeToLog ("[void-engine] control socket closed; exiting");
        juce::MessageManager::callAsync ([] { juce::JUCEApplicationBase::quit(); });
    }

private:
    void sendHello (const juce::String& token)
    {
        flatbuffers::FlatBufferBuilder fbb;
        std::vector<flatbuffers::Offset<flatbuffers::String>> capsVec;
        for (const char* c : { "engine.tracktion.v1", "telemetry.clock", "telemetry.meters",
                              "persistence.checkpoint.v1", "transport.live", "plugin.external.v1" })
            capsVec.push_back (fbb.CreateString (c));
        auto caps = fbb.CreateVector (capsVec);
        auto hello = vp::CreateWorkerHello (fbb, vp::WorkerKind_ENGINE,
                                            fbb.CreateString (instanceId_.toRawUTF8()),
                                            1, 0,
                                            fbb.CreateString (token.toRawUTF8()),
                                            session_.epoch(), caps);
        fbb.Finish (vp::CreateControlEnvelope (fbb, vp::ControlFrame_WorkerHello,
                                               hello.Union()));
        if (! control_.sendFrame (fbb.GetBufferPointer(), fbb.GetSize()))
            juce::Logger::writeToLog ("[void-engine] WorkerHello send failed");
    }

    void dispatch (const std::vector<uint8_t>& bytes)
    {
        flatbuffers::Verifier v (bytes.data(), bytes.size());
        if (! v.VerifyBuffer<vp::ControlEnvelope>())
            return;

        const auto* env = flatbuffers::GetRoot<vp::ControlEnvelope> (bytes.data());
        if (env == nullptr || env->frame_type() != vp::ControlFrame_RequestFrame)
            return;

        const auto* rf = env->frame_as_RequestFrame();
        switch (rf->request_type())
        {
            case vp::ControlRequest_PersistentCommand:
            {
                const auto* cmd = rf->request_as_PersistentCommand();
                const auto hash = payloadHash (bytes);
                auto result = session_.applyPersistentCommand (*cmd, hash);
                sendReceipt (*cmd, result, hash);
                break;
            }
            case vp::ControlRequest_TransportRequest:
            {
                const auto* tr = rf->request_as_TransportRequest();
                const bool hadEdit = session_.edit() != nullptr;
                session_.applyTransportRequest (*tr);
                sendTransportAck (*tr, hadEdit);
                break;
            }
            case vp::ControlRequest_ReadRequest:
                sendReadResponse (*rf->request_as_ReadRequest());
                break;
            case vp::ControlRequest_PluginScanRequest:
                sendScanRejected (*rf->request_as_PluginScanRequest());
                break;
            default: break;
        }
    }

    void sendReceipt (const vp::PersistentCommand& cmd,
                      const voidengine::CommandResult& r,
                      const juce::String& hash)
    {
        flatbuffers::FlatBufferBuilder fbb;
        auto rec = vp::CreateCommandReceipt (
            fbb,
            fbb.CreateString (cmd.command_id() ? cmd.command_id()->c_str() : ""),
            fbb.CreateString (cmd.transaction_id() ? cmd.transaction_id()->c_str() : ""),
            r.status, r.error, r.revision, session_.epoch(),
            fbb.CreateString (r.message.toRawUTF8()),
            fbb.CreateString (hash.toRawUTF8()));
        auto ev = vp::CreateEventFrame (fbb, vp::ControlEvent_CommandReceipt, rec.Union());
        fbb.Finish (vp::CreateControlEnvelope (fbb, vp::ControlFrame_EventFrame, ev.Union()));
        control_.sendFrame (fbb.GetBufferPointer(), fbb.GetSize());
    }

    void sendTransportAck (const vp::TransportRequest& tr, bool ok)
    {
        if (! telemetry_.isOpen())
            return;
        flatbuffers::FlatBufferBuilder fbb;
        auto ack = vp::CreateTransportAck (
            fbb, fbb.CreateString (tr.request_id() ? tr.request_id()->c_str() : ""),
            ok, ok ? vp::ErrorCode_NONE : vp::ErrorCode_NOT_FOUND);
        fbb.Finish (vp::CreateTelemetryFrame (fbb, vp::TelemetryEvent_TransportAck,
                                            ack.Union()));
        telemetry_.sendFrame (fbb.GetBufferPointer(), fbb.GetSize());
    }

    void sendReadResponse (const vp::ReadRequest& req)
    {
        std::vector<std::pair<juce::String, juce::String>> items;
        juce::String nextCursor;
        bool done = true;
        vp::ErrorCode err = vp::ErrorCode_NONE;
        session_.handleRead (req, items, nextCursor, done, err);

        flatbuffers::FlatBufferBuilder fbb;
        std::vector<flatbuffers::Offset<vp::ReadItem>> rows;
        rows.reserve (items.size());
        for (auto& [oid, json] : items)
            rows.push_back (vp::CreateReadItem (fbb, fbb.CreateString (oid.toRawUTF8()),
                                                fbb.CreateString (json.toRawUTF8())));
        auto resp = vp::CreateReadResponse (
            fbb, fbb.CreateString (req.request_id() ? req.request_id()->c_str() : ""),
            session_.revision(), fbb.CreateVector (rows),
            fbb.CreateString (nextCursor.toRawUTF8()), done, err);
        auto ev = vp::CreateEventFrame (fbb, vp::ControlEvent_ReadResponse, resp.Union());
        fbb.Finish (vp::CreateControlEnvelope (fbb, vp::ControlFrame_EventFrame, ev.Union()));
        control_.sendFrame (fbb.GetBufferPointer(), fbb.GetSize());
    }

    void sendScanRejected (const vp::PluginScanRequest& req)
    {
        flatbuffers::FlatBufferBuilder fbb;
        auto res = vp::CreatePluginScanResult (
            fbb, fbb.CreateString (req.request_id() ? req.request_id()->c_str() : ""),
            false, false, 0, 0, 0, 0, 0,
            vp::ErrorCode_UNSUPPORTED_CAPABILITY,
            fbb.CreateString ("plugin scans are served by void-plugin-scanner"));
        auto ev = vp::CreateEventFrame (fbb, vp::ControlEvent_PluginScanResult, res.Union());
        fbb.Finish (vp::CreateControlEnvelope (fbb, vp::ControlFrame_EventFrame, ev.Union()));
        control_.sendFrame (fbb.GetBufferPointer(), fbb.GetSize());
    }

    void timerCallback() override
    {
        const auto* edit = session_.edit();
        const auto* tc = edit != nullptr ? &edit->getTransport() : nullptr;
        const double sr = probe_.sampleRate.load();

        vp::TransportState state = vp::TransportState_STOPPED;
        int64_t timelineSample = 0, loopStart = 0, loopEnd = 0;
        if (tc != nullptr)
        {
            if (tc->isPlaying()) state = vp::TransportState_PLAYING;
            timelineSample = (int64_t) std::llround (tc->getPosition().inSeconds() * sr);
            if (edit != nullptr && tc->looping)
            {
                const auto& tseq = edit->tempoSequence;
                const auto loop = tc->getLoopRange();
                loopStart = (int64_t) std::llround (
                    te::toBeats (loop.getStart(), tseq).inBeats() * voidengine::kTicksPerQuarter);
                loopEnd = (int64_t) std::llround (
                    te::toBeats (loop.getEnd(), tseq).inBeats() * voidengine::kTicksPerQuarter);
            }
        }

        if (telemetry_.isOpen())
        {
            flatbuffers::FlatBufferBuilder fbb;
            auto snap = vp::CreateClockSnapshot (
                fbb, fbb.CreateString (session_.projectId().toRawUTF8()), session_.epoch(),
                timelineSample, (int64_t) probe_.deviceSamples.load(),
                (uint32_t) sr, state, loopStart, loopEnd,
                session_.revision(), clockSeq_++, hostClockNs());
            fbb.Finish (vp::CreateTelemetryFrame (fbb, vp::TelemetryEvent_ClockSnapshot,
                                                snap.Union()));
            telemetry_.sendFrame (fbb.GetBufferPointer(), fbb.GetSize());

            if (edit != nullptr)
                sendMeters (const_cast<te::Edit*> (session_.edit()));
        }
    }

    void sendMeters (te::Edit* edit)
    {
        for (auto* t : te::getAudioTracks (*edit))
        {
            const auto trackId = t->state.getProperty ("voidId").toString();
            if (trackId.isEmpty())
                continue;
            auto* client = session_.meterClientFor (trackId);
            if (client == nullptr)
                continue;
            const auto l = client->getAndClearAudioLevel (0);
            const auto r = client->getAndClearAudioLevel (
                juce::jmax (0, client->getNumChannelsUsed() - 1));
            const bool clipped = client->getAndClearOverload();

            flatbuffers::FlatBufferBuilder mb;
            auto m = vp::CreateMeterFrame (
                mb, mb.CreateString (session_.projectId().toRawUTF8()), session_.epoch(),
                mb.CreateString (trackId.toRawUTF8()),
                juce::Decibels::decibelsToGain (l.dB, -100.0f),
                juce::Decibels::decibelsToGain (r.dB, -100.0f),
                juce::Decibels::decibelsToGain (l.dB, -100.0f),
                juce::Decibels::decibelsToGain (r.dB, -100.0f),
                clipped, meterSeq_++);
            mb.Finish (vp::CreateTelemetryFrame (mb, vp::TelemetryEvent_MeterFrame,
                                               m.Union()));
            telemetry_.sendFrame (mb.GetBufferPointer(), mb.GetSize());
        }
    }

    voidengine::EngineSession session_;
    voidengine::RtProbe probe_;
    UdsChannel control_;
    UdsChannel telemetry_;
    juce::String deviceErr_;
    juce::String instanceId_;
    uint64_t clockSeq_ = 0, meterSeq_ = 0;
};

} // namespace

int main (int argc, char* argv[])
{
    juce::ignoreUnused (argc, argv);
    juce::ScopedJuceInitialiser_GUI gui;

    const auto* controlPath   = std::getenv ("VOID_CONTROL_SOCK");
    const auto* telemetryPath = std::getenv ("VOID_TELEMETRY_SOCK");
    const auto* token         = std::getenv ("VOID_WORKER_TOKEN");

    if (controlPath == nullptr || token == nullptr)
    {
        std::fprintf (stderr, "void-engine: missing VOID_CONTROL_SOCK / VOID_WORKER_TOKEN\n");
        return 2;
    }

    UdsChannel control, telemetry;
    if (! control.connectTo (controlPath))
    {
        std::fprintf (stderr, "void-engine: cannot connect %s\n", controlPath);
        return 3;
    }
    if (telemetryPath != nullptr)
        telemetry.connectTo (telemetryPath); // optional — telemetry loss is non-fatal

    std::fprintf (stderr, "void-engine: connected to supervisor\n");

    EngineWorker worker (std::move (control), std::move (telemetry),
                         juce::String (token));
    std::thread reader ([&worker] { worker.runReader(); });

    juce::MessageManager::getInstance()->runDispatchLoop();

    if (reader.joinable())
        reader.detach();
    return 0;
}
