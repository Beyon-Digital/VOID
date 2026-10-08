// Project template descriptors (W11 item 2, DOC-01).
//
// A template is *declarative capability metadata*: a name/description (as
// i18n keys), a starting tempo/sample rate/time signature and a track
// spec. Applying one issues ordinary persistent ops — CreateProjectOp
// then AddTrackOp (+ InsertPluginOp for instrument specs) — so a
// templated project is indistinguishable from a hand-built one. No
// template is ever a project clone: descriptors carry no clips, notes or
// asset references.

import type { TrackKind } from 'void-client';
import { FOUR_OSC, SAMPLER } from '../instruments/descriptors';

/** One track a template asks the engine to create. */
export interface TrackSpec {
  /** Engine TrackKind sent in AddTrackOp. */
  kind: TrackKind;
  /** i18n key for the default track name (resolved through the translator). */
  nameKey: string;
  /**
   * Optional builtin instrument to attach — an `InstrumentDescriptor`
   * pluginUid installed via insertInstrumentOp (InsertPluginOp,
   * format VOID-BUILTIN). Instrument specs imply kind INSTRUMENT.
   */
  instrumentUid?: string;
}

export interface ProjectTemplateDescriptor {
  /** Stable template id (descriptor registry key). */
  id: string;
  /** i18n keys for the picker UI. */
  nameKey: string;
  descriptionKey: string;
  /** Sent on CreateProjectOp. */
  sampleRate: number;
  initialBpm: number;
  /** Applied via SetTimeSignatureOp only when it differs from 4/4. */
  timeSignature?: { numerator: number; denominator: number };
  tracks: readonly TrackSpec[];
}

/** 'Empty' — a bare project; nothing is created beyond the container. */
export const TEMPLATE_EMPTY: ProjectTemplateDescriptor = {
  id: 'builtin.empty',
  nameKey: 'templates.empty.name',
  descriptionKey: 'templates.empty.description',
  sampleRate: 48000,
  initialBpm: 120,
  tracks: [],
};

/** 'Vocal + Drums' — one audio take lane plus a sampler drum kit. */
export const TEMPLATE_VOCAL_DRUMS: ProjectTemplateDescriptor = {
  id: 'builtin.vocal_drums',
  nameKey: 'templates.vocalDrums.name',
  descriptionKey: 'templates.vocalDrums.description',
  sampleRate: 48000,
  initialBpm: 128,
  tracks: [
    { kind: 'AUDIO', nameKey: 'templates.track.vocals' },
    { kind: 'INSTRUMENT', nameKey: 'templates.track.drums', instrumentUid: SAMPLER.pluginUid },
  ],
};

/** 'Live Band' — typical 5-piece layout plus a stereo mix bus. */
export const TEMPLATE_LIVE_BAND: ProjectTemplateDescriptor = {
  id: 'builtin.live_band',
  nameKey: 'templates.liveBand.name',
  descriptionKey: 'templates.liveBand.description',
  sampleRate: 48000,
  initialBpm: 110,
  tracks: [
    { kind: 'AUDIO', nameKey: 'templates.track.vocals' },
    { kind: 'AUDIO', nameKey: 'templates.track.guitar' },
    { kind: 'AUDIO', nameKey: 'templates.track.bass' },
    { kind: 'INSTRUMENT', nameKey: 'templates.track.drums', instrumentUid: SAMPLER.pluginUid },
    { kind: 'INSTRUMENT', nameKey: 'templates.track.keys', instrumentUid: FOUR_OSC.pluginUid },
    { kind: 'BUS', nameKey: 'templates.track.mixBus' },
  ],
};

/** The shipped template registry, in picker order. */
export const BUILTIN_TEMPLATES: readonly ProjectTemplateDescriptor[] = [
  TEMPLATE_EMPTY,
  TEMPLATE_VOCAL_DRUMS,
  TEMPLATE_LIVE_BAND,
];

export function templateById(
  id: string,
  catalog: readonly ProjectTemplateDescriptor[] = BUILTIN_TEMPLATES,
): ProjectTemplateDescriptor | undefined {
  return catalog.find((t) => t.id === id);
}
