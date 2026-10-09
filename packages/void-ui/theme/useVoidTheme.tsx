// Signal Studio theme context — owns the .void-studio scope element, the
// injected token stylesheet, and the dark/daylight toggle.
//
// `useVoidTheme` returns the active theme and setters; `VoidThemeProvider`
// wraps children in `<div class="void-studio" data-void-theme=…>` so every
// descendant picks up the semantic tokens + type scale from theme/tokens.css.
// The provider also injects the stylesheet into document.head so non-bundler
// consumers (void-tauri) get tokens without a CSS pipeline.

import * as React from 'react';
import { VOID_THEME_CSS, VOID_THEME_STYLE_ID } from './voidTheme';

export type VoidTheme = 'dark' | 'daylight';

export const VOID_THEME_STORAGE_KEY = 'void-studio-theme';

export interface VoidThemeContextValue {
  theme: VoidTheme;
  setTheme: (theme: VoidTheme) => void;
  toggleTheme: () => void;
}

const VoidThemeContext = React.createContext<VoidThemeContextValue>({
  theme: 'dark',
  setTheme: () => undefined,
  toggleTheme: () => undefined,
});

/** Inject the Signal Studio token stylesheet once (idempotent). */
export function injectVoidTheme(
  doc: Document | undefined = typeof document === 'undefined' ? undefined : document,
): void {
  if (!doc || doc.getElementById(VOID_THEME_STYLE_ID)) return;
  const el = doc.createElement('style');
  el.id = VOID_THEME_STYLE_ID;
  el.textContent = VOID_THEME_CSS;
  doc.head.appendChild(el);
}

function initialTheme(): VoidTheme {
  if (typeof window === 'undefined') return 'dark';
  try {
    const saved = window.localStorage.getItem(VOID_THEME_STORAGE_KEY);
    return saved === 'daylight' ? 'daylight' : 'dark';
  } catch {
    return 'dark';
  }
}

export interface VoidThemeProviderProps {
  children: React.ReactNode;
  /** Force a theme (e.g. the dev gallery renders one column per theme). */
  theme?: VoidTheme;
  /** Extra class names merged onto the .void-studio wrapper. */
  className?: string;
  style?: React.CSSProperties;
}

export function VoidThemeProvider({
  children,
  theme: forced,
  className,
  style,
}: VoidThemeProviderProps) {
  const [theme, setThemeState] = React.useState<VoidTheme>(forced ?? initialTheme());

  React.useEffect(() => {
    injectVoidTheme();
  }, []);

  const setTheme = React.useCallback((next: VoidTheme) => {
    setThemeState(next);
    try {
      window.localStorage.setItem(VOID_THEME_STORAGE_KEY, next);
    } catch {
      /* storage unavailable — theme simply won't persist */
    }
  }, []);

  const toggleTheme = React.useCallback(() => {
    setThemeState((t) => {
      const next = t === 'dark' ? 'daylight' : 'dark';
      try {
        window.localStorage.setItem(VOID_THEME_STORAGE_KEY, next);
      } catch {
        /* noop */
      }
      return next;
    });
  }, []);

  const value = React.useMemo(
    () => ({ theme, setTheme, toggleTheme }),
    [theme, setTheme, toggleTheme],
  );

  return (
    <VoidThemeContext.Provider value={value}>
      <div
        className={className ? `void-studio ${className}` : 'void-studio'}
        data-void-theme={theme}
        style={style}
      >
        {children}
      </div>
    </VoidThemeContext.Provider>
  );
}

export function useVoidTheme(): VoidThemeContextValue {
  return React.useContext(VoidThemeContext);
}
