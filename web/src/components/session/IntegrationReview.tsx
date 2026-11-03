// What the integration dialog shows of a branch before the human decides: its commits, its files in
// the review order with their signals, its criteria and the decisions the agent recorded.
import type { IntegrationPreview, ReviewSignal } from '../../lib/api';
import { DecisionList } from './DecisionList';
import { CriterionProvenanceBadge, PassedBeforeBadge, criterionDotClass } from './CriteriaSection';
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
    label: 'sans test',
    tone: 'accent',
    title: 'Aucun test en ligne et aucun fichier de test du même nom sur la branche.'
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
          <details className="text-xs">
            <summary className="cursor-pointer select-none text-muted hover:text-text">
              {preview.commits.length === 1
                ? "1 commit de l'agent, repris tel quel sur ta branche"
                : `${preview.commits.length} commits de l'agent, écrasés en un seul sur ta branche`}
            </summary>
            <div className="mt-1 flex flex-col">
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
          Fichiers{preview.files.length > 0 && <span className="ml-1 tabular-nums">{preview.files.length}</span>}
          {preview.files.length > 0 && (
            <span className="ml-2 text-xs text-muted">ordre de relecture · vérifié</span>
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
          <>
            {preview.commonSignals.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5 text-xs text-muted">
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
            <div className="flex flex-col">
              {preview.files.map((file) => (
                <div key={file.path} className="flex items-center gap-2 py-1 font-mono text-xs">
                  <span className="grow overflow-hidden text-left text-ellipsis whitespace-nowrap" title={file.path}>
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
          </>
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
