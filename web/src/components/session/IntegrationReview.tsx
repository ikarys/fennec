// What the integration dialog shows of a branch before the human decides: its commits, its files in
// the review order with their signals, its criteria and the decisions the agent recorded.
import type { IntegrationPreview, ReviewSignal } from '../../lib/api';
import { DecisionList } from './DecisionList';
import { CriterionProvenanceBadge, PassedBeforeBadge, criterionDotClass } from './CriteriaSection';
import { ChevronRightIcon } from '../Icons';
import { Button } from '../ui/Button';
import { Badge } from '../ui/Badge';
import type { BadgeTone } from '../ui/Badge';
import { SectionLabel } from '../ui/SectionLabel';

// `protected` has its own badge, shown before these.
const SIGNAL_LABEL: Record<Exclude<ReviewSignal, 'protected'>, { label: string; tone: BadgeTone; title: string }> = {
  sensitive: {
    label: 'zone sensible',
    tone: 'err',
    title: 'Fichier dans une zone sensible de .agents/architecture : ta revue est obligatoire.'
  },
  bypass: {
    label: 'contournement',
    tone: 'err',
    title: 'Les lignes ajoutées font taire un lint, un contrôle de types ou un test.'
  },
  architecture: {
    label: 'architecture',
    tone: 'err',
    title: "Le fichier viole une règle d'architecture."
  },
  conflictMarker: {
    label: 'marqueurs de conflit',
    tone: 'err',
    title: 'Des marqueurs de conflit git restent dans les lignes ajoutées.'
  },
  testsWeakened: {
    label: 'tests affaiblis',
    tone: 'err',
    title: 'Des tests ou assertions ont été affaiblis dans ce fichier.'
  },
  hot: {
    label: 'souvent modifié',
    tone: 'accent',
    title: 'Modifié 5 fois ou plus ces 90 derniers jours.'
  },
  untested: {
    label: 'sans test propre',
    tone: 'accent',
    title: 'Aucun test en ligne et aucun fichier de test du même nom sur la branche ; les modules de déclaration seule, le support de test et l’outillage de build ne comptent pas.'
  },
  large: {
    label: 'gros changement',
    tone: 'neutral',
    title: '200 lignes ou plus modifiées.'
  }
};


interface IntegrationReviewProps {
  preview: IntegrationPreview;
  /** The project the narrate agent runs in; without one the Raconter button is not offered. */
  projectId: string | null;
  narrating: boolean;
  onNarrate: () => void;
}

