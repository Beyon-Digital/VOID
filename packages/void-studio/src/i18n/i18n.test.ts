import { describe, expect, it } from 'vitest';
import { createTranslator, isMissingKey, missingKeys } from './catalog';
import { en } from './en';

describe('translator', () => {
  const t = createTranslator(en);

  it('resolves keys and interpolates params', () => {
    expect(t('templates.empty.name')).toBe('Empty');
    expect(t('templates.apply.summary', { applied: 3, total: 5 })).toBe(
      '3 of 5 template operations applied',
    );
    expect(t('recents.lastOpened', { at: 'x' })).toBe('last opened x');
  });

  it('marks missing keys visibly instead of rendering empty', () => {
    const r = t('does.not.exist');
    expect(isMissingKey(r, 'does.not.exist')).toBe(true);
    expect(r).toContain('does.not.exist');
  });

  it('layers a partial catalog over the fallback', () => {
    const partial = { 'templates.empty.name': 'Leer' };
    const tt = createTranslator(partial, en);
    expect(tt('templates.empty.name')).toBe('Leer');
    expect(tt('templates.empty.description')).toBe(en['templates.empty.description']);
    expect(tt('missing.key')).toBe('⟦missing.key⟧');
  });
});

describe('english catalog integrity', () => {
  it('every key the UI surfaces uses is defined', () => {
    const required = [
      ...['empty', 'vocalDrums', 'liveBand'].flatMap((k) => [
        `templates.${k}.name`,
        `templates.${k}.description`,
      ]),
      ...['vocals', 'drums', 'keys', 'guitar', 'bass', 'mixBus'].map((k) => `templates.track.${k}`),
      'launcher.title',
      'recents.title',
      'recents.empty',
      'notes.title',
      'notes.opsUnavailable',
      'relink.title',
      'relink.ingestGap',
      'onboarding.title',
      'help.title',
    ];
    expect(missingKeys(en, required)).toEqual([]);
  });

  it('onboarding step keys all resolve', () => {
    const t = createTranslator(en);
    for (let i = 1; i <= 5; i++) {
      expect(t(`onboarding.step${i}.title`)).not.toContain('⟦');
      expect(t(`onboarding.step${i}.body`)).not.toContain('⟦');
    }
  });
});
