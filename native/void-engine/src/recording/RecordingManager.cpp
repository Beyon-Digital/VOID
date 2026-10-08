#include "RecordingManager.h"
#include "../session/EngineSession.h"

namespace voidengine
{

namespace
{
constexpr juce::int64 kMinFreeBytes = 64LL * 1024 * 1024; // 64 MiB headroom

juce::String safeName (const juce::String& s)
{
    return juce::File::createLegalFileName (s.isEmpty() ? "take" : s);
}
} // namespace

//==============================================================================
juce::File VoidEngineBehaviour::getFileForNewAudioRecording (te::Track& t,
                                                             const juce::String& ext)
{
    if (takeFileProvider)
        if (auto f = takeFileProvider (t); f != juce::File())
            return f;
    return te::EngineBehaviour::getFileForNewAudioRecording (t, ext);
}

//==============================================================================
RecordingManager::RecordingManager (EngineSession& s) : session_ (s)
{
    auto& dm = s.engine().getDeviceManager();
    dm.addChangeListener (this);
    if (auto* edit = s.edit())
        edit->getTransport().addListener (this);
}

RecordingManager::~RecordingManager()
{
    shutdown();
}

void RecordingManager::shutdown()
{
    if (shuttingDown_)
        return;
    shuttingDown_ = true;
    auto& dm = session_.engine().getDeviceManager();
    dm.removeChangeListener (this);
    if (auto* edit = session_.edit())
    {
        edit->getTransport().removeListener (this);
        if (edit->getTransport().isRecording())
            edit->getTransport().stopRecording (true);
    }
    for (auto& a : armed_)
        if (a->capture != nullptr)
            a->capture->end();
    armed_.clear();
    if (journal_ && journal_->status == TakeStatus::recording && takeDir_ != juce::File())
    {
        journal_->status = TakeStatus::incomplete;
        journal_->endedUtc = juce::Time::getCurrentTime().toISO8601 (true);
        TakeJournalFile::write (takeDir_, *journal_);
    }
}

bool RecordingManager::isRecording() const noexcept
{
    const auto* edit = session_.edit();
    return edit != nullptr && edit->getTransport().isRecording();
}

te::EditPlaybackContext* RecordingManager::contextOrNull() const
{
    auto* edit = session_.edit();
    return edit != nullptr ? edit->getCurrentPlaybackContext() : nullptr;
}

juce::File RecordingManager::container() const
{
    return juce::File (session_.containerDirPath());
}

juce::Result RecordingManager::fail (const juce::String& msg, bool stopTake)
{
    lastError_ = msg;
    phase_ = Phase::failed;
    if (stopTake && isRecording())
        session_.edit()->getTransport().stop (false, false);
    if (journal_ && journal_->status == TakeStatus::recording)
    {
        journal_->status = TakeStatus::incomplete;
        journal_->error = msg;
        journal_->endedUtc = juce::Time::getCurrentTime().toISO8601 (true);
        if (takeDir_ != juce::File())
            TakeJournalFile::write (takeDir_, *journal_);
    }
    return juce::Result::fail (msg);
}

//==============================================================================
RecordingManager::ArmedTarget* RecordingManager::armedFor (te::AudioTrack& t) const
{
    for (auto& a : armed_)
        if (a->track == &t)
            return a.get();
    return nullptr;
}

RecordingManager::ArmedTarget* RecordingManager::armedForItemId (te::EditItemID id) const
{
    for (auto& a : armed_)
        if (a->track != nullptr && a->track->itemID == id)
            return a.get();
    return nullptr;
}

juce::Array<juce::String> RecordingManager::armedTrackItemIds() const
{
    juce::Array<juce::String> out;
    for (auto& a : armed_)
        if (a->destination != nullptr && a->destination->recordEnabled.get())
            out.add (a->track->state.getProperty ("voidId").toString());
    return out;
}

//==============================================================================
juce::Result RecordingManager::arm (te::AudioTrack& dest, const juce::String& inputName,
                                    bool midi, Monitor monitor)
{
    if (auto* e = armedFor (dest); e != nullptr && e->destination != nullptr
        && e->destination->recordEnabled.get())
        return juce::Result::ok();

    if (auto* edit = session_.edit())
        edit->getTransport().ensureContextAllocated();
    auto* ctx = contextOrNull();
    if (ctx == nullptr)
        return fail ("no playback context (no audio device)", false);

    auto* edit = session_.edit();
    te::InputDeviceInstance::Destination* dst = nullptr;
    te::InputDeviceInstance* inst = nullptr;
    bool isMidi = midi;

    if (inputName.startsWith ("track:"))
    {
        // Track device: record another track's output — no hardware needed.
        const auto srcId = inputName.fromFirstOccurrenceOf ("track:", false, false);
        auto* src = session_.findTrackById (srcId);
        if (src == nullptr)
            return fail ("source track not found: " + srcId, false);
        dst = te::assignTrackAsInput (dest, *src,
                                      isMidi ? te::InputDevice::trackMidiDevice
                                             : te::InputDevice::trackWaveDevice);
        if (dst == nullptr)
            return fail ("assignTrackAsInput failed", false);
        inst = &dst->input;
    }
    else
    {
        // "wave:N"/"midi:N" -> the Nth matching device (multitrack takes
        // from per-channel inputs); "<name>" -> first device whose name
        // contains it; "" -> first device.
        int devIndex = -1;
        juce::String match = inputName;
        if (match.contains (":"))
        {
            const auto tail = match.fromLastOccurrenceOf (":", false, false);
            if (tail.containsOnly ("0123456789") && tail.isNotEmpty())
            {
                devIndex = tail.getIntValue();
                match = match.upToLastOccurrenceOf (":", false, false);
            }
        }
        auto inputs = ctx->getAllInputs();
        int seen = 0;
        for (auto* i : inputs)
        {
            auto& d = i->getInputDevice();
            const bool dMidi = d.isMidi();
            if (dMidi != isMidi)
                continue;
            if (d.isTrackDevice())
                continue; // track devices only via explicit "track:<id>"
            const bool matches = match.isEmpty()
                || d.getName().containsIgnoreCase (match)
                || (match == "wave" && ! dMidi)
                || (match == "midi" && dMidi);
            if (! matches)
                continue;
            if (devIndex >= 0 && seen++ < devIndex)
                continue;
            auto res = i->setTarget (dest.itemID, true, nullptr);
            if (! res)
                return fail ("setTarget: " + res.error(), false);
            dst = *res;
            inst = i;
            break;
        }
        if (dst == nullptr)
            return fail (juce::String (isMidi ? "no MIDI input device available"
                                             : "no wave input device available"),
                         false);
    }

    dst->recordEnabled = true;
    inst->getInputDevice().setMonitorMode (
        monitor == Monitor::off ? te::InputDevice::MonitorMode::off :
        monitor == Monitor::on  ? te::InputDevice::MonitorMode::on
                                : te::InputDevice::MonitorMode::automatic);
    edit->dispatchPendingUpdatesSynchronously();

    auto at = std::make_unique<ArmedTarget>();
    at->track = &dest;
    at->instance = inst;
    at->destination = dst;
    at->midi = isMidi;
    armed_.push_back (std::move (at));
    if (phase_ != Phase::recording)
        phase_ = Phase::armed;
    return juce::Result::ok();
}

juce::Result RecordingManager::disarm (te::AudioTrack& dest)
{
    auto* e = armedFor (dest);
    if (e == nullptr)
        return juce::Result::fail ("track not armed");
    if (e->destination != nullptr)
        e->destination->recordEnabled = false;
    if (e->capture != nullptr)
        e->capture->end();
    for (auto it = armed_.begin(); it != armed_.end(); ++it)
        if (it->get() == e) { armed_.erase (it); break; }
    if (armed_.empty() && phase_ == Phase::armed)
        phase_ = Phase::idle;
    return juce::Result::ok();
}

juce::Result RecordingManager::setMonitor (te::AudioTrack& dest, Monitor m)
{
    auto* e = armedFor (dest);
    if (e == nullptr || e->instance == nullptr)
        return juce::Result::fail ("track not armed");
    e->instance->getInputDevice().setMonitorMode (
        m == Monitor::off ? te::InputDevice::MonitorMode::off :
        m == Monitor::on  ? te::InputDevice::MonitorMode::on
                          : te::InputDevice::MonitorMode::automatic);
    return juce::Result::ok();
}

void RecordingManager::setCountIn (te::Edit::CountIn mode)
{
    if (auto* edit = session_.edit())
        edit->setCountInMode (mode);
}

void RecordingManager::setMetronome (bool enabled, float gain, bool recordingOnly)
{
    if (auto* edit = session_.edit())
    {
        edit->clickTrackEnabled = enabled;
        edit->clickTrackGain = gain;
        edit->clickTrackRecordingOnly = recordingOnly;
    }
}

void RecordingManager::setPunchInOut (bool enabled)
{
    if (auto* edit = session_.edit())
        edit->recordingPunchInOut = enabled;
}

//==============================================================================
juce::Result RecordingManager::startRecording()
{
    if (auto* edit = session_.edit(); edit == nullptr)
        return juce::Result::fail ("no open project");
    if (isRecording())
        return juce::Result::fail ("already recording");

    auto* ctx = contextOrNull();
    if (ctx == nullptr)
        return fail ("no playback context (no audio device)", false);

    const auto armedIds = armedTrackItemIds();
    if (armedIds.isEmpty())
        return fail ("no armed tracks", false);

    // Disk headroom: never begin a take that cannot write.
    const auto dir = container();
    const auto free_ = dir.getBytesFreeOnVolume();
    if (free_ >= 0 && free_ < kMinFreeBytes)
        return fail ("insufficient disk space for take", false);

    // New take journal.
    TakeJournal j;
    j.takeId = juce::Uuid().toString().replace ("-", "").substring (0, 15);
    j.projectId = session_.projectId();
    j.startedUtc = juce::Time::getCurrentTime().toISO8601 (true);
    j.sampleRate = session_.engine().getDeviceManager().getSampleRate();
    juce::StringArray trackIdList;
    for (auto& i : armedIds) trackIdList.add (i);
    j.trackIds = trackIdList.joinIntoString (",");
    j.inputSummary = {};
    for (auto& a : armed_)
        if (a->destination != nullptr && a->destination->recordEnabled.get())
            j.inputSummary += a->track->state.getProperty ("voidId").toString()
                              + "<-" + a->instance->getInputDevice().getName() + " ";

    takeDir_ = TakeJournalFile::takeDir (dir, j.takeId);
    journal_ = j;
    if (! TakeJournalFile::begin (dir, *journal_))
        return fail ("cannot create take journal", false);

    // Route TE recording files into recordings/<takeId>/.
    if (auto* beh = dynamic_cast<VoidEngineBehaviour*> (&session_.engine().getEngineBehaviour()))
        beh->takeFileProvider = [this] (te::Track& t) -> juce::File
        {
            const auto name = safeName (t.state.getProperty ("voidId").toString());
            return takeDir_.getChildFile ("chunk-" + name + "-1.wav");
        };

    // Start MIDI captures for armed midi targets.
    for (auto& a : armed_)
        if (a->midi && a->instance != nullptr
            && a->destination != nullptr && a->destination->recordEnabled.get())
        {
            a->capture = std::make_unique<MidiCapture>();
            const auto tid = a->track->state.getProperty ("voidId").toString();
            a->capture->begin (*a->instance, takeDir_, tid);
        }

    phase_ = Phase::recording;
    auto& tc = session_.edit()->getTransport();
    tc.ensureContextAllocated();
    tc.record (false);

    if (! tc.isRecording())
    {
        if (auto* beh = dynamic_cast<VoidEngineBehaviour*> (&session_.engine().getEngineBehaviour()))
            beh->takeFileProvider = {};
        return fail ("engine refused to start recording (no inputs available?)");
    }

    startTimerHz (1); // journal chunk updates, message thread only
    return juce::Result::ok();
}

juce::Result RecordingManager::stopRecording (bool discard)
{
    auto* edit = session_.edit();
    if (edit == nullptr)
        return juce::Result::fail ("no open project");
    if (! isRecording())
        return juce::Result::fail ("not recording");
    phase_ = Phase::stopping;
    edit->getTransport().stop (discard, false);
    return juce::Result::ok();
}

juce::Result RecordingManager::punchIn (te::AudioTrack& dest)
{
    auto* e = armedFor (dest);
    if (e == nullptr || e->instance == nullptr)
        return juce::Result::fail ("track not armed");
    if (! isRecording())
        return juce::Result::fail ("not recording");
    e->instance->setRecordingEnabled (dest.itemID, true);
    return juce::Result::ok();
}

juce::Result RecordingManager::punchOut (te::AudioTrack& dest)
{
    auto* e = armedFor (dest);
    if (e == nullptr || e->instance == nullptr)
        return juce::Result::fail ("track not armed");
    if (! isRecording())
        return juce::Result::fail ("not recording");
    e->instance->setRecordingEnabled (dest.itemID, false);
    return juce::Result::ok();
}

//==============================================================================
void RecordingManager::panic()
{
    // All-notes-off on every MIDI input's held keys + every enabled output.
    auto& dm = session_.engine().getDeviceManager();
    for (auto& a : armed_)
        if (a->capture != nullptr)
            if (auto* mid = dynamic_cast<te::MidiInputDevice*> (&a->instance->getInputDevice()))
                a->capture->emitAllNotesOff (*mid);
    for (int i = 0; i < dm.getNumMidiOutDevices(); ++i)
        if (auto* dev = dm.getMidiOutDevice (i); dev != nullptr && dev->isEnabled())
            dev->sendNoteOffMessages();

    auto* edit = session_.edit();
    if (edit != nullptr && isRecording())
        edit->getTransport().stop (false, false); // keep the partial take
    else if (edit != nullptr)
        edit->getTransport().stop (true, false);
}

void RecordingManager::transportStopped()
{
    // Covers plain STOP while recording: TE finalizes clips itself; our
    // listener path closes the journal in recordingStopped.
}

void RecordingManager::deviceListChanged()
{
    if (! isRecording())
        return;
    auto& dm = session_.engine().getDeviceManager();
    for (auto& a : armed_)
    {
        if (a->instance == nullptr)
            continue;
        auto& d = a->instance->getInputDevice();
        if (d.isTrackDevice())
            continue;
        bool alive = true;
        if (auto* mid = dynamic_cast<te::MidiInputDevice*> (&d))
            alive = mid->isEnabled();
        else if (auto* wid = dynamic_cast<te::WaveInputDevice*> (&d))
            alive = wid->isEnabled() && dm.deviceManager.getCurrentAudioDevice() != nullptr;
        if (! alive)
        {
            fail ("input device lost during take: " + d.getName());
            return;
        }
    }
}

//==============================================================================
void RecordingManager::recordingStarted (te::SyncPoint, std::optional<te::TimeRange>)
{
    phase_ = Phase::recording;
}

void RecordingManager::recordingStopped (te::SyncPoint, bool discard)
{
    stopTimer();
    // Flush MIDI captures before reading their stats so the journal sees
    // the true byte/event counts (off-callback, message thread).
    for (auto& a : armed_)
        if (a->capture != nullptr)
            a->capture->end();
    if (journal_ && journal_->status == TakeStatus::recording)
    {
        scanTakeDirChunks();
        updateJournalChunks();
        journal_->status = discard ? TakeStatus::discarded : TakeStatus::finalized;
        journal_->endedUtc = juce::Time::getCurrentTime().toISO8601 (true);
        // Second pass seals finalized chunks with their sha256 fingerprints.
        if (! discard)
            scanTakeDirChunks();
        if (takeDir_ != juce::File())
            TakeJournalFile::write (takeDir_, *journal_);
    }
    if (auto* beh = dynamic_cast<VoidEngineBehaviour*> (&session_.engine().getEngineBehaviour()))
        beh->takeFileProvider = {};
    phase_ = discard ? Phase::idle : (armed_.empty() ? Phase::idle : Phase::armed);
}

void RecordingManager::recordingFinished (te::InputDeviceInstance& inst,
                                          te::EditItemID targetID,
                                          const juce::ReferenceCountedArray<te::Clip>& clips)
{
    if (! journal_)
        return;
    juce::String trackId;
    for (auto& a : armed_)
        if (a->track != nullptr && a->track->itemID == targetID)
            trackId = a->track->state.getProperty ("voidId").toString();
    int n = 0;
    for (auto* c : clips)
    {
        // Stamp recorded clips with VOID ids so reads/recovery can name them.
        auto cid = c->state.getProperty ("voidId").toString();
        if (cid.isEmpty())
        {
            cid = "rec-" + journal_->takeId + "-" + (trackId.isEmpty() ? inst.getInputDevice().getName() : trackId)
                  + "-" + juce::String (++n);
            c->state.setProperty ("voidId", cid, nullptr);
        }
        if (! journal_->clipIds.contains (cid))
            journal_->clipIds.add (cid);

        // Authoritative chunk entry straight from the finished clip — the
        // per-instance recording contexts are already torn down at this
        // point, so getRecordingFile() is no longer reliable.
        if (auto* wac = dynamic_cast<te::WaveAudioClip*> (c))
        {
            const auto f = wac->getAudioFile().getFile();
            if (f.existsAsFile())
            {
                TakeChunk chunk;
                chunk.path = f.getRelativePathFrom (takeDir_);
                chunk.kind = "audio";
                chunk.bytesObserved = f.getSize();
                if (auto* wid = dynamic_cast<te::WaveInputDevice*> (&inst.getInputDevice()))
                {
                    const int bpf = (wid->getBitDepth() / 8) * juce::jmax (1, (int) wid->getChannels().getNumChannels());
                    chunk.channels = (int) wid->getChannels().getNumChannels();
                    chunk.framesObserved = bpf > 0 ? (chunk.bytesObserved - 128) / bpf : 0;
                }
                if (journal_->status == TakeStatus::finalized)
                    chunk.sha256 = juce::SHA256 (f).toHexString();
                TakeJournalFile::recordChunk (takeDir_, *journal_, chunk);
            }
        }
    }
    if (takeDir_ != juce::File())
        TakeJournalFile::write (takeDir_, *journal_);
}

void RecordingManager::updateJournalChunks()
{
    if (! journal_ || takeDir_ == juce::File())
        return;
    for (auto& a : armed_)
    {
        if (a->instance == nullptr)
            continue;
        const auto f = a->instance->getRecordingFile (a->track->itemID);
        if (f == juce::File() || ! f.existsAsFile())
            continue;
        TakeChunk c;
        c.path = f.getRelativePathFrom (takeDir_);
        c.kind = "audio";
        auto* wid = dynamic_cast<te::WaveInputDevice*> (&a->instance->getInputDevice());
        const int bytesPerFrame = wid != nullptr ? (wid->getBitDepth() / 8) * juce::jmax (1, (int) wid->getChannels().getNumChannels()) : 4;
        c.channels = wid != nullptr ? (int) wid->getChannels().getNumChannels() : 0;
        c.bytesObserved = f.getSize();
        c.framesObserved = bytesPerFrame > 0 ? (c.bytesObserved - 128) / bytesPerFrame : 0; // minus wav header est.
        if (journal_->status == TakeStatus::finalized)
            c.sha256 = juce::SHA256 (f).toHexString();
        TakeJournalFile::recordChunk (takeDir_, *journal_, c);
    }
    // MIDI event chunks
    for (auto& a : armed_)
    {
        if (! a->midi || a->capture == nullptr)
            continue;
        const auto f = takeDir_.getChildFile ("midi-" + a->track->state.getProperty ("voidId").toString() + ".jsonl");
        if (! f.existsAsFile())
            continue;
        TakeChunk c;
        c.path = f.getFileName();
        c.kind = "midi-events";
        c.bytesObserved = f.getSize();
        c.framesObserved = (juce::int64) a->capture->eventsCaptured();
        if (journal_->status == TakeStatus::finalized)
            c.sha256 = juce::SHA256 (f).toHexString();
        TakeJournalFile::recordChunk (takeDir_, *journal_, c);
    }
}

void RecordingManager::scanTakeDirChunks()
{
    // Definitive chunk inventory: every take file the provider wrote. No TE
    // dependency — context teardown makes getRecordingFile() unreliable here.
    if (! journal_ || takeDir_ == juce::File())
        return;
    TakeJournalFile::scanChunks (takeDir_, *journal_);
    // Live captures report truer event counts than a line scan mid-flush.
    for (auto& a : armed_)
        if (a->capture != nullptr && a->track != nullptr)
        {
            const auto name = "midi-" + a->track->state.getProperty ("voidId").toString() + ".jsonl";
            for (auto& e : journal_->chunks)
                if (e.path == name)
                {
                    e.framesObserved = (juce::int64) a->capture->eventsCaptured();
                    TakeJournalFile::recordChunk (takeDir_, *journal_, e);
                    break;
                }
        }
    if (journal_->status == TakeStatus::finalized)
        for (auto& e : journal_->chunks)
        {
            e.sha256 = juce::SHA256 (takeDir_.getChildFile (e.path)).toHexString();
            TakeJournalFile::recordChunk (takeDir_, *journal_, e);
        }
}

void RecordingManager::timerCallback()
{
    updateJournalChunks();
}

void RecordingManager::changeListenerCallback (juce::ChangeBroadcaster*)
{
    deviceListChanged();
}

void RecordingManager::recoverIncompleteTakes()
{
    auto recovered = TakeJournalFile::recoverIncomplete (container());
    for (auto& j : recovered)
        juce::Logger::writeToLog ("[void-engine] recovered incomplete take "
                                + j.takeId + " (" + juce::String ((int) j.chunks.size())
                                + " chunks)");
}

} // namespace voidengine
