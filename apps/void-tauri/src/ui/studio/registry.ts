// Screen registry — FILE CONVENTION ONLY.
// A screen lane adds `screens/<screen-id>/index.tsx` default-exporting its
// component; the glob below discovers it at build time. No shared manifest,
// no per-lane edits here. Unimplemented ids render the honest placeholder.

import type * as React from 'react';

type ScreenModule = { default?: React.ComponentType; Screen?: React.ComponentType };

const modules = import.meta.glob('./screens/*/index.tsx', { eager: true }) as Record<
  string,
  ScreenModule
>;

export const screenRegistry: Record<string, React.ComponentType> = {};

for (const [path, mod] of Object.entries(modules)) {
  const m = /^\.\/screens\/([a-z0-9-]+)\/index\.tsx$/.exec(path);
  if (!m) continue;
  const Component = mod.default ?? mod.Screen;
  if (Component) screenRegistry[m[1]] = Component;
}

export function screenFor(id: string): React.ComponentType | null {
  return screenRegistry[id] ?? null;
}

export function knownScreenIds(): string[] {
  return Object.keys(screenRegistry).sort();
}
