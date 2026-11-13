/**
 * The night report banner: what the autonomous queue did while the human slept.
 */
import { useState } from 'react';
import { Badge } from '../../ui/Badge';
import { Button } from '../../ui/Button';
import { CheckIcon, ChevronDownIcon, ChevronRightIcon, EyeIcon, MoonIcon } from '../../ui/Icons';
import { Card } from '../../ui/Card';
import { formatRelative } from '../../../lib/format';
import { IdeaStatusBadge } from './IdeaStatusBadge';
import { IdeaMeta } from './IdeaMeta';
import type { IdeaBoard } from '../../../lib/ideas';
import { useT, useLang } from '../../../i18n';

type Report = NonNullable<IdeaBoard['nightReport']>;

export function NightReport({
  report,
  caps,
  onOpenSession,
  onDismiss,
}: {
  report: Report;
  caps: IdeaBoard['queue']['caps'];
  onOpenSession: (sessionId: string) => void;
  onDismiss: () => Promise<boolean>;
}) {
  // Open by default: the report is what the human reads first after a run.
  const [open, setOpen] = useState(true);
  const t = useT();
  const lang = useLang();

  const badges: { label: string; tone: 'accent' | 'ok' | 'err' }[] = [];
  if (report.summary.review > 0)
    badges.push({ label: t('attentionToReview', { count: report.summary.review }), tone: 'accent' });
  if (report.summary.question > 0)
    badges.push({
      label: t('attentionQuestions', {
        count: report.summary.question,
      }),
      tone: 'accent'
    });
  if (report.summary.halted > 0)
    badges.push({
      label: t('attentionHalted', {
        count: report.summary.halted,
      }),
      tone: 'err'
    });
  if (report.summary.failed > 0)
    badges.push({
      label: t('reportFailed', {
        count: report.summary.failed,
      }),
      tone: 'err'
    });
  if (report.summary.integrated > 0)
    badges.push({
      label: t('integratedCount', {
        count: report.summary.integrated,
      }),
      tone: 'ok'
    });
  if (report.summary.running > 0)
    badges.push({ label: t('reportRunning', { count: report.summary.running }), tone: 'accent' });

  const toggleLabel = open ? t('hideIdeas') : t('showIdeas');

  return (
    <Card className="flex flex-col overflow-hidden px-4 py-2.5">
      <div
        className={`-mx-4 -mt-2.5 flex min-h-8 items-center gap-3 px-4 py-2.5 bg-linear-to-r from-accent-soft to-transparent to-75% ${
          open ? 'border-b border-border' : ''
        }`}
      >
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title={toggleLabel}
          aria-label={toggleLabel}
          aria-expanded={open}
          onClick={() => setOpen((v) => !v)}
        >
          {open ? <ChevronDownIcon className="h-4 w-4" /> : <ChevronRightIcon className="h-4 w-4" />}
        </Button>
        {report.autonomous ? (
          <span className="grid size-6 shrink-0 place-items-center rounded-md bg-accent-soft text-accent">
            <MoonIcon className="h-3.5 w-3.5" />
          </span>
        ) : null}
        <h3 className="m-0 text-sm font-medium">
          {report.autonomous ? t('nightReportTitle') : t('queueReportTitle')}
        </h3>
        <span className="text-xs text-muted" title={report.reason}>
          {formatRelative(lang, report.finishedAt)}
        </span>
        {badges.length > 0 ? (
          <div className="flex flex-wrap items-center gap-1.5">
            {badges.map((b) => (
              <Badge key={b.label} tone={b.tone}>
                {b.label}
              </Badge>
            ))}
          </div>
        ) : null}
        <div className="grow" />
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title={t('markRead')}
          aria-label={t('markRead')}
          onClick={() => void onDismiss()}
        >
          <CheckIcon className="h-4 w-4" />
        </Button>
      </div>

      {open ? (
        <div className="mt-2 flex max-h-[32rem] flex-col overflow-y-auto">
          {report.ideas.map((idea, i) => {
            const sid = idea.sessionId;
            return (
              <div
                key={idea.id}
                className={`flex flex-col gap-1 py-2 ${
                  i > 0 ? 'border-t border-border' : ''
                }`}
              >
                <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                  <span className="shrink-0">
                    <IdeaStatusBadge status={idea.status} />
                  </span>
                  <span className="min-w-0 flex-1 truncate text-sm text-text" title={idea.text}>
                    {idea.title}
                  </span>
                  <div className="flex flex-wrap items-center gap-1">
                    <IdeaMeta idea={idea} caps={caps} />
                  </div>
                  {sid ? (
                    <Button
                      variant="ghost"
                      size="sm"
                      iconOnly
                      title={t('actionOpen')}
                      aria-label={t('actionOpen')}
                      onClick={() => onOpenSession(sid)}
                    >
                      <EyeIcon className="h-4 w-4" />
                    </Button>
                  ) : null}
                </div>
                {(idea.halted || idea.step || idea.runEnd || idea.decisions.length > 0) && (
                  <div className="flex flex-col gap-0.5 pl-1 text-xs text-muted">
                    {idea.halted ? <span className="text-err">{idea.halted}</span> : null}
                    {idea.runEnd ? <span className="text-err">{idea.runEnd}</span> : null}
                    {idea.step ? <span>{t('reportStep', { step: idea.step })}</span> : null}
                    {idea.decisions.length > 0 ? (
                      <ul className="m-0 list-none p-0">
                        {idea.decisions.map((decision, j) => (
                          <li key={j}>
                            <span className="text-text">{decision.title}</span> → {decision.choice}
                          </li>
                        ))}
                      </ul>
                    ) : null}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      ) : null}
    </Card>
  );
}