export function IntegrationReview({ preview, projectId, narrating, onNarrate }: IntegrationReviewProps) {
  return (
    <>
      <section className="flex flex-col gap-1.5">
        <SectionLabel>Commits</SectionLabel>
        {preview.commits.length === 0 ? (
          <div className="text-xs text-muted">Aucun commit.</div>
        ) : (
          <details className="group text-xs">
            <summary className="flex cursor-pointer list-none items-center gap-2 rounded-md px-1.5 py-1 select-none text-muted hover:bg-accent-soft hover:text-text [&::-webkit-details-marker]:hidden">
              <ChevronRightIcon className="h-3 w-3 shrink-0 transition-transform group-open:rotate-90" />
              {preview.commits.length === 1
                ? "1 commit de l'agent, repris tel quel sur ta branche"
                : `${preview.commits.length} commits de l'agent, écrasés en un seul sur ta branche`}
            </summary>
            <div className="flex flex-col pl-6">
              {preview.commits.map((commit) => (
                <div key={commit.sha} className="flex items-start gap-2 py-1 text-xs">
                  <span className="shrink-0 font-mono text-muted">{commit.sha.slice(0, 8)}</span>
                  <span className="grow break-words text-text">{commit.subject}</span>
                </div>
              ))}
            </div>
          </details>
        )}
      </section>

      <section className="flex flex-col gap-1.5">
        <SectionLabel className="flex items-center">
          Fichiers
          {preview.files.length > 0 && (
            <span className="ml-1 rounded border border-border bg-raised px-1.5 text-[11px] tabular-nums text-text">
              {preview.files.length}
            </span>
          )}
          {preview.files.length > 0 && (
            <>
              <span className="ml-2 text-xs text-muted">ordre de relecture ·</span>
              <Badge tone="ok" className="ml-1.5" title="Calculé par fennec à partir du journal">
                vérifié
              </Badge>
            </>
          )}
          {preview.files.length > 0 && projectId && (
            <Button
              variant="secondary"
              size="sm"
              className="ml-auto"
              title="Un agent en lecture seule raconte le changement fichier par fichier, dans cet ordre ; son récit est indicatif."
              disabled={narrating}
              onClick={onNarrate}
            >
              Raconter
            </Button>
          )}
        </SectionLabel>
        {preview.files.length === 0 ? (
          <div className="text-xs text-muted">Aucun fichier.</div>
        ) : (
          <div className="overflow-hidden rounded-lg border border-border bg-raised">
            {preview.commonSignals.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5 border-b border-border px-3 py-1.5 text-xs text-muted">
                <span>Tous les fichiers :</span>
                {preview.commonSignals.map((signal) =>
                  signal === 'protected' ? null : (
                    <Badge key={signal} tone={SIGNAL_LABEL[signal].tone} title={SIGNAL_LABEL[signal].title}>
                      {SIGNAL_LABEL[signal].label}
                    </Badge>
                  ),
                )}
              </div>
            )}
            <div className="divide-y divide-border">
              {preview.files.map((file) => (
                <div key={file.path} className="group flex items-center gap-2 px-3 py-2 font-mono text-xs transition-colors hover:bg-accent-soft">
                  <span
                    className="min-w-0 grow overflow-hidden text-left text-ellipsis whitespace-nowrap text-text group-hover:text-accent-strong"
                    title={file.path}
                  >
                    {file.path}
                  </span>
                  {file.protected && <Badge tone="err">protégé</Badge>}
                  {file.carried && (
                    <Badge
                      tone="accent"
                      title="Le fichier avait des modifications non commitées sur lesquelles l'agent a travaillé : elles entrent dans ce commit."
                    >
                      ton travail inclus
                    </Badge>
                  )}
                  {file.signals.map((signal) =>
                    signal === 'protected' || preview.commonSignals.includes(signal) ? null : (
                      <Badge key={signal} tone={SIGNAL_LABEL[signal].tone} title={SIGNAL_LABEL[signal].title}>
                        {SIGNAL_LABEL[signal].label}
                      </Badge>
                    ),
                  )}
                  <span className="shrink-0 tabular-nums text-ok">+{file.added}</span>
                  <span className="shrink-0 tabular-nums text-err">−{file.removed}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </section>

      <section className="flex flex-col gap-1.5">
        <SectionLabel>Critères</SectionLabel>
        {preview.criteria.length === 0 ? (
          <div className="text-xs text-muted">Aucun critère.</div>
        ) : (
          <div className="flex flex-col">
            {preview.criteria.map((criterion) => {
              const provenance = criterion.status.type === 'pending' ? undefined : criterion.status.provenance;
              const dotClass = criterionDotClass(criterion.status.type, provenance);
              return (
                <div key={criterion.id} className="flex items-start gap-2 py-1.5 text-xs">
                  <span aria-hidden className={`mt-1 h-2 w-2 shrink-0 rounded-full ${dotClass}`} />
                  <span className="shrink-0 font-mono text-muted">{criterion.id}</span>
                  <span className="grow break-words text-text">{criterion.description}</span>
                  <PassedBeforeBadge passedBefore={criterion.passedBefore} />
                  <CriterionProvenanceBadge provenance={provenance} />
                </div>
              );
            })}
          </div>
        )}
      </section>

      <section className="flex flex-col gap-1.5">
        <SectionLabel>Décisions</SectionLabel>
        <DecisionList decisions={preview.decisions} />
      </section>
    </>
  );
}
