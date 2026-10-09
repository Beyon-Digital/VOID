// TakeJournal: per-take recording journal under <container>/recordings/<takeId>/.
//
// CONTRACTS.md §4: "recordings/<takeId>/  Growing take chunks + recoverable
// recording journal". The journal is a small JSON file rewritten atomically
// (temp file + rename) whenever a chunk length changes or the take state
// transitions. A take that dies mid-write keeps status "recording" on disk and
// is recovered + labelled "incomplete" on next open — never mistaken for a
// final asset.
#pragma once

#include <JuceHeader.h>

namespace voidengine
{

/// On-disk status values (stable strings — they are the recovery contract).
enum class TakeStatus
{
    recording,   ///< writer alive, take growing
    finalized,   ///< clean stop: clips created, lengths locked
    incomplete,  ///< crashed/interrupted take recovered on scan
    discarded    ///< user/system discarded; chunks quarantined, never used
};

juce::String takeStatusString (TakeStatus);

struct TakeChunk
{
    juce::String path;          ///< relative to the take dir
    juce::String kind;          ///< "audio" | "midi-events"
    int channels = 0;
    juce::int64 bytesObserved = 0;
    juce::int64 framesObserved = 0; ///< audio frames (or MIDI events)
    juce::String sha256;        ///< only at finalize; "" while growing
};

struct TakeJournal
{
    juce::String takeId;
    juce::String projectId;
    TakeStatus status = TakeStatus::recording;
    juce::String startedUtc;
    juce::String endedUtc;
    double sampleRate = 0.0;
    juce::String trackIds;      ///< comma-joined voidId list of armed targets
    juce::String inputSummary;  ///< human-readable input routing
    juce::String error;         ///< last error, if any
    std::vector<TakeChunk> chunks;
    juce::Array<juce::String> clipIds; ///< voidIds of clips created on finalize
};

/// Reads/writes journal.json inside one take directory.
class TakeJournalFile
{
public:
    /// Directory for a take inside the container.
    static juce::File takeDir (const juce::File& container, const juce::String& takeId);
    static juce::File journalFile (const juce::File& takeDir);

    /// Create recordings/<takeId>/ and write an initial journal.
    static bool begin (const juce::File& container, TakeJournal&);

    /// Rewrite journal.json atomically (temp sibling + rename).
    static bool write (const juce::File& takeDir, const TakeJournal&);

    /// Parse an existing journal; returns false if unreadable/corrupt.
    static bool read (const juce::File& takeDir, TakeJournal&);

    /// Update (or insert) a chunk entry and flush.
    static bool recordChunk (const juce::File& takeDir, TakeJournal&,
                             const TakeChunk&);

    /// Scan <container>/recordings for takes left in "recording" state.
    /// Each is marked `incomplete` on disk and returned for reporting.
    static std::vector<TakeJournal> recoverIncomplete (const juce::File& container);

    /// Rebuild the chunk list by scanning the take dir itself — used at
    /// recovery so killed takes report real persisted lengths, not guesses.
    static void scanChunks (const juce::File& takeDir, TakeJournal&);

    /// Best-effort salvage estimate for a chunk: frames readable if the file
    /// were salvaged now (bytes minus container header guess), always honest.
    static juce::int64 salvageableFrames (const TakeChunk&);

private:
    static juce::var toVar (const TakeJournal&);
    static bool fromVar (const juce::var&, TakeJournal&);
};

} // namespace voidengine
