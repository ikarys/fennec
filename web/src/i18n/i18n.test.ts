import { describe, expect, it } from 'vitest';
import { t } from './messages';

describe('t', () => {
  it('returns the english value for en', () => {
    expect(t('en', 'loading')).toBe('Loading…');
  });

  it('returns the french value for fr', () => {
    expect(t('fr', 'loading')).toBe('Chargement…');
  });

  it('interpolates {var} placeholders', () => {
    expect(t('en', 'integrateInto', { branch: 'main' })).toBe('Integrate into main');
    expect(t('fr', 'integrateInto', { branch: 'main' })).toBe('Intégrer dans main');
  });

  it('interpolates numbers as well', () => {
    expect(t('en', 'integrateInto', { branch: 42 })).toBe('Integrate into 42');
  });

  it('keeps a placeholder untouched when its var is missing', () => {
    expect(t('en', 'integrateInto', {})).toBe('Integrate into {branch}');
  });

  it('returns the string untouched when no vars are passed', () => {
    expect(t('fr', 'integrateInto')).toBe('Intégrer dans {branch}');
  });
});
