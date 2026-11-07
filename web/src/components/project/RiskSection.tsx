// Project page risks tab: server-measured risk tiles and zones with a re-measure action.
import { useEffect, useState } from 'react';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';
import { getProjectRisks, type RiskReport, type RiskSignal, type RiskZone } from '../../lib/api/risks';
import { Badge, type BadgeTone } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { Section } from '../ui/Section';
import { Tile } from '../ui/Tile';
import { ActivityIcon, AlertIcon, ChevronRightIcon, RefreshIcon, ReviewIcon, SpinnerIcon, WandIcon } from '../Icons';

type T = (key: MsgKey, vars?: Record<string, string | number>) => string;

// One colour per signal, shared by the tiles, the module columns and the file badges, so a colour reads as a signal everywhere.
const SIGNAL_META: Record<RiskSignal, { tone: BadgeTone; label: (zone: RiskZone, t: T) => string }> = {
  unreviewed: { tone: 'err', label: (_zone, t) => t('signalNeverReviewed') },
  untested: { tone: 'attention', label: (_zone, t) => t('signalNoTest') },
  churn: { tone: 'info', label: (zone, t) => t('signalChurn', { count: zone.churn }) },
};

/** A module's count for one signal, in its fixed column; nothing when zero. */
function Count({ value, tone, title }: { value: number; tone: BadgeTone; title: string }) {
  return (
    <span className="w-28 shrink-0 text-right">
      {value > 0 ? (
        <Badge tone={tone} appearance="pill" title={title}>
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
  const t = useT();
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
          label={t('neverReviewed')}
          figure={data ? `${data.unreviewedPercent} %` : '…'}
          detail={data ? t('neverReviewedDetail', { ai: data.aiCommits, total: data.commits }) : '…'}
          title={t('neverReviewedTitle')}
          tone="err"
          active={data ? data.unreviewed > 0 : false}
        />
        <Tile
          icon={<AlertIcon />}
          label={t('noOwnTest')}
          figure={data ? `${data.untested}` : '…'}
          detail={data ? t('riskNoOwnTestFilesOn', { total: data.files }) : '…'}
          tone="attention"
          active={data ? data.untested > 0 : false}
        />
        <Tile
          icon={<ActivityIcon />}
          label={t('oftenModified')}
          figure={data ? `${data.churning}` : '…'}
          detail={data ? t('oftenModifiedDetail', { days: data.churnDays }) : '…'}
          tone="info"
          active={data ? data.churning > 0 : false}
        />
      </div>
      {error ? <div className="px-4 pb-2 text-err text-xs">{error}</div> : null}
      <Section
        title={t('riskZones')}
        flush
        description={t('riskZonesDescription')}
        actions={
          <>
            <Button
              variant="primary"
              size="sm"
              disabled={busy || !data?.remedyPrompt}
              onClick={() => data && setConfirming(data.remedyPrompt)}
              title={t('proposeTasks')}
            >
              {t('remedy')}
            </Button>
            <Badge tone="ok" appearance="pill" title={t('computedFromGit')}>
              {t('verified')}
            </Badge>
            <Button
              variant="ghost"
              size="sm"
              iconOnly
              title={t('measureAgain')}
              aria-label={t('measureAgain')}
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
                <span className="min-w-0 grow">{t('moduleCol')}</span>
                <span className="w-28 shrink-0 text-right">{t('filesNeverReviewed')}</span>
                <span className="w-28 shrink-0 text-right">{t('filesNoTest')}</span>
                <span className="w-28 shrink-0 text-right">{t('filesOftenModified')}</span>
                <span className="w-20 shrink-0 text-right">{t('filesCol')}</span>
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
                    <Count value={module.unreviewed} tone={SIGNAL_META.unreviewed.tone} title={t('filesNeverReviewed')} />
                    <Count value={module.untested} tone={SIGNAL_META.untested.tone} title={t('filesNoTest')} />
                    <Count value={module.churning} tone={SIGNAL_META.churn.tone} title={t('filesOftenModified')} />
                    <span className="w-20 shrink-0 text-right font-mono text-xs text-muted tabular-nums">{module.zones.length}</span>
                    {/* Hidden until hover so the counts stay the row's reading; the summary must not toggle on its click. */}
                    <Button
                      variant="soft"
                      size="sm"
                      iconOnly
                      className="opacity-0 group-hover/row:opacity-100 focus-visible:opacity-100"
                      title={t('remedyModule')}
                      aria-label={t('remedyModuleAria', { path: module.path })}
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
                        <Badge key={signal} tone={SIGNAL_META[signal].tone} appearance="pill">
                          {SIGNAL_META[signal].label(zone, t)}
                        </Badge>
                      ))}
                      <span className="w-20 shrink-0 text-right font-mono text-xs text-faint tabular-nums">{t('linesUnit', { count: zone.lines })}</span>
                    </div>
                  ))}
                </details>
              ))}
            </Card>
          ) : (
            <div className="px-4 py-3 text-sm text-muted">{t('noRiskZones')}</div>
          )
        ) : null}
      </Section>
      <ConfirmDialog
        open={confirming !== null}
        title={t('remedyQuestion')}
        body={t('remedyBody')}
        confirmLabel={t('launch')}
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
