import { useEffect, useState, type ComponentType } from 'react';
import { getInsight, type InsightCard, type InsightPeriod, type InsightRow, type InsightView } from '../lib/api';
import {
  ActivityIcon,
  AlertIcon,
  ArrowUpIcon,
  CoinIcon,
  CpuIcon,
  InboxIcon,
  LayersIcon,
  ProjectIcon
} from '../components/ui/Icons';
import { type BadgeTone } from '../components/ui/Badge';
import { Card } from '../components/ui/Card';
import { Tile } from '../components/ui/Tile';
import { ContextComposition } from '../components/session/ContextComposition';
import { SectionLabel } from '../components/ui/SectionLabel';
import { Segmented, type Segment } from '../components/ui/Segmented';
import { Select } from '../components/ui/Field';
import { useT } from '../i18n';
import { type MsgKey } from '../i18n/messages';

type Icon = ComponentType<{ className?: string }>;

const PERIOD_SEGMENTS: Segment<InsightPeriod>[] = [
  { value: '7', label: 'insightPeriod7' },
  { value: '30', label: 'insightPeriod30' },
  { value: '90', label: 'insightPeriod90' },
  { value: 'all', label: 'insightPeriodAll' }
];

/** One tone per KPI tile, keyed on the server's stable kind; the same Tile as the risks tab. */
const CARD_TONE: Record<InsightCard['kind'], { Icon: Icon; tone: BadgeTone }> = {
  calls: { Icon: ActivityIcon, tone: 'accent' },
  input: { Icon: InboxIcon, tone: 'neutral' },
  cache: { Icon: LayersIcon, tone: 'ok' },
  lostCache: { Icon: AlertIcon, tone: 'err' },
  output: { Icon: ArrowUpIcon, tone: 'neutral' },
  cost: { Icon: CoinIcon, tone: 'attention' }
};

function KpiCard({ kind, label, value, detail }: { kind: InsightCard['kind']; label: string; value: string; detail: string }) {
  const { Icon, tone } = CARD_TONE[kind];
  return <Tile icon={<Icon />} label={label} figure={value} detail={detail} title={detail} tone={tone} active={tone !== 'neutral'} />;
}

function SectionHeader({ icon: Icon, title }: { icon?: Icon; title: string }) {
  return (
    <div className="flex items-center gap-2">
      {Icon ? <Icon className="h-4 w-4 text-muted" /> : null}
      <SectionLabel>{title}</SectionLabel>
    </div>
  );
}

function InsightTable({
  title,
  icon: Icon,
  rows,
  onNameClick
}: {
  title: string;
  icon?: Icon;
  rows: InsightRow[];
  onNameClick?: (key: string) => void;
}) {
  const t = useT();
  return (
    <Card className="p-4">
      <SectionHeader icon={Icon} title={title} />
      <div className="mt-3 overflow-x-auto">
        <table className="w-full min-w-[44rem] table-fixed whitespace-nowrap text-sm tabular-nums">
          <colgroup>
            <col className="w-[26%]" />
            <col />
            <col />
            <col />
            <col />
            <col />
            <col />
            <col />
          </colgroup>
          <thead>
            <tr>
              <th className="py-2 pr-3 pl-0 text-left text-xs font-normal text-muted">{t('nameLabel')}</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">{t('callsLabel')}</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">{t('inputLabel')}</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">{t('cacheLabel')}</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">{t('outputLabel')}</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">{t('insightTableLostCache')}</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted" title={t('insightTableCostTitle')}>{t('insightTableCost')}</th>
              <th className="py-2 pr-0 pl-3 text-right text-xs font-normal text-muted">{t('insightTableTime')}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.key} className="border-t border-border transition-colors hover:bg-raised">
                <td className="py-2 pr-3 pl-0">
                  {onNameClick ? (
                    <button
                      type="button"
                      onClick={() => onNameClick(row.key)}
                      title={row.name}
                      className="block max-w-full truncate text-left text-accent hover:underline"
                    >
                      {row.name}
                    </button>
                  ) : (
                    <span title={row.name} className="block truncate">
                      {row.name}
                    </span>
                  )}
                </td>
                <td className="px-3 py-2 text-right">{row.calls}</td>
                <td className="px-3 py-2 text-right">{row.input}</td>
                <td className="px-3 py-2 text-right">{row.cache}</td>
                <td className="px-3 py-2 text-right">{row.output}</td>
                <td className={`px-3 py-2 text-right ${row.lost === '—' ? '' : 'text-err'}`}>{row.lost}</td>
                <td className={`px-3 py-2 text-right ${row.cost === '—' ? '' : 'text-accent'}`}>{row.cost}</td>
                <td className="py-2 pr-0 pl-3 text-right">{row.time}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  );
}

