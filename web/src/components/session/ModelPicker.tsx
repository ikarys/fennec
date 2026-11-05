import { useCallback, useRef, useState } from 'react';
import type { ModelChoice, ModelChoices, ModelRef, ProviderListing } from '../../lib/api';
import { getSettings } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { SectionLabel } from '../ui/SectionLabel';
import { TextInput } from '../ui/Field';
import { useDismissable } from '../../hooks/useDismissable';
import { useT } from '../../i18n';

/** The model id without its vendor prefix (`org/model` → `model`). */
export function modelShortName(model: string): string {
  const slash = model.lastIndexOf('/');
  return slash === -1 ? model : model.slice(slash + 1);
}

interface ModelPickerProps {
  current: ModelChoice | null;
  choices: ModelChoices | null;
  onChange: (ref: ModelRef) => void;
  disabled?: boolean;
}

function sameRef(a: ModelRef, b: ModelRef): boolean {
  return a.provider === b.provider && a.model === b.model;
}

/**
 * The session's model in the composer: favourites and the default first, every
 * provider's models on demand (listing them calls each provider).
 */
export function ModelPicker({ current, choices, onChange, disabled }: ModelPickerProps) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState('');
  const [allModels, setAllModels] = useState<ProviderListing[] | null>(null);
  const [loadingAll, setLoadingAll] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const close = useCallback(() => {
    setOpen(false);
    setFilter('');
  }, []);
  useDismissable(open, containerRef, close);

  function choose(ref: ModelRef) {
    onChange(ref);
    close();
  }

  const query = filter.trim().toLowerCase();

  function matches(choice: ModelChoice): boolean {
    if (query === '') return true;
    return choice.model.toLowerCase().includes(query) || choice.providerName.toLowerCase().includes(query);
  }

  const favorites = choices ? choices.favorites.filter(matches) : [];
  const currentIsFavorite = current && choices ? choices.favorites.some((favorite) => sameRef(favorite, current)) : false;
  const showCurrentFirst = current && !currentIsFavorite && matches(current);
  // The default is always one click away, even when it is not a favourite.
  const showDefault =
    choices &&
    !choices.favorites.some((favorite) => sameRef(favorite, choices.default)) &&
    !(current && sameRef(current, choices.default)) &&
    matches(choices.default);

  async function loadAll() {
    if (allModels !== null || loadingAll) return;
    setLoadingAll(true);
    try {
      const response = await getSettings();
      setAllModels(response.listings);
    } catch {
      // Keep `allModels` null so the human can try again.
    } finally {
      setLoadingAll(false);
    }
  }

  function entry(choice: ModelChoice, isCurrent: boolean) {
    const isDefault = choices ? sameRef(choices.default, choice) : false;
    return (
      <Button
        key={`${choice.provider}/${choice.model}`}
        variant="ghost"
        size="sm"
        multiline
        className="w-full"
        onClick={() => choose({ provider: choice.provider, model: choice.model })}
      >
        <span className="flex w-full items-center gap-1.5">
          <span className="w-3 shrink-0 text-left">{isCurrent ? '✓' : ''}</span>
          <span className="min-w-0 grow">{modelShortName(choice.model)}</span>
          {isDefault && <Badge tone="neutral">{t('defaultBadge')}</Badge>}
        </span>
      </Button>
    );
  }

  return (
    <div ref={containerRef} className="relative">
      <Button
        variant="ghost"
        size="sm"
        disabled={disabled}
        title={current ? `${current.providerName} · ${current.model}` : undefined}
        onClick={() => setOpen((value) => !value)}
      >
        {current ? modelShortName(current.model) : '…'}
      </Button>
      {open && (
        <Card className="absolute bottom-full right-0 z-10 mb-1 w-80 max-h-80 overflow-y-auto p-2">
          <div className="flex flex-col gap-2">
            <TextInput
              autoFocus
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder={t('filter')}
              className="w-full"
            />
            {showCurrentFirst && current && entry(current, true)}
            {showDefault && entry(choices.default, false)}
            {favorites.length > 0 && (
              <div className="flex flex-col gap-1">
                <SectionLabel>{t('favorites')}</SectionLabel>
                {favorites.map((favorite) => entry(favorite, current ? sameRef(favorite, current) : false))}
              </div>
            )}
            <Button variant="ghost" size="sm" className="w-full" onClick={loadAll} disabled={loadingAll}>
              {loadingAll ? t('loading') : t('allModels')}
            </Button>
            {allModels && allModels.length > 0 && (
              <div className="flex flex-col gap-2">
                {allModels.map((listing) => {
                  const models = listing.models.filter((model) => {
                    if (query === '') return true;
                    return model.id.toLowerCase().includes(query) || listing.name.toLowerCase().includes(query);
                  });
                  if (models.length === 0) return null;
                  return (
                    <div key={listing.provider} className="flex flex-col gap-1">
                      <SectionLabel>{listing.name}</SectionLabel>
                      {models.map((model) => {
                        const choice: ModelChoice = {
                          provider: listing.provider,
                          model: model.id,
                          providerName: listing.name
                        };
                        return entry(choice, current ? sameRef(choice, current) : false);
                      })}
                    </div>
                  );
                })}
              </div>
            )}
            {allModels && allModels.length === 0 && <p className="m-0 text-xs text-muted">{t('noModelsListed')}</p>}
          </div>
        </Card>
      )}
    </div>
  );
}
