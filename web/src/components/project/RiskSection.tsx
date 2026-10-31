// Project page risks tab: server-measured risk tiles and zones with a re-measure action.
import { useEffect, useState } from 'react';
import { getProjectRisks, type RiskReport, type RiskSignal, type RiskZone } from '../../lib/api/risks';
import { Badge, type BadgeTone } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Section } from '../ui/Section';
import { Tile } from '../ui/Tile';
import { ActivityIcon, AlertIcon, RefreshIcon, ReviewIcon, SpinnerIcon } from '../Icons';

const SIGNAL_META: Record<RiskSignal, { tone: BadgeTone; label: (zone: RiskZone) => string }> = {
  unreviewed: { tone: 'accent', label: () => 'jamais relu' },
  untested: { tone: 'neutral', label: () => 'sans tests' },
  churn: { tone: 'neutral', label: (zone) => `${zone.churn} commits` },
};

export function RiskSection({ projectId }: { projectId: string }) {
  const [data, setData] = useState<RiskReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [measuring, setMeasuring] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);

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
          data.zones.length > 0 ? (
            data.zones.map((zone, index) => (
              <div
                key={zone.path}
                className={`flex flex-wrap items-center gap-3 px-4 py-2.5 text-sm${index > 0 ? ' border-t border-border' : ''}`}
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
            ))
          ) : (
            <div className="px-4 py-3 text-sm text-muted">Aucune zone à risque.</div>
          )
        ) : null}
      </Section>
    </div>
  );
}
