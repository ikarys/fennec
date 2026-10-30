import { useEffect, useState } from 'react';
import { getInsight, type InsightPeriod, type InsightRow, type InsightView } from '../lib/api';
import { Card } from '../components/ui/Card';
import { ContextComposition } from '../components/session/ContextComposition';
import { SectionLabel } from '../components/ui/SectionLabel';
import { Segmented, type Segment } from '../components/ui/Segmented';
import { Select } from '../components/ui/Field';

const PERIOD_SEGMENTS: Segment<InsightPeriod>[] = [
  { value: '7', label: '7 j' },
  { value: '30', label: '30 j' },
  { value: '90', label: '90 j' },
  { value: 'all', label: 'Tout' }
];

function InsightTable({
  title,
  rows,
  onNameClick
}: {
  title: string;
  rows: InsightRow[];
  onNameClick?: (key: string) => void;
}) {
  return (
    <Card className="p-4">
      <SectionLabel>{title}</SectionLabel>
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
              <th className="py-2 pr-3 pl-0 text-left text-xs font-normal text-muted">Nom</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">Appels</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">Entrée</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">Cache</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">Sortie</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted">Cache perdu</th>
              <th className="px-3 py-2 text-right text-xs font-normal text-muted" title="Estimation au tarif de référence (Paramètres, Coût)">Coût</th>
              <th className="py-2 pr-0 pl-3 text-right text-xs font-normal text-muted">Temps</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.key} className="border-t border-border">
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
                <td className="px-3 py-2 text-right">{row.lost}</td>
                <td className="px-3 py-2 text-right">{row.cost}</td>
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
          setError(fetchError instanceof Error ? fetchError.message : 'Impossible de charger l’insight');
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
          <div className="flex items-baseline gap-2">
            <h1 className="text-xl font-semibold">Insight</h1>
            {view?.scope ? <span className="text-muted">{view.scope}</span> : null}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Select
              aria-label="Projet"
              value={project ?? ''}
              onChange={(event) => setProject(event.target.value === '' ? null : event.target.value)}
            >
              <option value="">Tous les projets</option>
              {(view?.projectOptions ?? []).map((option) => (
                <option key={option.id} value={option.id}>
                  {option.name}
                </option>
              ))}
            </Select>

            <Segmented
              label="Période"
              segments={PERIOD_SEGMENTS}
              value={period}
              onChange={setPeriod}
            />
          </div>
        </div>

        {error ? <p className="text-err">{error}</p> : null}
        {!view && !error ? <div className="text-muted">Chargement…</div> : null}

        {view ? (
          <>
            {view.cards.length > 0 ? (
              <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
                {view.cards.map((card) => (
                  <Card key={card.label} className="p-4">
                    <SectionLabel>{card.label}</SectionLabel>
                    <div className="mt-2 text-2xl font-semibold tabular-nums">{card.value}</div>
                    <div className="mt-1 text-xs text-muted">{card.detail}</div>
                  </Card>
                ))}
              </div>
            ) : null}

            <Card className="p-4">
              <SectionLabel>Entrée par jour</SectionLabel>

              {view.days.length > 0 ? (
                <>
                  <div className="mt-3 flex h-40 items-end gap-1">
                    {view.days.map((day) => (
                      <div key={day.day} title={day.title} className="flex h-full min-w-0 flex-1">
                        <div className="flex h-full w-full flex-col-reverse">
                          <div className="bg-accent/35" style={{ height: `${day.cachedPercent}%` }} />
                          <div className="bg-accent" style={{ height: `${day.freshPercent}%` }} />
                          <div className="bg-err" style={{ height: `${day.lostPercent}%` }} />
                        </div>
                      </div>
                    ))}
                  </div>

                  <div className="mt-1 flex gap-1">
                    {view.days.map((day) => (
                      <div key={day.day} className="min-w-0 flex-1 truncate text-[0.6875rem] text-muted">
                        {day.label}
                      </div>
                    ))}
                  </div>

                  <div className="mt-3 flex flex-wrap items-center gap-4 text-xs text-muted">
                    <span className="flex items-center gap-1.5">
                      <span className="h-2.5 w-2.5 rounded-sm bg-accent/35" />
                      cache
                    </span>
                    <span className="flex items-center gap-1.5">
                      <span className="h-2.5 w-2.5 rounded-sm bg-accent" />
                      hors cache
                    </span>
                    <span className="flex items-center gap-1.5">
                      <span className="h-2.5 w-2.5 rounded-sm bg-err" />
                      cache perdu
                    </span>
                  </div>
                </>
              ) : (
                <p className="mt-3 text-muted">Aucun appel sur la période.</p>
              )}
            </Card>

            {view.consumers ? (
              <Card className="p-4">
                <ContextComposition composition={view.consumers} title="Ce qui consomme l'entrée" />
              </Card>
            ) : null}

            {view.projects.length > 0 ? (
              <InsightTable
                title="Par projet"
                rows={view.projects}
                onNameClick={(key) => setProject(key)}
              />
            ) : null}

            <InsightTable title="Modèles" rows={view.models} />
          </>
        ) : null}
      </div>
    </div>
  );
}

