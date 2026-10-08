// Design tokens + the one injected stylesheet (focus rings, reduced motion).
// Inline styles are the convention here; :focus-visible and media queries
// can't be expressed inline, so injectVoidStyles() adds them once.

export const tokens = {
  bg: 'var(--void-bg, #0a0a0a)',
  surface: 'var(--void-surface, #15151a)',
  surfaceRaised: 'var(--void-surface-raised, #1d1d24)',
  border: 'var(--void-border, #2a2a33)',
  accent: 'var(--void-accent, #6a8dff)',
  accentSoft: 'var(--void-accent-soft, rgba(106,141,255,0.16))',
  danger: 'var(--void-danger, #ff5d5d)',
  warn: 'var(--void-warn, #e0b34f)',
  ok: 'var(--void-ok, #56c98d)',
  text: 'var(--void-text, #e4e4ea)',
  textSecondary: 'var(--void-text-secondary, #9a9aa5)',
  textMuted: 'var(--void-text-muted, #5c5c68)',
  radius: 'var(--void-radius, 6px)',
  mono: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
  sans: 'Inter, system-ui, -apple-system, sans-serif',
} as const;

const STYLE_ID = 'void-ui-a11y-styles';

const CSS = `
.${STYLE_ID}-focusable:focus-visible {
  outline: 2px solid var(--void-accent, #6a8dff);
  outline-offset: 2px;
}
.${STYLE_ID}-focusable:focus:not(:focus-visible) {
  outline: none;
}
@media (prefers-reduced-motion: reduce) {
  .${STYLE_ID}-animated {
    transition: none !important;
    animation: none !important;
  }
}
`;

/** Inject the shared focus/reduced-motion stylesheet (idempotent). */
export function injectVoidStyles(doc: Document | undefined = typeof document === 'undefined' ? undefined : document): void {
  if (!doc || doc.getElementById(STYLE_ID)) return;
  const el = doc.createElement('style');
  el.id = STYLE_ID;
  el.textContent = CSS;
  doc.head.appendChild(el);
}

export const focusClass = `${STYLE_ID}-focusable`;
export const animatedClass = `${STYLE_ID}-animated`;
