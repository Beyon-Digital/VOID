// EngineSession::writeCheckpoint — F0-minimal verified checkpoint write.
// Sequence per CONTRACTS.md §4 (staging → verify → CURRENT swap). The full
// retention/lineage/DB-reconcile state machine is owned by the persistence
// lane (crates/void-project); this is deliberately narrower.

#include "../session/EngineSession.h"

namespace voidengine
{

CommandResult EngineSession::writeCheckpoint (const juce::String& reason,
                                              juce::String& checkpointIdOut)
{
    if (edit_ == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_NOT_FOUND, 0, "no open project" };

    const auto container = containerDir();
    const auto cpId = juce::Uuid().toString();
    const auto staging = container.getChildFile ("staging").getChildFile (cpId);
    if (! staging.createDirectory().wasOk())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_DISK_FULL, 0, "cannot create staging dir" };

    // Engine snapshot — record current revision inside the edit state so a
    // reopened project knows the coordinator revision it was saved at.
    edit_->state.setProperty ("voidRevision", (juce::int64) revision_, nullptr);
    te::EditFileOperations fops (*edit_);
    const auto editDest = staging.getChildFile ("engine.tracktionedit");
    juce::FileOutputStream fos (editDest);
    fos.setPosition (0);
    fos.truncate();
    fos << edit_->state.toXmlString();
    fos.flush();
    edit_->state.removeProperty ("voidRevision", nullptr);
    if (! editDest.existsAsFile() || editDest.getSize() == 0)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_DISK_FULL, 0, "engine state write failed" };

    // app-state.json
    {
        auto* o = new juce::DynamicObject();
        o->setProperty ("projectId", projectId_);
        o->setProperty ("name", projectName_);
        o->setProperty ("revision", (juce::int64) revision_);
        o->setProperty ("reason", reason);
        staging.getChildFile ("app-state.json")
            .replaceWithText (juce::JSON::toString (juce::var (o), true));
    }

    // command-receipts.json — committed receipts at this boundary
    {
        juce::Array<juce::var> rows;
        for (auto& kv : receipts_)
        {
            auto* r = new juce::DynamicObject();
            r->setProperty ("commandId", juce::String (kv.first.c_str()));
            r->setProperty ("transactionId", kv.second.transactionId);
            r->setProperty ("status", (int) kv.second.status);
            r->setProperty ("error", (int) kv.second.error);
            r->setProperty ("revision", (juce::int64) kv.second.revision);
            r->setProperty ("payloadHash", kv.second.payloadHash);
            rows.add (juce::var (r));
        }
        staging.getChildFile ("command-receipts.json")
            .replaceWithText (juce::JSON::toString (juce::var (rows), true));
    }

    // manifest.json — complete file inventory with sha256
    {
        juce::Array<juce::var> files;
        for (auto& f : staging.findChildFiles (juce::File::findFiles, false))
            if (f.getFileName() != "manifest.json")
            {
                auto* e = new juce::DynamicObject();
                e->setProperty ("path", f.getRelativePathFrom (staging));
                e->setProperty ("sha256", juce::SHA256 (f).toHexString());
                e->setProperty ("bytes", (juce::int64) f.getSize());
                files.add (juce::var (e));
            }
        auto* m = new juce::DynamicObject();
        m->setProperty ("checkpointId", cpId);
        m->setProperty ("projectId", projectId_);
        m->setProperty ("revision", (juce::int64) revision_);
        m->setProperty ("format", "void-checkpoint/1");
        m->setProperty ("files", juce::var (files));
        staging.getChildFile ("manifest.json")
            .replaceWithText (juce::JSON::toString (juce::var (m), true));
    }

    // Publish: move staging → checkpoints/<id> then atomic CURRENT swap.
    const auto dest = container.getChildFile ("checkpoints").getChildFile (cpId);
    dest.getParentDirectory().createDirectory();
    if (! staging.moveFileTo (dest))
        return { vp::AckStatus_REJECTED, vp::ErrorCode_DISK_FULL, 0, "checkpoint publish move failed" };

    auto* cur = new juce::DynamicObject();
    cur->setProperty ("checkpointId", cpId);
    cur->setProperty ("manifestSha256",
                      juce::SHA256 (dest.getChildFile ("manifest.json")).toHexString());
    container.getChildFile ("CURRENT")
        .replaceWithText (juce::JSON::toString (juce::var (cur), true));

    checkpointIdOut = cpId;
    return { vp::AckStatus_APPLIED, vp::ErrorCode_NONE, 0, "SAVE_DURABLE " + cpId };
}

} // namespace voidengine