export function InsightPage() {
  const t = useT();
  const [period, setPeriod] = useState<InsightPeriod>('30');
  const [project, setProject] = useState<string | null>(null);
  const [view, setView] = useState<InsightView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    setView(null);
    setError(null);

    getInsight(period, project)
      .then((data) => {
        if (!cancelled) {
          setView(data);
        }
      })
      .catch((fetchError) => {
        if (!cancelled) {
          setError(fetchError instanceof Error ? fetchError.message : t('insightLoadError'));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [period, project]);

  return (
    <div className="min-h-0 grow overflow-y-auto bg-bg">
      <div className="mx-auto flex w-full max-w-5xl flex-col gap-6 p-6">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-md bg-accent-soft text-accent">
              <ActivityIcon className="h-4 w-4" />
            </span>
            <div>
              <h1 className="text-2xl font-semibold">{t('insight')}</h1>
              <p className="text-xs text-muted">{t('insightUsageSubtitle')}</p>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Select
              aria-label={t('homeProject')}
              value={project ?? ''}
              onChange={(event) => setProject(event.target.value === '' ? null : event.target.value)}
            >
              <option value="">{t('insightAllProjects')}</option>
              {(view?.projectOptions ?? []).map((option) => (
                <option key={option.id} value={option.id}>
                  {option.name}
                </option>
              ))}
            </Select>

            <Segmented
              label={t('insightPeriodLabel')}
              segments={PERIOD_SEGMENTS.map((segment) => ({ ...segment, label: t(segment.label as MsgKey) }))}
              value={period}
              onChange={setPeriod}
            />
          </div>
        </div>

        {error ? <p className="text-err">{error}</p> : null}
        {!view && !error ? <div className="text-muted">{t('loading')}</div> : null}

        {view ? (
          <>
            {view.cards.length > 0 ? (
              <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
                {view.cards.map((card) => (
                  <KpiCard key={card.kind} kind={card.kind} label={card.label} value={card.value} detail={card.detail} />
                ))}
              </div>
            ) : null}

            <Card className="p-4">
              <SectionHeader icon={ActivityIcon} title={t('insightDailyInput')} />

              {view.days.length > 0 ? (
                <>
                  <div className="mt-3 flex h-40 items-end gap-1 border-b border-border pb-px">
                    {view.days.map((day) => (
                      <div key={day.day} title={day.title} className="group flex h-full min-w-0 flex-1 items-end">
                        <div className="flex h-full w-full flex-col-reverse transition-[filter] group-hover:brightness-110">
                          <div className="rounded-t-[3px] bg-accent/35" style={{ height: `${day.cachedPercent}%` }} />
                          <div className="rounded-t-[3px] bg-accent" style={{ height: `${day.freshPercent}%` }} />
                          <div className="rounded-t-[3px] bg-err" style={{ height: `${day.lostPercent}%` }} />
                        </div>
                      </div>
                    ))}
                  </div>

                  <div className="mt-1 flex gap-1">
                    {view.days.map((day) => (
                      <div key={day.day} className="min-w-0 flex-1 truncate text-xs text-muted">
                        {day.label}
                      </div>
                    ))}
                  </div>

                  <div className="mt-3 flex flex-wrap items-center gap-4 text-xs text-muted">
                    <span className="flex items-center gap-1.5">
                      <span className="h-2.5 w-2.5 rounded-full bg-accent/35" />
                      {t('insightLegendCache')}
                    </span>
                    <span className="flex items-center gap-1.5">
                      <span className="h-2.5 w-2.5 rounded-full bg-accent" />
                      {t('insightLegendFresh')}
                    </span>
                    <span className="flex items-center gap-1.5">
                      <span className="h-2.5 w-2.5 rounded-full bg-err" />
                      {t('insightLegendLostCache')}
                    </span>
                  </div>
                </>
              ) : (
                <p className="mt-3 text-muted">{t('insightNoCalls')}</p>
              )}
            </Card>

            {view.consumers ? (
              <Card className="p-4">
                <ContextComposition composition={view.consumers} title={t('insightConsumersTitle')} />
              </Card>
            ) : null}

            {view.projects.length > 0 ? (
              <InsightTable title={t('insightByProject')} icon={ProjectIcon} rows={view.projects} onNameClick={(key) => setProject(key)} />
            ) : null}

            <InsightTable title={t('settingsTabModels')} icon={CpuIcon} rows={view.models} />
          </>
        ) : null}
      </div>
    </div>
  );
}
