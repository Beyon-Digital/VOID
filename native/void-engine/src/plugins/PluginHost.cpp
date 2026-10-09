// Engine-hosted plugin ops (W06 in-process scope — runtime isolation is W20).
// A crashing plugin in-process can take the whole worker down; that's the
// documented T24 behaviour, evidenced, not hidden.

#include "../session/EngineSession.h"
#include <map>
#include <memory>

namespace voidengine
{

namespace
{
// Open native editor windows keyed by plugin instance id.
std::map<std::string, std::unique_ptr<juce::DocumentWindow>> gEditorWindows;
}

CommandResult EngineSession::insertExternalPlugin (const vp::InsertPluginOp& op)
{
    if (edit_ == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_NOT_FOUND, 0, "no open project" };
    auto* t = findTrack (op.track_id() ? op.track_id()->c_str() : "");
    if (t == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_NOT_FOUND, 0, "track not found" };

    const auto instanceId = juce::String (op.plugin_instance_id() ? op.plugin_instance_id()->c_str() : "");
    const auto format = juce::String (op.format() ? op.format()->c_str() : "");
    const auto uid    = juce::String (op.plugin_uid() ? op.plugin_uid()->c_str() : "");
    if (instanceId.isEmpty() || format.isEmpty() || uid.isEmpty())
        return { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, 0, "missing plugin identity" };

    // Test-only crash fixture (T24): a UID of "void.crash" inserts a real
    // in-process plugin instance whose audio callback aborts the process.
    if (uid == "void.crash")
    {
        armCrashFixture();
        auto crash = edit_->getPluginCache().createNewPlugin (te::TextPlugin::xmlTypeName, {});
        if (auto* p = crash.get())
        {
            p->state.setProperty ("voidInstanceId", instanceId, nullptr);
            p->state.setProperty ("voidCrashFixture", true, nullptr);
            t->pluginList.insertPlugin (*crash, op.slot(), nullptr);
            plugins_[instanceId.toStdString()] = p;
            return { vp::AckStatus_APPLIED, vp::ErrorCode_NONE, 0, "crash fixture armed (test)" };
        }
        return { vp::AckStatus_REJECTED, vp::ErrorCode_PLUGIN_UNAVAILABLE, 0, "fixture insert failed" };
    }

    juce::PluginDescription desc;
    if (format == "VST3")
    {
        desc.fileOrIdentifier = uid; // bundle path or VST3 UID
        desc.pluginFormatName = "VST3";
    }
    else if (format == "AU")
    {
        // uid form: "AudioUnit:<type>/<sub>/<mfr>" hex or .component path
        if (uid.startsWith ("AudioUnit:"))
        {
            auto parts = juce::StringArray::fromTokens (uid.substring (10), "/", "");
            if (parts.size() == 3)
            {
                desc.fileOrIdentifier = uid;
                desc.pluginFormatName = "AudioUnit";
            }
            else
                return { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, 0, "bad AU uid" };
        }
        else
        {
            desc.fileOrIdentifier = uid;
            desc.pluginFormatName = "AudioUnit";
        }
    }
    else
    {
        return { vp::AckStatus_REJECTED, vp::ErrorCode_UNSUPPORTED_CAPABILITY, 0,
                 "unsupported plugin format" };
    }

    auto plug = edit_->getPluginCache().createNewPlugin (te::ExternalPlugin::xmlTypeName, desc);
    if (plug == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_PLUGIN_UNAVAILABLE, 0,
                 "plugin unavailable or failed to load" };

    auto* p = plug.get();
    p->state.setProperty ("voidInstanceId", instanceId, nullptr);
    p->state.setProperty ("voidPluginUid", uid, nullptr);
    p->state.setProperty ("voidFormat", format, nullptr);
    t->pluginList.insertPlugin (*plug, op.slot(), nullptr);
    plugins_[instanceId.toStdString()] = p;
    return { vp::AckStatus_APPLIED, vp::ErrorCode_NONE, 0, {} };
}

CommandResult EngineSession::removePlugin (const vp::RemovePluginOp& op)
{
    const auto id = juce::String (op.plugin_instance_id() ? op.plugin_instance_id()->c_str() : "");
    auto* p = findPlugin (id);
    if (p == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_NOT_FOUND, 0, "plugin not found" };
    gEditorWindows.erase (id.toStdString());
    p->removeFromParent();
    plugins_.erase (id.toStdString());
    return { vp::AckStatus_APPLIED, vp::ErrorCode_NONE, 0, {} };
}

CommandResult EngineSession::setPluginParam (const vp::SetPluginParamOp& op)
{
    auto* p = findPlugin (op.plugin_instance_id() ? op.plugin_instance_id()->c_str() : "");
    if (p == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_NOT_FOUND, 0, "plugin not found" };
    const auto pid = juce::String (op.param_id() ? op.param_id()->c_str() : "");
    const float v = op.value();
    if (! std::isfinite (v))
        return { vp::AckStatus_REJECTED, vp::ErrorCode_BAD_REQUEST, 0, "NaN/Inf rejected" };
    for (auto* param : p->getAutomatableParameters())
        if (param->paramID == pid || param->paramName == pid)
        {
            param->setParameter (v, juce::sendNotificationSync);
            return { vp::AckStatus_APPLIED, vp::ErrorCode_NONE, 0, {} };
        }
    return { vp::AckStatus_REJECTED, vp::ErrorCode_NOT_FOUND, 0, "param not found" };
}

CommandResult EngineSession::openPluginEditor (const vp::OpenPluginEditorOp& op)
{
    const auto id = juce::String (op.plugin_instance_id() ? op.plugin_instance_id()->c_str() : "");
    auto* p = findPlugin (id);
    if (p == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_NOT_FOUND, 0, "plugin not found" };
    if (gEditorWindows.count (id.toStdString()))
        return { vp::AckStatus_DUPLICATE, vp::ErrorCode_NONE, revision_, "editor already open" };
    p->showWindowExplicitly(); // real native editor — engine-hosted
    auto* w = p->windowState ? p->windowState->pluginWindow.get() : nullptr;
    if (w == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_PLUGIN_UNAVAILABLE, 0,
                 "plugin reported no editor" };
    // Keep a strong ref only for bookkeeping; the window belongs to the plugin.
    gEditorWindows.emplace (id.toStdString(), nullptr);
    return { vp::AckStatus_APPLIED, vp::ErrorCode_NONE, 0, "editor open" };
}

CommandResult EngineSession::closePluginEditor (const vp::ClosePluginEditorOp& op)
{
    const auto id = juce::String (op.plugin_instance_id() ? op.plugin_instance_id()->c_str() : "");
    auto* p = findPlugin (id);
    if (p == nullptr)
        return { vp::AckStatus_REJECTED, vp::ErrorCode_NOT_FOUND, 0, "plugin not found" };
    gEditorWindows.erase (id.toStdString());
    p->hideWindowForShutdown();
    return { vp::AckStatus_APPLIED, vp::ErrorCode_NONE, 0, "editor closed" };
}

} // namespace voidengine
