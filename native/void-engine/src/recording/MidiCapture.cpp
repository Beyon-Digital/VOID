#include "MidiCapture.h"
#include <chrono>

namespace voidengine
{

static uint64_t hostNs()
{
    return (uint64_t) std::chrono::duration_cast<std::chrono::nanoseconds> (
               std::chrono::steady_clock::now().time_since_epoch()).count();
}

MidiCapture::MidiCapture() = default;

MidiCapture::~MidiCapture()
{
    end();
}

bool MidiCapture::begin (te::InputDeviceInstance& instance, const juce::File& takeDir,
                         const juce::String& trackId)
{
    end();
    trackId_ = trackId;
    outFile_ = takeDir.getChildFile ("midi-" + trackId + ".jsonl");
    out_ = outFile_.createOutputStream();
    if (out_ == nullptr || ! out_->openedOk())
        return false;
    instance_ = &instance;
    eventsWritten_ = 0;
    discarded_ = false;
    instance_->addConsumer (this);
    active_ = true;
    startTimerHz (20); // message thread only
    return true;
}

void MidiCapture::end()
{
    if (! active_.load() && instance_ == nullptr)
        return;
    active_ = false;
    stopTimer();
    if (instance_ != nullptr)
        instance_->removeConsumer (this);
    instance_ = nullptr;
    drain();
    if (out_ != nullptr)
    {
        out_->flush();
        out_.reset();
    }
}

void MidiCapture::handleIncomingMidiMessage (const juce::MidiMessage& m,
                                             te::MPESourceID)
{
    // Realtime path: copy ≤3 bytes + stamps into the lock-free fifo. No locks,
    // no allocation, no disk — the callback never waits on the journal.
    Event e;
    const auto n = juce::jmin (3, m.getRawDataSize());
    if (n > 0)
        std::memcpy (e.bytes, m.getRawData(), (size_t) n);
    e.size   = (uint8_t) n;
    e.stamp  = m.getTimeStamp();
    e.hostNs = hostNs();
    // Lock-free SPSC write; full FIFO drops (journal gap is honest, not hidden).
    const auto w = fifo_.write (1);
    if (w.blockSize1 > 0)
    {
        fifoBuf_[(size_t) w.startIndex1] = e;
        fifo_.finishedWrite (1);
    }
}

void MidiCapture::discardRecordings (te::EditItemID)
{
    // TE also fires this when a normal stop drops pre-roll padding — the
    // journal records take state on its own; no marker line needed.
}

int MidiCapture::emitAllNotesOff (te::MidiInputDevice& device)
{
    int sent = 0;
    for (int ch = 1; ch <= 16; ++ch)
    {
        auto msg = juce::MidiMessage::allNotesOff (ch);
        Event e;
        e.bytes[0] = (uint8_t) msg.getRawData()[0];
        e.bytes[1] = (uint8_t) msg.getRawData()[1];
        e.bytes[2] = (uint8_t) msg.getRawData()[2];
        e.size = 3;
        e.stamp = juce::Time::getMillisecondCounterHiRes() * 0.001;
        e.hostNs = hostNs();
        const auto w = fifo_.write (1);
        if (w.blockSize1 > 0)
        {
            fifoBuf_[(size_t) w.startIndex1] = e;
            fifo_.finishedWrite (1);
            ++sent;
        }
        device.handleIncomingMessage (msg);
    }
    return sent;
}

void MidiCapture::timerCallback()
{
    drain();
}

void MidiCapture::drain()
{
    if (out_ == nullptr)
        return;
    for (;;)
    {
        const auto r = fifo_.read (1);
        if (r.blockSize1 <= 0)
            break;
        const Event e = fifoBuf_[(size_t) r.startIndex1];
        fifo_.finishedRead (1);
        const auto status = e.bytes[0] & 0xf0;
        const auto chan   = (e.bytes[0] & 0x0f) + 1;
        juce::String kind = "raw";
        switch (status)
        {
            case 0x80: kind = "noteOff"; break;
            case 0x90: kind = (e.size >= 3 && e.bytes[2] == 0) ? "noteOff" : "noteOn"; break;
            case 0xa0: kind = "polyAftertouch"; break;
            case 0xb0:
                kind = (e.bytes[1] == 64) ? "sustain" :
                       (e.bytes[1] == 123) ? "allNotesOff" :
                       (e.bytes[1] == 120) ? "allSoundOff" : "cc";
                break;
            case 0xc0: kind = "program"; break;
            case 0xd0: kind = "aftertouch"; break;
            case 0xe0: kind = "pitchBend"; break;
            default:   kind = "raw"; break;
        }
        juce::String line;
        line << "{\"track\":\"" << trackId_ << "\","
             << "\"kind\":\"" << kind << "\","
             << "\"status\":" << (int) (e.bytes[0] & 0xf0) << ","
             << "\"chan\":" << chan << ","
             << "\"d1\":" << (int) (e.size > 1 ? e.bytes[1] : 0) << ","
             << "\"d2\":" << (int) (e.size > 2 ? e.bytes[2] : 0) << ","
             << "\"ts\":" << juce::String (e.stamp, 6) << ","
             << "\"hostNs\":" << juce::String ((juce::int64) e.hostNs)
             << "}\n";
        out_->writeText (line, false, false, nullptr);
        ++eventsWritten_;
    }
    out_->flush();
}

} // namespace voidengine
