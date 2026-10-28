import { useState } from 'react';
import type { ProviderListing } from '../../lib/api';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { FieldRow, Select, TextInput } from '../ui/Field';

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
  onChange: (index: number, patch: Partial<ProviderForm>) => void;
  onAdd: (provider: ProviderForm) => void;
  onRemove: (index: number) => void;
  onFetch: (provider: ProviderForm) => Promise<void>;
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
  onChange,
  onAdd,
  onRemove,
  onFetch
}: ProvidersSectionProps) {
  const [draft, setDraft] = useState<ProviderDraft>(emptyDraft);
  const [fetchingId, setFetchingId] = useState<string | null>(null);
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
  }

  return (
    <Card className="p-4">
      <div className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Providers</h2>

        {providers.map((provider, index) => {
          const listing = listings.find((item) => item.provider === provider.id);
          const isLocal = provider.id === 'local';

          return (
            <div key={provider.id} className="flex flex-col gap-4 border-t border-border pt-4">
              <div className="flex items-center gap-3">
                <h3 className="font-mono text-sm font-medium">{provider.name || provider.id}</h3>
                <Button
                  variant="secondary"
                  size="sm"
                  className="ml-auto"
                  disabled={fetchingId === provider.id}
                  onClick={async () => {
                    setFetchingId(provider.id);
                    try {
                      await onFetch(provider);
                    } catch {
                      // A failed fetch leaves the listing as it was.
                    } finally {
                      setFetchingId(null);
                    }
                  }}
                >
                  {fetchingId === provider.id ? 'Récupération…' : 'Récupérer'}
                </Button>
                {!isLocal && (
                  <Button variant="danger" size="sm" onClick={() => onRemove(index)}>
                    Retirer
                  </Button>
                )}
              </div>

              <div className="grid gap-3 md:grid-cols-2">
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

              {listing ? (
                listing.error ? (
                  <p className="m-0 text-xs text-err">{listing.error}</p>
                ) : (
                  <p className="m-0 text-xs text-muted">{listing.models.length} modèle(s) listé(s)</p>
                )
              ) : (
                <p className="m-0 text-xs text-muted">Récupérer pour lister ses modèles.</p>
              )}
            </div>
          );
        })}

        <div className="flex flex-col gap-3 border-t border-border pt-4">
          <h3 className="text-sm font-medium">Ajouter un provider</h3>
          <div className="grid gap-3 md:grid-cols-3">
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
          <div className="flex items-center gap-3">
            <Button variant="secondary" size="sm" disabled={draftDisabled} onClick={addProvider}>
              Ajouter
            </Button>
            <p className="m-0 text-xs text-muted">Ajouter, puis « Récupérer » pour lister ses modèles.</p>
          </div>
        </div>
      </div>
    </Card>
  );
}
