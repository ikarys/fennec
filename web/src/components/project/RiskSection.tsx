// Project page risks tab: server-measured risk tiles and zones with a re-measure action.
import { useEffect, useState } from 'react';
import { getProjectRisks, type RiskReport, type RiskSignal, type RiskZone } from '../../lib/api/risks';
import { Badge, type BadgeTone } from '../ui/Badge';
import { Button } from '../ui/Button';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { Section } from '../ui/Section';
import { Tile } from '../ui/Tile';
import { ActivityIcon, AlertIcon, RefreshIcon, ReviewIcon, SpinnerIcon } from '../Icons';

const SIGNAL_META: Record<RiskSignal, { tone: BadgeTone; label: (zone: RiskZone) => string }> = {
  unreviewed: { tone: 'accent', label: () => 'jamais relu' },
  untested: { tone: 'neutral', label: () => 'sans tests' },
  churn: { tone: 'neutral', label: (zone) => `${zone.churn} commits` },
};

export function RiskSection({
  projectId,
  busy,
  onStart,
}: {
  projectId: string;
  /** A session is being created: the launch button waits. */
  busy: boolean;
  onStart: (prompt: string, mode: string) => void;
}) {
  const [data, setData] = useState<RiskReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [measuring, setMeasuring] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setData(null);
    setError(null);
    setMeasuring(true);
    getProjectRisks(projectId)
      .then((report) => {
        if (!cancelled) {
          setData(report);
          setMeasuring(false);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled) {
          setError(loadError instanceof Error ? loadError.message : String(loadError));
          setMeasuring(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, reloadKey]);

  const refresh = () => {
    setReloadKey((key) => key + 1);
  };

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
        <Tile
          icon={<ReviewIcon />}
          label="Jamais relu"
          figure={data ? `${data.unreviewedPercent} %` : '…'}
          detail={data ? `des lignes · ${data.unreviewed} fichiers entiers` : '…'}
          title="Part des lignes (git blame) venant de commits IA jamais intégrés par fennec"
          tone="accent"
          active={data ? data.unreviewed > 0 : false}
        />
        <Tile
          icon={<AlertIcon />}
          label="Sans tests"
          figure={data ? `${data.untested}` : '…'}
          detail={data ? `fichiers sur ${data.files}` : '…'}
          tone="neutral"
        />
        <Tile
          icon={<ActivityIcon />}
          label="Souvent modifiés"
          figure={data ? `${data.churning}` : '…'}
          detail={data ? `5 commits ou plus en ${data.churnDays} jours` : '…'}
          tone="neutral"
        />
      </div>
      {error ? <div className="px-4 pb-2 text-err text-xs">{error}</div> : null}
      <Section
        title="Zones à risque"
        flush
        description="Un commit IA non relu porte un co-auteur IA (Co-Authored-By) sans trailer Fennec-Session : l'intégration par fennec est la revue humaine."
        actions={
          <>
            <Button
              variant="primary"
              size="sm"
              disabled={busy || !data?.remedyPrompt}
              onClick={() => setConfirming(true)}
              title="Proposer des tâches pour ces zones"
            >
              Remédier
            </Button>
            <Badge tone="neutral" title="Calculé depuis git, sans modèle">
              vérifié
            </Badge>
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              title="Mesurer à nouveau"
              aria-label="Mesurer à nouveau"
              disabled={measuring}
              onClick={refresh}
            >
              {measuring ? <SpinnerIcon className="h-3.5 w-3.5" /> : <RefreshIcon className="h-3.5 w-3.5" />}
            </Button>
          </>
        }
      >
        {data ? (
          data.modules.length > 0 ? (
            data.modules.map((module, index) => (
              <details key={module.path} className={index > 0 ? 'border-t border-border' : ''}>
                <summary className="flex cursor-pointer items-center gap-3 px-4 py-2.5 text-sm list-none [&::-webkit-details-marker]:hidden">
                  <span className="min-w-0 flex-1 truncate font-mono text-xs" title={module.path}>
                    {module.path}
                  </span>
                  {module.unreviewed > 0 ? (
                    <Badge tone="accent">
                      {module.unreviewed} {module.unreviewed > 1 ? 'jamais relus' : 'jamais relu'}
                    </Badge>
                  ) : null}
                  {module.untested > 0 ? <Badge tone="neutral">{module.untested} sans tests</Badge> : null}
                  {module.churning > 0 ? (
                    <Badge tone="neutral">
                      {module.churning} {module.churning > 1 ? 'souvent modifiés' : 'souvent modifié'}
                    </Badge>
                  ) : null}
                  <span className="text-xs text-muted tabular-nums">
                    {module.zones.length} {module.zones.length > 1 ? 'fichiers' : 'fichier'}
                  </span>
                </summary>
                {module.zones.map((zone) => (
                  <div
                    key={zone.path}
                    className="flex flex-wrap items-center gap-3 border-t border-border py-2.5 pr-4 pl-8 text-sm"
                  >
                    <span className="min-w-0 flex-1 truncate font-mono text-xs" title={zone.path}>
                      {zone.path}
                    </span>
                    {zone.signals.map((signal) => (
                      <Badge key={signal} tone={SIGNAL_META[signal].tone}>
                        {SIGNAL_META[signal].label(zone)}
                      </Badge>
                    ))}
                    <span className="text-xs text-muted tabular-nums">{zone.lines} lignes</span>
                  </div>
                ))}
              </details>
            ))
          ) : (
            <div className="px-4 py-3 text-sm text-muted">Aucune zone à risque.</div>
          )
        ) : null}
      </Section>
      <ConfirmDialog
        open={confirming}
        title="Remédier ?"
        body="Un agent en lecture seule lit les modules les plus risqués et propose quelques tâches (5 au plus), les plus risqués d'abord ; les fichiers sans logique sont écartés. Tu choisis celles qui entrent au backlog. Relire reste ton travail : il te dit quels fichiers lire d'abord."
        confirmLabel="Lancer"
        variant="primary"
        onConfirm={() => {
          if (!data) return;
          setConfirming(false);
          onStart(data.remedyPrompt, 'remedy');
        }}
        onCancel={() => setConfirming(false)}
      />
    </div>
  );
}
