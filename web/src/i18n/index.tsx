import { createContext, useContext } from 'react';
import type { ReactNode } from 'react';
import { t, type Lang, type MsgKey } from './messages';

const I18nContext = createContext<Lang>('en');

export function I18nProvider({ lang, children }: { lang: Lang; children: ReactNode }) {
  return <I18nContext.Provider value={lang}>{children}</I18nContext.Provider>;
}

/**
 * Translate a key in the UI's current language: `const t = useT(); t('loading')`.
 * Bound to the language chosen on the Settings page (default `en`).
 */
export function useT(): (key: MsgKey, vars?: Record<string, string | number>) => string {
  const lang = useContext(I18nContext);
  return (key, vars) => t(lang, key, vars);
}

/** The UI's current language, for the pure lib functions that take a `lang` argument. */
export function useLang(): Lang {
  return useContext(I18nContext);
}
