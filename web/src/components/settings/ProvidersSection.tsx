import { useState } from 'react';
import type { ProviderListing } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Section, type SectionSave } from '../ui/Section';
import { ChevronRightIcon, CloseIcon, PlusIcon, RefreshIcon, SpinnerIcon, TrashIcon } from '../Icons';
import { FieldRow, Select, TextInput } from '../ui/Field';
import { formOnlyModels, type ModelForm } from './ModelPrefsSection';

export interface ProviderForm {
  id: string;
  name: string;
  baseUrl: string;
  keyKind: 'none' | 'env' | 'stored';
  envVar: string;
  newKey: string;
  sealed: string | null;
}

interface ProvidersSectionProps {
  providers: ProviderForm[];
  listings: ProviderListing[];
  /** The form's per-model entries: those absent from a listing were added by hand. */
  values: Record<string, ModelForm>;
  onChange: (index: number, patch: Partial<ProviderForm>) => void;
  onAdd: (provider: ProviderForm) => void;
  onRemove: (index: number) => void;
  onFetch: (provider: ProviderForm) => Promise<void>;
  /** The group's own save: its dot and its "Enregistrer". */
  save?: SectionSave;
  onAddModel: (providerId: string, modelId: string) => void;
}

interface ProviderDraft {
  id: string;
  name: string;
  baseUrl: string;
}

const emptyDraft: ProviderDraft = { id: '', name: '', baseUrl: '' };

