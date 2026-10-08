// First-run onboarding + help state (W11 item 2, PRO-06).
//
// Whether the user has seen the tour is ordinary app preference state —
// legitimately app-local (not document data), persisted through the
// injected KeyValueStore. Tour/help copy lives in the i18n catalogs; this
// module owns only the step list shape and the seen/dismissed flags.

import { createStore, StoreApi } from 'zustand/vanilla';
import { memoryStore, type KeyValueStore } from '../recents/persistence';

export const ONBOARDING_STORAGE_KEY = 'void.onboarding.v1';

/** Ordered first-run steps — i18n keys, resolved by the shell's `t`. */
export const ONBOARDING_STEPS: readonly { id: string; titleKey: string; bodyKey: string }[] = [
  { id: 'project', titleKey: 'onboarding.step1.title', bodyKey: 'onboarding.step1.body' },
  { id: 'views', titleKey: 'onboarding.step2.title', bodyKey: 'onboarding.step2.body' },
  { id: 'transport', titleKey: 'onboarding.step3.title', bodyKey: 'onboarding.step3.body' },
  { id: 'notes-relink', titleKey: 'onboarding.step4.title', bodyKey: 'onboarding.step4.body' },
  { id: 'keyboard', titleKey: 'onboarding.step5.title', bodyKey: 'onboarding.step5.body' },
];

export interface OnboardingState {
  /** True once the user has completed or skipped the first-run tour. */
  seen: boolean;
  /** Current step index while the tour is showing. */
  step: number;
  /** Tour currently displayed (first run, or re-opened from help). */
  tourOpen: boolean;
  /** Help panel displayed. */
  helpOpen: boolean;
  persistError?: string;
}

export interface OnboardingActions {
  openTour(): void;
  nextStep(): void;
  prevStep(): void;
  /** Complete or skip — both mark seen and close. */
  finishTour(): void;
  openHelp(): void;
  closeHelp(): void;
}

export type OnboardingStore = StoreApi<OnboardingState & { actions: OnboardingActions }>;

interface Persisted {
  seen?: unknown;
}

export function createOnboardingStore(
  kv?: KeyValueStore,
  storageKey = ONBOARDING_STORAGE_KEY,
): OnboardingStore {
  const backing = kv ?? memoryStore();

  const readSeen = (): boolean => {
    try {
      const raw = backing.getItem(storageKey);
      if (!raw) return false;
      const doc = JSON.parse(raw) as Persisted;
      return doc.seen === true;
    } catch {
      return false;
    }
  };

  const persist = (seen: boolean): string | undefined => {
    try {
      backing.setItem(storageKey, JSON.stringify({ v: 1, seen }));
      return undefined;
    } catch (e) {
      return String(e);
    }
  };

  const seen = readSeen();

  return createStore<OnboardingState & { actions: OnboardingActions }>()((set, get) => ({
    seen,
    step: 0,
    tourOpen: !seen,
    helpOpen: false,
    persistError: undefined,
    actions: {
      openTour: () => set({ tourOpen: true, step: 0 }),
      nextStep: () =>
        set((s) => ({ step: Math.min(s.step + 1, ONBOARDING_STEPS.length - 1) })),
      prevStep: () => set((s) => ({ step: Math.max(s.step - 1, 0) })),
      finishTour: () =>
        set(() => ({ tourOpen: false, seen: true, persistError: persist(true) })),
      openHelp: () => set({ helpOpen: true }),
      closeHelp: () => set({ helpOpen: false }),
    },
  }));
}
