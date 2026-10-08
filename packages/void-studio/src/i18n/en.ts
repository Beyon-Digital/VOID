// English catalog (W11, PRO-06) — the shipped locale and the key list
// every other locale mirrors. Keys are dotted by surface:
//   templates.*  launcher.*  recents.*  notes.*  relink.*  onboarding.*  help.*
// `en` is typed as a plain catalog so `missingKeys(en, …)` can QA it.

import type { I18nCatalog } from './catalog';

export const en: I18nCatalog = {
  // -- templates -------------------------------------------------------------
  'templates.title': 'Project templates',
  'templates.empty.name': 'Empty',
  'templates.empty.description':
    'A blank project at the default tempo and sample rate. No tracks — add them as you go.',
  'templates.vocalDrums.name': 'Vocal + Drums',
  'templates.vocalDrums.description':
    'One audio track for vocals and a Sampler instrument track for drums.',
  'templates.liveBand.name': 'Live Band',
  'templates.liveBand.description':
    'Vocal, guitar and bass audio tracks, drum and keys instrument tracks, and a mix bus.',
  'templates.track.vocals': 'Vocals',
  'templates.track.drums': 'Drums',
  'templates.track.keys': 'Keys',
  'templates.track.guitar': 'Guitar',
  'templates.track.bass': 'Bass',
  'templates.track.mixBus': 'Mix Bus',
  'templates.apply.summary': '{applied} of {total} template operations applied',
  'templates.apply.failed': 'template apply stopped — {op} was rejected ({error})',

  // -- launcher / create-open -------------------------------------------------
  'launcher.title': 'Project',
  'launcher.newProject': 'New project',
  'launcher.projectName': 'project name',
  'launcher.containerDir': 'container directory (.void)',
  'launcher.create': 'Create project',
  'launcher.openByPath': 'Open by path',
  'launcher.openProjectId': 'project id (from a previous open)',
  'launcher.open': 'Open project',
  'launcher.busy': 'working…',
  'launcher.created': 'project {name} created at revision {revision}',
  'launcher.opened': 'project {name} opened at revision {revision}',
  'launcher.rejected': '{op} rejected: {error}{message}',

  // -- recents ----------------------------------------------------------------
  'recents.title': 'Recent projects',
  'recents.empty': 'no recent projects — created and opened projects appear here',
  'recents.open': 'Open',
  'recents.remove': 'Remove from recents',
  'recents.lastOpened': 'last opened {at}',
  'recents.kindCreated': 'created here',
  'recents.kindOpened': 'opened here',
  'recents.openFailed': 'open failed: {error}',
  'recents.persistError': 'recents cache could not be written ({error}) — list is session-only',

  // -- notes ------------------------------------------------------------------
  'notes.title': 'Notes',
  'notes.projectInfo': 'Project information',
  'notes.projectNote': 'Project note',
  'notes.trackNote': 'Track note — {track}',
  'notes.noTrackSelected': 'select a track to see its note',
  'notes.documentValue': 'Document value',
  'notes.noDocumentValue': '— none reported by the engine view —',
  'notes.draftLabel': 'Draft (local, unsent)',
  'notes.draftPlaceholder': 'type a note…',
  'notes.apply': 'Apply note to document',
  'notes.clearDraft': 'Discard draft',
  'notes.opsUnavailable':
    'SetProjectNoteOp / SetTrackNoteOp are not in protocol major.1 — this draft is kept locally as unsent intent (docs/engine/NEEDS.md §rev2). The field above shows the document value the engine view reports.',
  'notes.field.name': 'name',
  'notes.field.sampleRate': 'sample rate',
  'notes.field.bpm': 'tempo',
  'notes.field.key': 'key',
  'notes.field.timeSignature': 'time signature',
  'notes.field.trackCount': 'tracks',
  'notes.field.assetCount': 'assets',
  'notes.field.storageBytes': 'storage',

  // -- relink recovery ----------------------------------------------------------
  'relink.title': 'Missing assets',
  'relink.read': 'Read ASSET_LIST',
  'relink.none': 'no missing assets reported in the latest ASSET_LIST read',
  'relink.noData': 'no ASSET_LIST page read yet — missing media shows here after a read',
  'relink.missingCount': '{count} missing asset(s)',
  'relink.chooseFile': 'Choose replacement file',
  'relink.verifying': 'hashing candidate…',
  'relink.match': 'candidate matches the expected content — relink will restore the original media',
  'relink.mismatch':
    'candidate hash {actual} differs from expected {expected} — this is a replacement, not the original media',
  'relink.confirmReplace': 'I understand this replaces the original media',
  'relink.attach': 'Attach re-linked asset',
  'relink.attached': 'attach receipt: {status} at revision {revision}',
  'relink.attachFailed': 'attach rejected: {error}{message}',
  'relink.ingestGap':
    'AttachAssetOp registers an already-imported blob — the coordinator must place the bytes under assets/sha256/ first (docs/engine/NEEDS.md §rev2). The receipt below is the real engine answer, not a simulated success.',
  'relink.expected': 'expected sha256',

  // -- onboarding -----------------------------------------------------------------
  'onboarding.title': 'Welcome to VOID Studio',
  'onboarding.next': 'Next',
  'onboarding.back': 'Back',
  'onboarding.skip': 'Skip tour',
  'onboarding.done': 'Start working',
  'onboarding.step1.title': 'Create or open a project',
  'onboarding.step1.body':
    'Start from a template or open a recent project. Everything stays local — no account, no cloud.',
  'onboarding.step2.title': 'Tracks and views',
  'onboarding.step2.body':
    'Tracks, clips and meters are read from the engine through bounded views. Press “read” on a list to refresh it — the screen never keeps its own copy of the song.',
  'onboarding.step3.title': 'Transport',
  'onboarding.step3.body':
    'Play, stop and seek from the transport bar. PANIC sends all-notes-off immediately — it is never queued behind edits.',
  'onboarding.step4.title': 'Notes and missing media',
  'onboarding.step4.body':
    'Project and track notes live with the document. If media is missing after a move, the relink panel lists it and verifies a replacement by content hash.',
  'onboarding.step5.title': 'Keyboard first',
  'onboarding.step5.body':
    'Every workspace has a keyboard path: Ctrl/⌘+1..3 switches workspaces, Tab reaches every control, Delete removes the selected clips.',

  // -- help ------------------------------------------------------------------------
  'help.title': 'Help',
  'help.close': 'Close help',
  'help.reopenTour': 'Replay the first-run tour',
  'help.section.project.title': 'Projects',
  'help.section.project.body':
    'A project lives in a .void container directory. Create from a template or open by path; recent projects record real opens from this install.',
  'help.section.views.title': 'Read views',
  'help.section.views.body':
    'The engine is the document authority. Lists on screen are bounded read pages — refresh them after edits instead of trusting a stale row.',
  'help.section.transport.title': 'Transport',
  'help.section.transport.body':
    'PLAY/STOP/SEEK ride a separate control channel. PANIC is acknowledged on send and bypasses the edit queue.',
  'help.section.notes.title': 'Notes',
  'help.section.notes.body':
    'Note editing requires protocol ops that are not in major.1 yet — drafts are kept locally and applied once the ops land (NEEDS.md §rev2).',
  'help.section.relink.title': 'Relink recovery',
  'help.section.relink.body':
    'Missing assets render as placeholders, never silent substitutes. Pick the original file to relink by hash, or a different file to replace it explicitly.',
};