export function ProvidersSection({
  providers,
  listings,
  values,
  onChange,
  onAdd,
  onRemove,
  onFetch,
  save,
  onAddModel
}: ProvidersSectionProps) {
  const [draft, setDraft] = useState<ProviderDraft>(emptyDraft);
  const [fetchingId, setFetchingId] = useState<string | null>(null);
  const [modelDrafts, setModelDrafts] = useState<Record<string, string>>({});
  const [addingProvider, setAddingProvider] = useState(false);
  const [addingModelFor, setAddingModelFor] = useState<string | null>(null);
  const draftId = draft.id.trim();
  const draftDisabled =
    draftId === '' || draftId.includes('/') || providers.some((provider) => provider.id === draftId);

  function addProvider() {
    if (draftDisabled) return;
    onAdd({
      id: draftId,
      name: draft.name.trim(),
      baseUrl: draft.baseUrl.trim(),
      keyKind: 'none',
      envVar: '',
      newKey: '',
      sealed: null
    });
    setDraft(emptyDraft);
    setAddingProvider(false);
  }

  function addModel(providerId: string) {
    const modelId = (modelDrafts[providerId] ?? '').trim();
    if (modelId === '') return;
    onAddModel(providerId, modelId);
    setModelDrafts((previous) => ({ ...previous, [providerId]: '' }));
  }

  /** Header buttons must not fold the provider they sit in. */
  function keepOpen(event: React.MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
  }

  return (
    <Section
      title="Providers"
      save={save}
      actions={
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          aria-label="Ajouter un provider"
          title="Ajouter un provider"
          onClick={() => setAddingProvider((current) => !current)}
        >
          <PlusIcon className="h-3.5 w-3.5" />
        </Button>
      }
    >
      {addingProvider && (
        <Card variant="inset">
          <div className="grid gap-2 md:grid-cols-3">
            <FieldRow label="Identifiant" hint="Sans '/'">
              <TextInput
                value={draft.id}
                onChange={(event) => setDraft({ ...draft, id: event.target.value })}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label="Nom">
              <TextInput
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                className="w-full"
              />
            </FieldRow>
            <FieldRow label="URL de base">
              <TextInput
                value={draft.baseUrl}
                onChange={(event) => setDraft({ ...draft, baseUrl: event.target.value })}
                className="w-full"
              />
            </FieldRow>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="secondary" size="sm" disabled={draftDisabled} onClick={addProvider}>
              Ajouter
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setAddingProvider(false)}>
              Annuler
            </Button>
            <p className="m-0 text-xs text-faint">Ajouter, puis « Récupérer » pour lister ses modèles.</p>
          </div>
        </Card>
      )}

      <div className="flex flex-col">
        {providers.map((provider, index) => {
          const listing = listings.find((item) => item.provider === provider.id);
          const isLocal = provider.id === 'local';
          const error = listing?.error ?? null;
          const added = formOnlyModels(values, provider.id, listing);
          const count = (listing?.models.length ?? 0) + added.length;
          const modelDraft = modelDrafts[provider.id] ?? '';

          return (
            <div key={provider.id} className="border-t border-border first:border-t-0">
              <details
                // A failed listing opens by itself: the error is why the human came here.
                open={error ? true : undefined}
                className="group"
              >
                <summary className="-mx-2 flex h-9 cursor-pointer list-none items-center gap-2 rounded px-2 select-none hover:bg-raised [&::-webkit-details-marker]:hidden">
                  <ChevronRightIcon className="h-3 w-3 shrink-0 text-muted transition-transform group-open:rotate-90" />
                  <span className="min-w-0 truncate font-mono text-sm font-medium">
                    {provider.name || provider.id}
                  </span>
                  {error ? (
                    <Badge tone="err" title={error}>
                      erreur
                    </Badge>
                  ) : (
                    <Badge tone="neutral">{count === 0 ? 'aucun modèle' : `${count} modèle${count > 1 ? 's' : ''}`}</Badge>
                  )}
                  <span className="ml-auto flex shrink-0 items-center gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      iconOnly
                      aria-label="Ajouter un modèle"
                      title="Ajouter un modèle par son id (endpoint sans /models)"
                      onClick={(event) => {
                        keepOpen(event);
                        setAddingModelFor((current) => (current === provider.id ? null : provider.id));
                      }}
                    >
                      <PlusIcon className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      iconOnly
                      disabled={fetchingId === provider.id}
                      aria-label="Récupérer"
                      title={fetchingId === provider.id ? 'Récupération…' : 'Récupérer la liste des modèles (/models)'}
                      onClick={(event) => {
                        keepOpen(event);
                        void (async () => {
                          setFetchingId(provider.id);
                          try {
                            await onFetch(provider);
                          } catch {
                            // A failed fetch leaves the listing as it was.
                          } finally {
                            setFetchingId(null);
                          }
                        })();
                      }}
                    >
                      {fetchingId === provider.id ? <SpinnerIcon className="h-3.5 w-3.5" /> : <RefreshIcon className="h-3.5 w-3.5" />}
                    </Button>
                    {!isLocal && (
                      <Button
                        variant="ghost"
                        size="sm"
                        iconOnly
                        aria-label="Retirer"
                        title="Retirer ce provider"
                        onClick={(event) => { keepOpen(event); onRemove(index); }}
                      >
                        <TrashIcon className="h-3.5 w-3.5" />
                      </Button>
                    )}
                  </span>
                </summary>

                <div className="flex flex-col gap-3 pb-4 pt-1">
                  {error && <p className="m-0 text-xs text-err">{error}</p>}
                  <div className="grid gap-2 md:grid-cols-2">
                    <FieldRow label="Nom">
                      <TextInput
                        value={provider.name}
                        onChange={(event) => onChange(index, { name: event.target.value })}
                        className="w-full"
                      />
                    </FieldRow>

                    <FieldRow
                      label="URL de base"
                      hint={isLocal ? "Vide : l'URL de l'environnement (FENNEC_LLM_URL)." : undefined}
                    >
                      <TextInput
                        value={provider.baseUrl}
                        placeholder={isLocal ? listing?.baseUrl ?? '' : ''}
                        onChange={(event) => onChange(index, { baseUrl: event.target.value })}
                        className="w-full"
                      />
                    </FieldRow>

                    <FieldRow label="Clé API">
                      <Select
                        value={provider.keyKind}
                        onChange={(event) =>
                          onChange(index, { keyKind: event.target.value as ProviderForm['keyKind'] })
                        }
                        className="w-full"
                      >
                        <option value="none">Aucune</option>
                        <option value="env">Variable d'environnement</option>
                        <option value="stored">Clé enregistrée (chiffrée)</option>
                      </Select>
                    </FieldRow>

                    {provider.keyKind === 'env' && (
                      <FieldRow label="Variable d'environnement">
                        <TextInput
                          value={provider.envVar}
                          placeholder="OPENROUTER_API_KEY"
                          onChange={(event) => onChange(index, { envVar: event.target.value })}
                          className="w-full"
                        />
                      </FieldRow>
                    )}

                    {provider.keyKind === 'stored' && (
                      <FieldRow label="Nouvelle clé">
                        <TextInput
                          type="password"
                          autoComplete="off"
                          value={provider.newKey}
                          placeholder={
                            provider.sealed
                              ? "Une clé chiffrée est enregistrée : saisir pour la remplacer"
                              : "Coller la clé"
                          }
                          onChange={(event) => onChange(index, { newKey: event.target.value })}
                          className="w-full"
                        />
                      </FieldRow>
                    )}
                  </div>

                  {added.length > 0 && (
                    <p className="m-0 text-xs text-muted">
                      Ajouté{added.length > 1 ? 's' : ''} : <span className="font-mono">{added.join(', ')}</span> — « Enregistrer » pour garder.
                    </p>
                  )}
                </div>
              </details>
              {/* Not a <form>: the Settings page is one, and a nested submit
                  bubbles to it, saving the page before the model is added. */}
              {addingModelFor === provider.id && (
                <div className="flex items-center gap-2 pb-3 pl-5">
                  <TextInput
                    autoFocus
                    value={modelDraft}
                    onChange={(event) => setModelDrafts((previous) => ({ ...previous, [provider.id]: event.target.value }))}
                    onKeyDown={(event) => {
                      if (event.key !== 'Enter') return;
                      event.preventDefault();
                      addModel(provider.id);
                    }}
                    placeholder="Id du modèle (endpoint sans /models)"
                    className="w-full max-w-xs"
                  />
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    iconOnly
                    disabled={modelDraft.trim() === ''}
                    aria-label="Ajouter"
                    title="Ajouter le modèle"
                    onClick={() => addModel(provider.id)}
                  >
                    <PlusIcon className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    iconOnly
                    aria-label="Annuler"
                    title="Annuler"
                    onClick={() => setAddingModelFor(null)}
                  >
                    <CloseIcon className="h-3.5 w-3.5" />
                  </Button>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </Section>
  );
}
