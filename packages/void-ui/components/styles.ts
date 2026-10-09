// Design tokens + the one injected stylesheet (focus rings, reduced motion).
// Inline styles are the convention here; :focus-visible and media queries
// can't be expressed inline, so injectVoidStyles() adds them once.

export const tokens = {
  // Signal Studio semantic tokens (theme/tokens.css). Fallbacks match the
  // dark theme so un-themed surfaces still render legibly.
  bg: 'var(--void-bg, #111214)',
  surface: 'var(--void-surface, #191B1E)',
  surfaceRaised: 'var(--void-surface-raised, var(--void-raised, #23262A))',
  raised: 'var(--void-raised, #23262A)',
  hover: 'var(--void-hover, #30343A)',
  border: 'var(--void-border, var(--void-line, #353A40))',
  line: 'var(--void-line, #353A40)',
  accent: 'var(--void-accent, #D8FC71)',
  onAccent: 'var(--void-on-accent, #172004)',
  accentSoft: 'var(--void-accent-soft, #29331B)',
  violet: 'var(--void-violet, #C4B0FB)',
  violetSoft: 'var(--void-violet-soft, #322943)',
  blue: 'var(--void-blue, #9BCDF4)',
  blueSoft: 'var(--void-blue-soft, #203443)',
  mint: 'var(--void-mint, #8ED9B3)',
  mintSoft: 'var(--void-mint-soft, #1E352B)',
  orange: 'var(--void-orange, #F6BB81)',
  orangeSoft: 'var(--void-orange-soft, #3C2D21)',
  danger: 'var(--void-danger, #FF918C)',
  dangerSoft: 'var(--void-danger-soft, #422728)',
  warn: 'var(--void-warn, var(--void-orange, #F6BB81))',
  ok: 'var(--void-ok, var(--void-mint, #8ED9B3))',
  white: 'var(--void-white, #FFFFFF)',
  ink: 'var(--void-ink, #111214)',
  text: 'var(--void-text, #F0F2EB)',
  textSecondary: 'var(--void-text-secondary, var(--void-muted, #ADB4B9))',
  muted: 'var(--void-muted, #ADB4B9)',
  textMuted: 'var(--void-text-muted, var(--void-subtle, #7F8990))',
  subtle: 'var(--void-subtle, #7F8990)',
  radius: 'var(--void-radius, var(--void-radius-8, 8px))',
  radius4: 'var(--void-radius-4, 4px)',
  radius8: 'var(--void-radius-8, 8px)',
  radius12: 'var(--void-radius-12, 12px)',
  radius16: 'var(--void-radius-16, 16px)',
  radius24: 'var(--void-radius-24, 24px)',
  space4: 'var(--void-space-4, 4px)',
  space8: 'var(--void-space-8, 8px)',
  space12: 'var(--void-space-12, 12px)',
  space16: 'var(--void-space-16, 16px)',
  space20: 'var(--void-space-20, 20px)',
  space24: 'var(--void-space-24, 24px)',
  space32: 'var(--void-space-32, 32px)',
  space40: 'var(--void-space-40, 40px)',
  space48: 'var(--void-space-48, 48px)',
  fontBody: 'var(--void-font-body, "Inter", system-ui, sans-serif)',
  fontDisplay: 'var(--void-font-display, "Space Grotesk", "Inter", system-ui, sans-serif)',
  fontNumeric: 'var(--void-font-numeric, "IBM Plex Mono", ui-monospace, monospace)',
  mono: 'var(--void-font-numeric, "IBM Plex Mono"), ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
  sans: 'var(--void-font-body, "Inter"), system-ui, -apple-system, sans-serif',
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
