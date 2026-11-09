import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { Plan, Report } from '../../lib/plan';
import type { PreflightResultView } from '../../lib/fold';
import { CheckIcon, ChevronRightIcon, CloseIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { Card } from '../ui/Card';
import { SectionLabel } from '../ui/SectionLabel';
import { PreflightSection } from './PreflightSection';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';

const REMARK_PLUGINS = [remarkGfm];

// Inline code: Thread.tsx's inline style, on bg-code so it reads on the raised card.
const INLINE_CODE = 'rounded-md bg-code px-1 py-0.5 font-mono text-xs text-text';

// Titles and bodies may hold inline markdown (code, **bold**).
function Inline({ children }: { children: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={REMARK_PLUGINS}
      components={{
        p: ({ children }) => <>{children}</>,
        code: ({ children }) => <code className={INLINE_CODE}>{children}</code>,
      }}
    >
      {children}
    </ReactMarkdown>
  );
}

function PathChip({ path }: { path: string }) {
  return <code className="rounded-md bg-code px-1.5 py-0.5 font-mono text-xs text-text">{path}</code>;
}

// A step's files, on the raised inset: bg-panel keeps them lighter than bg-code.
const STEP_FILE_CHIP = 'rounded-md bg-panel px-1.5 py-0.5 font-mono text-xs text-text';

export function PlanBody({ plan, preflight }: { plan: Plan; preflight?: PreflightResultView[] }) {
  const t = useT();
  return (
    <div className="flex flex-col gap-4">
      {plan.goal && (
        <p className="m-0 text-sm leading-relaxed text-muted">
          <Inline>{plan.goal}</Inline>
        </p>
      )}
      <ol className="m-0 flex flex-col gap-2">
        {plan.steps.map((step, i) => {
          const hasDetails = step.files.length > 0 || step.change || step.verify;
          return (
            <li key={i} className="relative flex gap-3">
              {i < plan.steps.length - 1 && <span aria-hidden="true" className="absolute -bottom-2 left-3 top-7 w-px bg-border" />}
              <span
                className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold ${
                  i === 0 ? 'border border-accent/40 bg-accent-soft text-accent-strong' : 'border border-border bg-panel text-muted'
                }`}
              >
                {step.number}
              </span>
              <Card variant="inset" className="min-w-0 flex-1">
                <details className="group">
                  <summary className="flex cursor-pointer list-none items-start gap-2 text-sm select-none">
                    <span className="min-w-0 grow font-medium text-accent-strong">
                      <Inline>{step.title}</Inline>
                    </span>
                    {hasDetails && (
                      <ChevronRightIcon className="mt-0.5 h-3 w-3 shrink-0 text-muted transition-transform group-hover:text-text group-open:rotate-90" />
                    )}
                  </summary>
                  {hasDetails && (
                    <div className="flex flex-col gap-2 pt-2">
                      {step.files.length > 0 && (
                        <div className="flex flex-wrap gap-1">
                          {step.files.map((f) => (
                            <code key={f} className={STEP_FILE_CHIP}>
                              {f}
                            </code>
                          ))}
                        </div>
                      )}
                      {step.change && (
                        <div className="border-t border-border pt-2 text-sm leading-relaxed text-muted">
                          <Inline>{step.change}</Inline>
                        </div>
                      )}
                      {step.verify && (
                        <div className="flex items-start gap-1.5 text-sm text-muted">
                          <CheckIcon className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                          <span>
                            <Inline>{t('verifyLine', { verify: step.verify })}</Inline>
                          </span>
                        </div>
                      )}
                    </div>
                  )}
                </details>
              </Card>
            </li>
          );
        })}
      </ol>
      {plan.doneWhen.length > 0 && (
        <Card variant="inset">
          <SectionLabel>{t('doneWhen')}</SectionLabel>
          <ul className="m-0 flex flex-col gap-1.5">
            {plan.doneWhen.map((criterion, i) => (
              <li key={i} className="flex items-start gap-2 text-sm text-text">
                <span aria-hidden="true" className="mt-1 h-3.5 w-3.5 shrink-0 rounded-sm border border-border-strong bg-panel" />
                <span>
                  <Inline>{criterion}</Inline>
                </span>
              </li>
            ))}
          </ul>
        </Card>
      )}
      {preflight && preflight.length > 0 && <PreflightSection results={preflight} />}
      {plan.risks.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <SectionLabel>{t('risksLabel')}</SectionLabel>
          <ul className="m-0 flex flex-col gap-1">
            {plan.risks.map((risk, i) => (
              <li key={i} className="flex items-start gap-2 text-sm text-err">
                <span aria-hidden="true" className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-current" />
                <span>
                  <Inline>{risk}</Inline>
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

export function PlanCard({
  plan,
  preflight,
  pending,
  anchorId
}: {
  plan: Plan;
  preflight?: PreflightResultView[];
  pending?: boolean;
  anchorId?: string;
}) {
  const t = useT();
  return (
    <Card id={anchorId} className="overflow-hidden">
      <div className="flex items-center gap-2 border-b border-border bg-accent-soft/50 px-4 py-2.5">
        <Badge tone="accent">{t('planBadge')}</Badge>
        <span className="min-w-0 font-semibold text-text">
          <Inline>{plan.title}</Inline>
        </span>
        {pending && (
          <Badge tone="attention" dot className="ml-auto">
            {t('planPending')}
          </Badge>
        )}
      </div>
      <div className="p-4">
        <PlanBody plan={plan} preflight={preflight} />
      </div>
    </Card>
  );
}

function Tally({ done, total }: { done: number; total: number }) {
  const t = useT();
  const all = total > 0 && done === total;
  return <span className={all ? 'text-ok' : 'text-err'}>{t('stepsTally', { done, total })}</span>;
}

function CheckRow({ checked, text, detail }: { checked: boolean; text: string; detail: string }) {
  return (
    <li className="flex items-start gap-2 text-sm">
      {checked ? (
        <CheckIcon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-ok" />
      ) : (
        <CloseIcon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-err" />
      )}
      <span className="min-w-0 text-text">
        <Inline>{text}</Inline>
        {detail && (
          <span className="text-muted">
            {' — '}
            <Inline>{detail}</Inline>
          </span>
        )}
      </span>
    </li>
  );
}

export function ReportCard({ report }: { report: Report }) {
  const t = useT();
  const done = report.steps.filter((s) => s.checked).length;
  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex items-center gap-2">
        <Badge tone="accent">{t('reportBadge')}</Badge>
        <span className="min-w-0 font-semibold text-text">
          <Inline>{report.title}</Inline>
        </span>
        {report.steps.length > 0 && <Tally done={done} total={report.steps.length} />}
        <Badge tone="accent" title={t('reportIndicativeTitle')} className="ml-auto">{t('indicative')}</Badge>
      </div>
      {report.summary && (
        <p className="m-0 text-sm leading-relaxed text-text">
          <Inline>{report.summary}</Inline>
        </p>
      )}
      {report.steps.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <SectionLabel>{t('stepsLabel')}</SectionLabel>
          <ul className="m-0 flex flex-col gap-1.5">
            {report.steps.map((step, i) => (
              <CheckRow key={i} checked={step.checked} text={t('stepRow', { number: step.number, text: step.text })} detail={step.detail} />
            ))}
          </ul>
        </div>
      )}
      {report.doneWhen.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <SectionLabel>{t('doneWhen')}</SectionLabel>
          <ul className="m-0 flex flex-col gap-1.5">
            {report.doneWhen.map((item, i) => (
              <CheckRow key={i} checked={item.checked} text={item.text} detail={item.detail} />
            ))}
          </ul>
        </div>
      )}
      {report.changed.length > 0 && (
        // Folded: on a large change the file list would outgrow the rest of the report.
        <details className="group flex flex-col gap-1.5">
          <summary className="flex cursor-pointer list-none items-center gap-1.5 select-none">
            <ChevronRightIcon className="h-3 w-3 shrink-0 text-muted transition-transform group-hover:text-text group-open:rotate-90" />
            <SectionLabel>{t('panelModifiedFiles')}</SectionLabel>
            <Badge tone="neutral">{report.changed.length}</Badge>
          </summary>
          <ul className="m-0 mt-1.5 flex flex-col gap-1.5">
            {report.changed.map((c, i) => (
              <li key={i} className="flex items-start gap-2 text-sm">
                <PathChip path={c.path} />
                {c.what && (
                  <span className="min-w-0 text-muted">
                    <Inline>{c.what}</Inline>
                  </span>
                )}
              </li>
            ))}
          </ul>
        </details>
      )}
      {report.test && (
        <div className="flex flex-col gap-1.5">
          <SectionLabel>{t('toTest')}</SectionLabel>
          <div className="text-sm text-text">
            <Inline>{report.test}</Inline>
          </div>
        </div>
      )}
      {report.todo && (
        <div className="flex flex-col gap-1.5">
          <SectionLabel>{t('toDo')}</SectionLabel>
          <div className="text-sm text-text">
            <Inline>{report.todo}</Inline>
          </div>
        </div>
      )}
    </Card>
  );
}

export const IDEA_LABELS: Record<'none' | 'framing' | 'ready' | 'saved', MsgKey> = {
  none: 'ideaSaveToBacklog',
  framing: 'ideaReady',
  ready: 'ideaUpdate',
  saved: 'ideaInBacklog',
};
