#include "TakeJournal.h"

namespace voidengine
{

juce::String takeStatusString (TakeStatus s)
{
    switch (s)
    {
        case TakeStatus::recording:  return "recording";
        case TakeStatus::finalized:  return "finalized";
        case TakeStatus::incomplete: return "incomplete";
        case TakeStatus::discarded:  return "discarded";
    }
    return "recording";
}

static TakeStatus statusFromString (const juce::String& s)
{
    if (s == "finalized")  return TakeStatus::finalized;
    if (s == "incomplete") return TakeStatus::incomplete;
    if (s == "discarded")  return TakeStatus::discarded;
    return TakeStatus::recording;
}

juce::File TakeJournalFile::takeDir (const juce::File& container, const juce::String& takeId)
{
    return container.getChildFile ("recordings").getChildFile (takeId);
}

juce::File TakeJournalFile::journalFile (const juce::File& takeDir)
{
    return takeDir.getChildFile ("journal.json");
}

bool TakeJournalFile::begin (const juce::File& container, TakeJournal& j)
{
    const auto dir = takeDir (container, j.takeId);
    if (! dir.createDirectory().wasOk())
        return false;
    j.status = TakeStatus::recording;
    return write (dir, j);
}

bool TakeJournalFile::write (const juce::File& takeDir, const TakeJournal& j)
{
    const auto tmp = takeDir.getChildFile ("journal.json.tmp");
    if (! tmp.replaceWithText (juce::JSON::toString (toVar (j), true)))
        return false;
    const auto dest = journalFile (takeDir);
    if (dest.existsAsFile() && ! tmp.moveFileTo (dest))
        return false;
    if (! dest.existsAsFile() && ! tmp.moveFileTo (dest))
        return false;
    return true;
}

bool TakeJournalFile::read (const juce::File& takeDir, TakeJournal& j)
{
    const auto f = journalFile (takeDir);
    if (! f.existsAsFile())
        return false;
    const auto v = juce::JSON::parse (f.loadFileAsString());
    return v.isObject() && fromVar (v, j);
}

bool TakeJournalFile::recordChunk (const juce::File& takeDir, TakeJournal& j,
                                   const TakeChunk& c)
{
    for (auto& e : j.chunks)
        if (e.path == c.path)
        {
            e = c;
            return write (takeDir, j);
        }
    j.chunks.push_back (c);
    return write (takeDir, j);
}

std::vector<TakeJournal> TakeJournalFile::recoverIncomplete (const juce::File& container)
{
    std::vector<TakeJournal> out;
    const auto recRoot = container.getChildFile ("recordings");
    if (! recRoot.isDirectory())
        return out;

    for (auto& dir : recRoot.findChildFiles (juce::File::findDirectories, false))
    {
        TakeJournal j;
        if (! read (dir, j))
            continue; // not a take dir or unreadable — leave for humans
        if (j.status != TakeStatus::recording)
            continue;
        j.status = TakeStatus::incomplete;
        j.endedUtc = juce::Time::getCurrentTime().toISO8601 (true);
        // Killed takes never got a final journal write — rebuild the chunk
        // list from the files actually on disk so durations are known.
        scanChunks (dir, j);
        write (dir, j);
        out.push_back (std::move (j));
    }
    return out;
}

juce::int64 TakeJournalFile::salvageableFrames (const TakeChunk& c)
{
    return c.framesObserved;
}

juce::var TakeJournalFile::toVar (const TakeJournal& j)
{
    auto* o = new juce::DynamicObject();
    o->setProperty ("format", "void-take-journal/1");
    o->setProperty ("takeId", j.takeId);
    o->setProperty ("projectId", j.projectId);
    o->setProperty ("status", takeStatusString (j.status));
    o->setProperty ("startedUtc", j.startedUtc);
    o->setProperty ("endedUtc", j.endedUtc);
    o->setProperty ("sampleRate", j.sampleRate);
    o->setProperty ("trackIds", j.trackIds);
    o->setProperty ("inputSummary", j.inputSummary);
    if (j.error.isNotEmpty())
        o->setProperty ("error", j.error);

    juce::Array<juce::var> rows;
    for (auto& c : j.chunks)
    {
        auto* e = new juce::DynamicObject();
        e->setProperty ("path", c.path);
        e->setProperty ("kind", c.kind);
        e->setProperty ("channels", c.channels);
        e->setProperty ("bytesObserved", c.bytesObserved);
        e->setProperty ("framesObserved", c.framesObserved);
        if (c.sha256.isNotEmpty())
            e->setProperty ("sha256", c.sha256);
        rows.add (juce::var (e));
    }
    o->setProperty ("chunks", juce::var (rows));

    juce::Array<juce::var> clips;
    for (auto& id : j.clipIds)
        clips.add (juce::var (id));
    o->setProperty ("clipIds", juce::var (clips));
    return juce::var (o);
}

bool TakeJournalFile::fromVar (const juce::var& v, TakeJournal& j)
{
    if (! v.isObject() || v["format"].toString() != "void-take-journal/1")
        return false;
    j.takeId       = v["takeId"].toString();
    j.projectId    = v["projectId"].toString();
    j.status       = statusFromString (v["status"].toString());
    j.startedUtc   = v["startedUtc"].toString();
    j.endedUtc     = v["endedUtc"].toString();
    j.sampleRate   = (double) v["sampleRate"];
    j.trackIds     = v["trackIds"].toString();
    j.inputSummary = v["inputSummary"].toString();
    j.error        = v["error"].toString();

    if (auto* chunks = v["chunks"].getArray())
        for (auto& e : *chunks)
        {
            TakeChunk c;
            c.path           = e["path"].toString();
            c.kind           = e["kind"].toString();
            c.channels       = (int) e["channels"];
            c.bytesObserved  = (juce::int64) e["bytesObserved"];
            c.framesObserved = (juce::int64) e["framesObserved"];
            c.sha256         = e["sha256"].toString();
            j.chunks.push_back (c);
        }
    if (auto* clips = v["clipIds"].getArray())
        for (auto& id : *clips)
            j.clipIds.add (id.toString());
    return true;
}

void TakeJournalFile::scanChunks (const juce::File& takeDir, TakeJournal& j)
{
    // Inventory every take file on disk: chunk-*.wav (44-byte PCM header for
    // channels + data frames) and midi-*.jsonl (event line count). Files are
    // read-only here — TE may still have them mapped on a crashed writer.
    for (auto& f : takeDir.findChildFiles (juce::File::findFiles, false))
    {
        const auto name = f.getFileName();
        const bool wav = name.startsWith ("chunk-") && name.endsWith (".wav");
        const bool midi = name.startsWith ("midi-") && name.endsWith (".jsonl");
        if (! wav && ! midi)
            continue;
        TakeChunk c;
        c.path = name;
        c.kind = midi ? "midi-events" : "audio";
        c.bytesObserved = f.getSize();
        if (wav)
        {
            // RIFF chunk walk: TE pads headers with JUNK/bext and leaves the
            // data-size unfinalized in killed takes — never fixed offsets.
            juce::FileInputStream in (f);
            juce::MemoryBlock head;
            if (in.openedOk() && in.readIntoMemoryBlock (head, 65536))
            {
                auto rd16 = [&] (size_t o) { return (int) ((const uint8_t*) head.getData())[o]
                                                    | (((const uint8_t*) head.getData())[o + 1] << 8); };
                auto rd32 = [&] (size_t o) { return (uint32_t) ((const uint8_t*) head.getData())[o]
                                                    | ((uint32_t) ((const uint8_t*) head.getData())[o + 1] << 8)
                                                    | ((uint32_t) ((const uint8_t*) head.getData())[o + 2] << 16)
                                                    | ((uint32_t) ((const uint8_t*) head.getData())[o + 3] << 24); };
                int ch = 0, blockAlign = 0;
                juce::int64 dataBytes = 0, dataPayloadEnd = -1;
                if (head.getSize() >= 12 && std::memcmp (head.getData(), "RIFF", 4) == 0)
                {
                    size_t o = 12;
                    while (o + 8 <= head.getSize())
                    {
                        const auto id = (const char*) head.getData() + o;
                        const uint32_t sz = rd32 (o + 4);
                        if (std::memcmp (id, "fmt ", 4) == 0 && sz >= 16)
                        {
                            ch = rd16 (o + 10);
                            blockAlign = rd16 (o + 20);
                        }
                        else if (std::memcmp (id, "data", 4) == 0)
                        {
                            dataBytes = sz;
                            // Payload region begins after this chunk header.
                            dataPayloadEnd = (juce::int64) o + 8;
                            break;
                        }
                        o += 8 + sz + (sz & 1);
                    }
                }
                const int bpf = juce::jmax (1, blockAlign);
                c.channels = ch;
                if (dataBytes > 0)
                    c.framesObserved = (juce::int64) (dataBytes / (uint32_t) bpf);
                else if (dataPayloadEnd > 0 && c.bytesObserved > dataPayloadEnd)
                    // data-size not flushed: honest estimate from file extent.
                    c.framesObserved = (c.bytesObserved - dataPayloadEnd) / bpf;
            }
        }
        else
        {
            juce::FileInputStream in (f);
            juce::int64 lines = 0;
            juce::MemoryBlock mb;
            if (in.openedOk() && in.readIntoMemoryBlock (mb))
                for (size_t i = 0; i < mb.getSize(); ++i)
                    if (static_cast<const char*> (mb.getData())[i] == '\n')
                        ++lines;
            c.framesObserved = lines;
        }
        recordChunk (takeDir, j, c);
    }
}

} // namespace voidengine
