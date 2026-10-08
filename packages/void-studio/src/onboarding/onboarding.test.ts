import { describe, expect, it } from 'vitest';
import { memoryStore } from '../recents/persistence';
import { createOnboardingStore, ONBOARDING_STEPS } from './onboarding';

describe('onboarding store', () => {
  it('opens the tour on first run, closes and marks seen on finish', () => {
    const s = createOnboardingStore(memoryStore());
    expect(s.getState().seen).toBe(false);
    expect(s.getState().tourOpen).toBe(true);
    s.getState().actions.nextStep();
    expect(s.getState().step).toBe(1);
    s.getState().actions.finishTour();
    expect(s.getState().seen).toBe(true);
    expect(s.getState().tourOpen).toBe(false);
  });

  it('persists seen across store instances; reopening the tour works from help', () => {
    const kv = memoryStore();
    createOnboardingStore(kv).getState().actions.finishTour();
    const s2 = createOnboardingStore(kv);
    expect(s2.getState().seen).toBe(true);
    expect(s2.getState().tourOpen).toBe(false);
    s2.getState().actions.openTour();
    expect(s2.getState().tourOpen).toBe(true);
    expect(s2.getState().step).toBe(0);
  });

  it('step navigation is bounded', () => {
    const s = createOnboardingStore(memoryStore());
    s.getState().actions.prevStep();
    expect(s.getState().step).toBe(0);
    for (let i = 0; i < 10; i++) s.getState().actions.nextStep();
    expect(s.getState().step).toBe(ONBOARDING_STEPS.length - 1);
  });

  it('help opens and closes independently of the tour', () => {
    const s = createOnboardingStore(memoryStore());
    s.getState().actions.finishTour();
    s.getState().actions.openHelp();
    expect(s.getState().helpOpen).toBe(true);
    s.getState().actions.closeHelp();
    expect(s.getState().helpOpen).toBe(false);
  });

  it('tour steps resolve against the shipped catalog', async () => {
    const { createTranslator } = await import('../i18n/catalog');
    const { en } = await import('../i18n/en');
    const t = createTranslator(en);
    for (const step of ONBOARDING_STEPS) {
      expect(t(step.titleKey)).not.toContain('⟦');
      expect(t(step.bodyKey)).not.toContain('⟦');
    }
  });
});
