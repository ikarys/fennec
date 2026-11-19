// The default-model menu's groups: the environment default first, each
// provider's models under its name, a stored model the providers no longer list.
import { describe, expect, it } from 'vitest';
import { t, type MsgKey } from '../../i18n/messages';
import { modelMenuGroups } from './ModelSection';

const translate = (key: MsgKey, vars?: Record<string, string | number>) => t('en', key, vars);

const listings = [
  {
    provider: 'databrick',
    name: 'Databrick',
    models: [{ id: 'system.ai.deepseek-v4-1-flash-1017' }, { id: 'system.ai.glm-5-3-flash' }]
  },
  { provider: 'hominfer', name: 'Hominfer', models: [{ id: 'qwen3.8-27b' }] }
];

describe('modelMenuGroups', () => {
  it('puts the environment default first, named when a default is set', () => {
    const groups = modelMenuGroups(listings, null, 'qwen3.8-27b', translate);
    expect(groups[0].label).toBeUndefined();
    expect(groups[0].choices).toEqual([{ value: '', label: 'Environment default (qwen3.8-27b)' }]);
  });

  it('names the environment default alone when no default is set', () => {
    const groups = modelMenuGroups(listings, null, '', translate);
    expect(groups[0].choices).toEqual([{ value: '', label: 'Environment default' }]);
  });

  it('groups each provider\'s models under its name', () => {
    const groups = modelMenuGroups(listings, null, 'qwen3.8-27b', translate);
    expect(groups.slice(1)).toEqual([
      {
        label: 'Databrick',
        choices: [
          { value: 'databrick/system.ai.deepseek-v4-1-flash-1017', label: 'system.ai.deepseek-v4-1-flash-1017' },
          { value: 'databrick/system.ai.glm-5-3-flash', label: 'system.ai.glm-5-3-flash' }
        ]
      },
      { label: 'Hominfer', choices: [{ value: 'hominfer/qwen3.8-27b', label: 'qwen3.8-27b' }] }
    ]);
  });

  it('keeps a stored model the providers no longer list', () => {
    const groups = modelMenuGroups(listings, { provider: 'old', model: 'gpt-9' }, 'qwen3.8-27b', translate);
    expect(groups.at(-1)?.choices).toEqual([{ value: 'old/gpt-9', label: 'gpt-9 — old (not listed)' }]);
  });

  it('drops the unlisted entry when the model is listed again', () => {
    const groups = modelMenuGroups(listings, { provider: 'hominfer', model: 'qwen3.8-27b' }, 'qwen3.8-27b', translate);
    expect(groups).toHaveLength(3);
  });
});
