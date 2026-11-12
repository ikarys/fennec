// The UI's user-visible strings, by language.
//
// `en` (`en.ts`) is the canonical dictionary: every key exists there, and `fr`
// (`fr.ts`) is typed against its keys so a missing translation is a compile
// error. `t` interpolates `{var}` placeholders and falls back to `en` on a
// missing entry.

import { en, type MsgKey } from './en';
import { fr } from './fr';

export type Lang = 'en' | 'fr';
export type { MsgKey };

const DICTS: Record<Lang, Record<MsgKey, string>> = { en, fr };

/** The UI's string in `lang`, with `{var}` interpolation; a missing entry falls back to `en`. */
export function t(lang: Lang, key: MsgKey, vars?: Record<string, string | number>): string {
  let text = DICTS[lang][key] ?? DICTS.en[key];
  if (vars) {
    // `{s}` pluralises after `{count}` unless the caller gives it: French
    // keeps 0 and 1 singular, English only 1.
    const count = vars.count;
    const all = typeof count === 'number' && !('s' in vars) ? { ...vars, s: count === 1 || (lang === 'fr' && count === 0) ? '' : 's' } : vars;
    text = text.replace(/\{(\w+)\}/g, (match, name) => (name in all ? String(all[name]) : match));
  }
  return text;
}