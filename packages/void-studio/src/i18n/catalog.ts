// Localizable copy plumbing (W11, PRO-06).
//
// One flat catalog of dotted keys -> message strings, resolved through a
// translator function. No i18n library dependency: the surface we need is
// lookup + `{param}` interpolation + a visible missing-key fallback.
// Catalogs are plain data so additional locales are added as files
// (`en.ts` ships; new locales mirror its keys).

/** A flat key -> message catalog. `{name}` placeholders interpolate. */
export type I18nCatalog = Readonly<Record<string, string>>;

/** Translate a key; interpolate `{param}` placeholders. */
export type Translator = (key: string, params?: Record<string, string | number>) => string;

/** Sentinel marking a key the catalog does not define. */
export function isMissingKey(resolved: string, key: string): boolean {
  return resolved === `⟦${key}⟧`;
}

function interpolate(message: string, params?: Record<string, string | number>): string {
  if (!params) return message;
  let out = message;
  for (const [k, v] of Object.entries(params)) {
    out = out.split(`{${k}}`).join(String(v));
  }
  return out;
}

/**
 * Build a translator over `catalog` with `fallbackCatalog` consulted for
 * keys the primary catalog misses (locale layering: a partial `de` over
 * `en`). A key missing everywhere renders as `⟦key⟧` — visible in dev,
 * never silently empty.
 */
export function createTranslator(
  catalog: I18nCatalog,
  fallbackCatalog?: I18nCatalog,
): Translator {
  return (key, params) => {
    const message = catalog[key] ?? fallbackCatalog?.[key];
    if (message === undefined) return `⟦${key}⟧`;
    return interpolate(message, params);
  };
}

/** Keys present in `required` but absent from `catalog` (catalog QA). */
export function missingKeys(catalog: I18nCatalog, required: readonly string[]): string[] {
  return required.filter((k) => catalog[k] === undefined);
}
