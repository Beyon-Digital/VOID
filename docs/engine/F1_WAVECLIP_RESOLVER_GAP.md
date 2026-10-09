# F1 engine finding — file-backed wave clips resolve to silence

Found by the F1 journey lane while qualifying T44 (offline song). Repro
and numbers live in `docs/verification/F1/JOURNEY_EVIDENCE.md`.

## Symptom

An audio clip inserted via `InsertAudioClipOp` and persisted survives a
worker kill/restart — the `AUDIOCLIP` element is in the checkpoint edit
XML — but renders **silence** on every render path. Synthesizing the same
edit with an **absolute** `source=` path renders the take correctly
(peak 0.81 in the F1 fixture).

## Root cause

Write side: `opInsertAudioClip` /
`te::AudioClipBase` stores a SourceFileReference whose `source` string is
written **relative to the edit file's path treated as a directory**
(Tracktion `getRelativePathFrom(editFile)` convention). Persisted value
(example, T44): `source="../../../assets/sha256/<sha>/vox-take.wav"`
relative to `checkpoints/live/engine.tracktionedit` →
`container/assets/sha256/…`. Correct, anchored, sandbox-safe.

Read side: `SourceFileReference::findFileFromString` resolves
unqualified refs through `edit.filePathResolver`, falling back to
`getEditFileFromProjectManager(edit).getChildFile(desc)`:

1. `Edit::filePathResolver` — `EngineSession` never sets it when
   constructing `te::Edit::Options` (`Ops.cpp` ~line 107: only `role` +
   `editFileRetriever` are set).
2. `getEditFileFromProjectManager` — VOID edits live outside a TE
   `Project` (no `project.json` siblings), so it returns an empty
   `juce::File`.
3. `juce::File{}.getChildFile("../../../assets/…")` resolves
   **process-CWD-relative** → never exists → `createAudioFile` fails →
   clip renders as silence (`JUCE Assertion failure in
   juce_File.cpp:219` on debug builds).

So a wave clip silently never sounds on reopen — exactly the failure
mode the W11 "import → save → reopen → export" journey must not have.

## Demonstrated fix (fixture)

`native/void-engine/tests/render_fixture.cpp` (--edit mode) now sets,
mirroring TE's write-side anchor:

```cpp
options.editFileRetriever = [editFile] { return editFile; };
options.filePathResolver = [editFile] (const juce::String& desc)
{
    return editFile.getChildFile (desc);   // edit path acts as dir
};
```

With only that resolver, the persisted relative `source=` renders the
take — confirming the anchor convention round-trips.

## Engine fix needed (not this lane's path)

In `EngineSession` (`native/void-engine/src/session/Ops.cpp`,
`opOpenProject` and `opCreateProject`): set
`options.filePathResolver` anchored to the live edit file —
`container/checkpoints/live/engine.tracktionedit` — so session-rendered
audio clips (and any post-reopen re-render inside the worker) resolve
the same strings the writer emits. Without it, wave-clip playback in the
worker is broken in the same way the fixture render was.

## Adjacent note — session-scoped asset registry

`opOpenProject` clears `assets_` and `rebuildIndexes()` doesn't
repopulate it (`app-state.json` has no asset list): `ASSET_LIST` reads
empty after reopen until the coordinator re-runs `AttachAssetOp`. The
clip→file binding persists in the edit XML so renders are unaffected,
but any coordinator relying on `ASSET_LIST` post-reopen will see an
empty inventory.
