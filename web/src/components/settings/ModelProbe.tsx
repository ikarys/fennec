import { useState } from 'react';
import { probeModel, type LimitSource, type ProbedModel } from '../../lib/api';
import { Button } from '../ui/Button';
import { SpinnerIcon, WandIcon } from '../ui/Icons';
import type { ModelForm } from '../../lib/settingsForm';
import { useLang, useT } from '../../i18n';
import { t, type Lang } from '../../i18n/messages';

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

function report(lang: Lang, probed: ProbedModel): string {
  const found = [
    probed.contextWindow != null && probed.contextSource
      ? t(lang, 'probeContextFound', { window: probed.contextWindow, source: SOURCE_LABELS[probed.contextSource] })
      : t(lang, 'probeContextNotFound'),
    probed.maxTokens != null ? t(lang, 'probeMaxTokens', { max: probed.maxTokens }) : null,
    probed.reasoningEfforts.length > 0 && probed.effortsSource
      ? t(lang, 'probeEfforts', { efforts: probed.reasoningEfforts.join('/'), source: SOURCE_LABELS[probed.effortsSource] })
      : t(lang, 'probeNoEffort')
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
  const t = useT();
  const lang = useLang();
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ text: string; failed: boolean } | null>(null);

  async function probe() {
    setBusy(true);
    setMessage(null);
    try {
      const probed = await probeModel(providerId, modelId);
      onPatch(formPatch(probed));
      setMessage({ text: report(lang, probed), failed: false });
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
        title={busy ? t('probing') : t('probeTitle')}
        onClick={() => void probe()}
      >
        {busy ? <SpinnerIcon className="h-4 w-4" /> : <WandIcon className="h-4 w-4" />}
      </Button>
      {message && (
        <span className={`text-xs ${message.failed ? 'text-err' : 'text-muted'}`}>
          {message.text}
          {!message.failed && <span className="text-faint">{t('probeIndicativeSuffix')}</span>}
        </span>
      )}
    </div>
  );
}
