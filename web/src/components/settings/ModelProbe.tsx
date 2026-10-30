import { useState } from 'react';
import { probeModel, type LimitSource, type ProbedModel } from '../../lib/api';
import { Button } from '../ui/Button';
import { SpinnerIcon, WandIcon } from '../Icons';
import type { ModelForm } from './ModelPrefsSection';

const SOURCE_LABELS: Record<LimitSource, string> = { endpoint: 'endpoint', catalog: 'catalogue' };

/** What a probe fills in the form: only what the endpoint or the catalog said. */
function formPatch(probed: ProbedModel): Partial<ModelForm> {
  const patch: Partial<ModelForm> = {};
  if (probed.contextWindow != null) patch.contextWindow = String(probed.contextWindow);
  if (probed.maxTokens != null) patch.maxTokens = String(probed.maxTokens);
  // No accepted effort: the model must not be sent one.
  patch.reasoningEffort = probed.defaultReasoningEffort ?? '';
  return patch;
}

function report(probed: ProbedModel): string {
  const found = [
    probed.contextWindow != null && probed.contextSource
      ? `contexte ${probed.contextWindow} (${SOURCE_LABELS[probed.contextSource]})`
      : 'contexte non trouvé',
    probed.maxTokens != null ? `max ${probed.maxTokens} (endpoint)` : null,
    probed.reasoningEfforts.length > 0 && probed.effortsSource
      ? `efforts ${probed.reasoningEfforts.join('/')} (${SOURCE_LABELS[probed.effortsSource]})`
      : 'aucun effort accepté'
  ].filter((part) => part !== null);
  return found.join(' · ');
}

/**
 * "Auto": probes the model at its stored endpoint and fills the form with
 * the limits found. Indicative: read from the endpoint's error messages.
 */
export function ModelProbe({
  providerId,
  modelId,
  onPatch
}: {
  providerId: string;
  modelId: string;
  onPatch: (patch: Partial<ModelForm>) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ text: string; failed: boolean } | null>(null);

  async function probe() {
    setBusy(true);
    setMessage(null);
    try {
      const probed = await probeModel(providerId, modelId);
      onPatch(formPatch(probed));
      setMessage({ text: report(probed), failed: false });
    } catch (error) {
      setMessage({ text: error instanceof Error ? error.message : String(error), failed: true });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-2">
      <Button
        variant="secondary"
        size="sm"
        iconOnly
        disabled={busy}
        aria-label="Auto"
        title={busy ? 'Sondage…' : "Quelques appels d'un token au modèle (endpoint enregistré) : efforts acceptés, limites nommées par l'endpoint, sinon le catalogue des familles connues"}
        onClick={() => void probe()}
      >
        {busy ? <SpinnerIcon className="h-4 w-4" /> : <WandIcon className="h-4 w-4" />}
      </Button>
      {message && (
        <span className={`text-xs ${message.failed ? 'text-err' : 'text-muted'}`}>
          {message.text}
          {!message.failed && <span className="text-faint"> — indicatif, « Enregistrer » pour garder</span>}
        </span>
      )}
    </div>
  );
}
