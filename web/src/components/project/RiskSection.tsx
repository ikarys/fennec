// Project page risks tab: server-measured risk tiles and zones with a re-measure action.
import { useEffect, useState } from 'react';
import { getProjectRisks, type RiskReport, type RiskSignal, type RiskZone } from '../../lib/api/risks';
import { Badge, type BadgeTone } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { Section } from '../ui/Section';
import { Tile } from '../ui/Tile';
import { ActivityIcon, AlertIcon, ChevronRightIcon, RefreshIcon, ReviewIcon, SpinnerIcon, WandIcon } from '../Icons';

// One colour per signal, shared by the tiles, the module columns and the file badges, so a colour reads as a signal everywhere.
const SIGNAL_META: Record<RiskSignal, { tone: BadgeTone; label: (zone: RiskZone) => string }> = {
  unreviewed: { tone: 'err', label: () => 'jamais relu' },
  untested: { tone: 'attention', label: () => 'sans tests' },
  churn: { tone: 'info', label: (zone) => `${zone.churn} commits` },
};

/** A module's count for one signal, in its fixed column; nothing when zero. */
function Count({ value, tone, title }: { value: number; tone: BadgeTone; title: string }) {
  return (
    <span className="w-24 shrink-0 text-right">
      {value > 0 ? (
        <Badge tone={tone} pill title={title}>
          {value}
        </Badge>
      ) : (
        <span className="text-xs text-faint">·</span>
      )}
    </span>
  );
}

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
  // The brief to remediate, global or one module's; null when no confirmation is open.
  const [confirming, setConfirming] = useState<string | null>(null);

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
          detail={data ? `des lignes · ${data.aiCommits} commits IA déclarés sur ${data.commits}` : '…'}
          title="Part des lignes (git blame) venant de commits IA jamais intégrés par fennec. Seuls les commits avec un co-auteur IA déclaré comptent : les autres passent pour humains."
          tone="err"
          active={data ? data.unreviewed > 0 : false}
        />
        <Tile
          icon={<AlertIcon />}
          label="Sans tests"
          figure={data ? `${data.untested}` : '…'}
          detail={data ? `fichiers sur ${data.files}` : '…'}
          tone="attention"
          active={data ? data.untested > 0 : false}
        />
        <Tile
          icon={<ActivityIcon />}
          label="Souvent modifiés"
          figure={data ? `${data.churning}` : '…'}
          detail={data ? `5 commits ou plus en ${data.churnDays} jours` : '…'}
          tone="info"
          active={data ? data.churning > 0 : false}
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
              onClick={() => data && setConfirming(data.remedyPrompt)}
              title="Proposer des tâches pour ces zones"
            >
              Remédier
            </Button>
            <Badge tone="ok" pill title="Calculé depuis git, sans modèle">
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
            <Card className="overflow-hidden">
              <div className="flex items-center gap-3 border-b border-border bg-raised px-4 py-2 text-xs uppercase tracking-wide text-faint">
                <span className="w-3.5 shrink-0" />
                <span className="min-w-0 grow">Module</span>
                <span className="w-24 shrink-0 text-right">Jamais relu</span>
                <span className="w-24 shrink-0 text-right">Sans tests</span>
                <span className="w-24 shrink-0 text-right">Modifiés</span>
                <span className="w-20 shrink-0 text-right">Fichiers</span>
                <span className="w-7 shrink-0" />
              </div>
              {data.modules.map((module, index) => (
                <details key={module.path} className={`group ${index > 0 ? 'border-t border-border' : ''}`}>
                  {/* An open module keeps the accent edge, so its files read as its own. */}
                  <summary className="group/row relative flex cursor-pointer list-none items-center gap-3 px-4 py-2.5 transition-colors before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-accent before:opacity-0 hover:bg-accent-soft hover:before:opacity-100 group-open:bg-accent-soft group-open:before:opacity-100 [&::-webkit-details-marker]:hidden">
                    <ChevronRightIcon className="h-3.5 w-3.5 shrink-0 text-faint transition-transform group-open:rotate-90 group-open:text-accent" />
                    <span className="min-w-0 grow truncate font-mono text-xs text-text group-open:text-accent-strong" title={module.path}>
                      {module.path}
                    </span>
                    <Count value={module.unreviewed} tone={SIGNAL_META.unreviewed.tone} title="Fichiers jamais relus" />
                    <Count value={module.untested} tone={SIGNAL_META.untested.tone} title="Fichiers sans tests" />
                    <Count value={module.churning} tone={SIGNAL_META.churn.tone} title="Fichiers souvent modifiés" />
                    <span className="w-20 shrink-0 text-right font-mono text-xs text-muted tabular-nums">{module.zones.length}</span>
                    {/* Hidden until hover so the counts stay the row's reading; the summary must not toggle on its click. */}
                    <Button
                      variant="soft"
                      size="sm"
                      iconOnly
                      className="opacity-0 group-hover/row:opacity-100 focus-visible:opacity-100"
                      title="Remédier ce module"
                      aria-label={`Remédier ${module.path}`}
                      disabled={busy}
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
                        setConfirming(module.remedyPrompt);
                      }}
                    >
                      <WandIcon className="h-3.5 w-3.5" />
                    </Button>
                  </summary>
                  {module.zones.map((zone) => (
                    <div
                      key={zone.path}
                      className="flex flex-wrap items-center gap-2 border-t border-border bg-bg/40 py-2 pr-4 pl-11 hover:bg-raised"
                    >
                      <span className="min-w-0 flex-1 truncate font-mono text-xs text-muted" title={zone.path}>
                        {zone.path}
                      </span>
                      {zone.signals.map((signal) => (
                        <Badge key={signal} tone={SIGNAL_META[signal].tone} pill>
                          {SIGNAL_META[signal].label(zone)}
                        </Badge>
                      ))}
                      <span className="w-20 shrink-0 text-right font-mono text-xs text-faint tabular-nums">{zone.lines} lignes</span>
                    </div>
                  ))}
                </details>
              ))}
            </Card>
          ) : (
            <div className="px-4 py-3 text-sm text-muted">Aucune zone à risque.</div>
          )
        ) : null}
      </Section>
      <ConfirmDialog
        open={confirming !== null}
        title="Remédier ?"
        body="Un agent en lecture seule lit les modules les plus risqués et propose quelques tâches (5 au plus), les plus risqués d'abord ; les fichiers sans logique sont écartés. Tu choisis celles qui entrent au backlog. Relire reste ton travail : il te dit quels fichiers lire d'abord."
        confirmLabel="Lancer"
        variant="primary"
        onConfirm={() => {
          if (confirming === null) return;
          setConfirming(null);
          onStart(confirming, 'remedy');
        }}
        onCancel={() => setConfirming(null)}
      />
    </div>
  );
}
