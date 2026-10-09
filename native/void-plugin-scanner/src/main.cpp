// void-plugin-scanner — W06/T22 contained plugin probe.
//
// One-shot process: scan a single AU/VST3 identifier, print a JSON result
// line, exit. Running the scan in a dedicated process is the containment
// model — a hang or crash kills only this process; the supervisor enforces
// the timeout and quarantines by binary SHA-256.
//
// Usage:
//   void-plugin-scanner --format AU --uid "AudioUnit:aufx,dely,appl"
//   void-plugin-scanner --format VST3 --uid "/path/to/plug.vst3"
//   void-plugin-scanner --selftest hang|crash   (fixture modes for T22)
//
// Exit: 0 scan completed (found may still be false — read JSON "error"),
//       2 bad args.

#include <juce_core/juce_core.h>
#include <juce_audio_processors/juce_audio_processors.h>
#include <juce_audio_processors/format/juce_AudioPluginFormatManagerHelpers.h>
#include <juce_cryptography/juce_cryptography.h>

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <thread>

namespace
{

juce::String jsonEsc (const juce::String& s)
{
    juce::String out;
    for (auto c : s)
        switch (c)
        {
            case '"':  out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n";  break;
            case '\r': break;
            default:   out += c;
        }
    return out;
}

juce::String sha256OfFile (const juce::File& f)
{
    if (! f.existsAsFile())
        return {};
    return juce::SHA256 (f).toHexString();
}

// For bundle plugins (.vst3/.component dirs) hash the inner Mach-O binary;
// fall back to hashing the identifier string so the cache key is still
// content-addressed to the thing we probed.
juce::String identityHash (const juce::String& fileOrIdentifier)
{
    // AU component identifiers are not filesystem paths — skip the file probe.
    if (fileOrIdentifier.startsWith ("AudioUnit:"))
        return "id-sha256:" + juce::SHA256 (fileOrIdentifier.toUTF8())
                                       .toHexString();
    juce::File f (fileOrIdentifier);
    if (f.existsAsFile())
        return sha256OfFile (f);
    if (f.isDirectory())
        return sha256OfFile (f.getChildFile ("Contents/MacOS")
                                 .getChildFile (f.getFileNameWithoutExtension()));
    return "id-sha256:" + juce::SHA256 (fileOrIdentifier.toUTF8())
                                   .toHexString();
}

int selftest (const juce::String& mode)
{
    if (mode == "hang")
    {
        std::fprintf (stderr, "[scanner] selftest hang: sleeping forever\n");
        for (;;)
            std::this_thread::sleep_for (std::chrono::seconds (60));
    }
    if (mode == "crash")
    {
        std::fprintf (stderr, "[scanner] selftest crash: aborting\n");
        std::abort();
    }
    std::fprintf (stderr, "[scanner] unknown selftest mode %s\n", mode.toRawUTF8());
    return 2;
}

} // namespace

int main (int argc, char* argv[])
{
    juce::ScopedJuceInitialiser_GUI guiInit; // AU host path needs a run loop env

    juce::String format, uid, selftestMode;
    for (int i = 1; i + 1 < argc; ++i)
    {
        const juce::String a (argv[i]);
        if (a == "--format")        format = argv[i + 1];
        else if (a == "--uid")      uid = argv[i + 1];
        else if (a == "--selftest") selftestMode = argv[i + 1];
    }
    if (selftestMode.isNotEmpty())
        return selftest (selftestMode);
    if (format.isEmpty() || uid.isEmpty())
    {
        std::fprintf (stderr, "usage: void-plugin-scanner --format AU|VST3 --uid <id> "
                              "[--selftest hang|crash]\n");
        return 2;
    }

    juce::MessageManager::getInstance();

    juce::PluginDescription desc;
    // JUCE format names are "AudioUnit" / "VST3"; CLI takes AU|VST3.
    const juce::String juceFormat = format == "AU" ? "AudioUnit" : format;
    desc.pluginFormatName = juceFormat;

    if (format == "VST3")
    {
        desc.fileOrIdentifier = uid; // .vst3 bundle path or platform file ref
    }
    else if (format == "AU")
    {
        // Accept "aufx,dely,appl" or the JUCE identifier "AudioUnit:aufx,dely,appl".
        desc.fileOrIdentifier = uid.startsWith ("AudioUnit:") ? uid
                                                             : "AudioUnit:" + uid;
    }
    else
    {
        std::printf ("{\"found\":false,\"error\":\"unsupported format: %s\"}\n",
                     format.toRawUTF8());
        return 0;
    }

    juce::AudioPluginFormatManager fm;
    juce::addDefaultFormatsToManager (fm);

    juce::OwnedArray<juce::PluginDescription> found;
    for (auto* f : fm.getFormats())
        if (f->getName() == juceFormat)
        {
            juce::String fileToScan = desc.fileOrIdentifier;
            // VST3 path scans resolve to bundle contents; AU identifiers scan
            // through the component manager inside the format.
            if (format == "VST3")
                f->findAllTypesForFile (found, fileToScan);
            else
            {
                // AU: no filesystem scan — instantiate directly. The
                // synchronous manager call does the run-loop pump internally.
                juce::String err;
                auto inst = fm.createPluginInstance (desc, 48000.0, 256, err);

                if (inst == nullptr)
                {
                    std::printf ("{\"found\":false,\"format\":\"%s\",\"uid\":\"%s\","
                                 "\"error\":\"%s\"}\n",
                                 format.toRawUTF8(), jsonEsc (uid).toRawUTF8(),
                                 jsonEsc (err).toRawUTF8());
                    return 0;
                }
                auto* pd = new juce::PluginDescription();
                inst->fillInPluginDescription (*pd);
                found.add (pd);
            }
        }

    if (found.isEmpty())
    {
        std::printf ("{\"found\":false,\"format\":\"%s\",\"uid\":\"%s\","
                     "\"error\":\"no plugin types resolved\"}\n",
                     format.toRawUTF8(), jsonEsc (uid).toRawUTF8());
        return 0;
    }

    for (auto* d : found)
        std::printf ("{\"found\":true,\"format\":\"%s\",\"uid\":\"%s\","
                     "\"name\":\"%s\",\"manufacturer\":\"%s\",\"version\":\"%s\","
                     "\"category\":\"%s\",\"binarySha256\":\"%s\"}\n",
                     format.toRawUTF8(), jsonEsc (uid).toRawUTF8(),
                     jsonEsc (d->name).toRawUTF8(),
                     jsonEsc (d->manufacturerName).toRawUTF8(),
                     jsonEsc (d->version).toRawUTF8(),
                     jsonEsc (d->category).toRawUTF8(),
                     identityHash (desc.fileOrIdentifier).toRawUTF8());
    return 0;
}
