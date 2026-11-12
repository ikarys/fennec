import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { pathParts, type Plan, type PlanCriterion, type Report } from '../../lib/plan';
import type { PreflightResultView } from '../../lib/fold';
import { AlertIcon, CheckIcon, ChevronRightIcon, CloseIcon, EyeIcon, FileIcon, PlayIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { Card } from '../ui/Card';
import { SectionLabel } from '../ui/SectionLabel';
import { PreflightSection } from './PreflightSection';
import { useT } from '../../i18n';
import { useFileLink } from './FileLinkContext';
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
        <div className="overflow-hidden rounded-md border border-border">
          <div className="flex items-center gap-2 border-b border-border bg-raised px-3 py-1.5">
            <CheckIcon className="h-3.5 w-3.5 text-accent" />
            <SectionLabel>{t('doneWhen')}</SectionLabel>
            <Badge tone="neutral">{plan.doneWhen.length}</Badge>
          </div>
          <ul className="m-0 flex flex-col divide-y divide-border">
            {plan.doneWhen.map((criterion, i) => <PlanCriterionRow key={i} id={`C${i + 1}`} criterion={criterion} />)}
          </ul>
        </div>
      )}
      {preflight && preflight.length > 0 && <PreflightSection results={preflight} />}
      {plan.risks.length > 0 && (
        <div className="overflow-hidden rounded-md border border-border">
          <div className="flex items-center gap-2 border-b border-border bg-raised px-3 py-1.5">
            <AlertIcon className="h-3.5 w-3.5 text-attention" />
            <SectionLabel>{t('risksLabel')}</SectionLabel>
            <Badge tone="attention">{plan.risks.length}</Badge>
          </div>
          <ul className="m-0 flex flex-col divide-y divide-border">
            {plan.risks.map((risk, i) => (
              <li key={i} className="flex items-start gap-2.5 px-3 py-2 text-sm text-text">
                <span aria-hidden="true" className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-attention" />
                <span className="min-w-0">
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

function CriterionId({ id }: { id: string }) {
  return (
    <span className="mt-px rounded-sm bg-accent-soft px-1.5 py-0.5 font-mono text-[11px] font-semibold text-accent">{id}</span>
  );
}

function PlanCriterionRow({ id, criterion }: { id: string; criterion: PlanCriterion }) {
  const t = useT();
  return (
    <li className="grid grid-cols-[auto_1fr] items-start gap-x-2.5 gap-y-1 px-3 py-2.5">
      <CriterionId id={id} />
      <span className="min-w-0 text-sm text-text"><Inline>{criterion.text}</Inline></span>
      <span className="col-start-2">
        {criterion.check ? (
          <Badge tone="info" title={t('criterionCheckTitle', { command: criterion.check })}>
            <PlayIcon className="h-3 w-3" />
            {t('criterionCheck')}
          </Badge>
        ) : (
          <Badge tone="attention" title={t('criterionJudgeTitle')}>
            <EyeIcon className="h-3 w-3" />
            {t('criterionJudge')}
          </Badge>
        )}
      </span>
    </li>
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

function Tally({ done, total, label }: { done: number; total: number; label: 'stepsTally' | 'criteriaTally' }) {
  const t = useT();
  const all = total > 0 && done === total;
  return <Badge tone={all ? 'ok' : 'err'}>{t(label, { done, total })}</Badge>;
}

// The agent's explanation is folded: closed, the report reads as a checklist.
function CheckRow({ checked, label, text, detail }: { checked: boolean; label: string; text: string; detail: string }) {
  const head = (
    <>
      <Badge tone={checked ? 'ok' : 'err'}>
        {checked ? <CheckIcon className="h-3 w-3" /> : <CloseIcon className="h-3 w-3" />}
        {label}
      </Badge>
      <span className="min-w-0 text-sm text-text"><Inline>{text}</Inline></span>
    </>
  );
  const grid = 'grid grid-cols-[auto_1fr_auto] items-start gap-x-2.5';
  return (
    <li className={`px-3 py-2.5 ${checked ? 'shadow-[inset_2px_0_0_var(--color-ok)]' : 'bg-err/5 shadow-[inset_2px_0_0_var(--color-err)]'}`}>
      {detail ? (
        <details className="group">
          <summary className={`${grid} cursor-pointer list-none select-none`}>
            {head}
            <ChevronRightIcon className="mt-1 h-3 w-3 shrink-0 text-muted transition-transform group-hover:text-text group-open:rotate-90" />
          </summary>
          <div className="pt-1.5 text-xs text-muted"><Inline>{detail}</Inline></div>
        </details>
      ) : (
        <div className={grid}>{head}</div>
      )}
    </li>
  );
}

function ChangedFileRow({ path, what }: { path: string; what: string }) {
  const { dir, name } = pathParts(path);
  const fileLink = useFileLink();
  return (
    <li className="flex flex-col gap-0.5 px-3 py-2" title={path}>
      {fileLink ? (
        <a
          href={fileLink(path)}
          className="group min-w-0 truncate font-mono text-xs hover:text-accent-strong"
        >
          <span className="text-muted">{dir}</span>
          <span className="font-semibold text-text group-hover:text-accent-strong">
            {name}
          </span>
        </a>
      ) : (
        <span className="min-w-0 truncate font-mono text-xs">
          <span className="text-muted">{dir}</span>
          <span className="font-semibold text-text">{name}</span>
        </span>
      )}
      {what && (
        <span className="text-xs text-muted">
          <Inline>{what}</Inline>
        </span>
      )}
    </li>
  );
}

export function ReportCard({ report }: { report: Report }) {
  const t = useT();
  const done = report.steps.filter((s) => s.checked).length;
  const met = report.doneWhen.filter((c) => c.checked).length;
  return (
    <Card className="flex flex-col gap-3 p-4">
      <div className="flex items-center gap-2">
        <Badge tone="accent">{t('reportBadge')}</Badge>
        <span className="min-w-0 font-semibold text-text">
          <Inline>{report.title}</Inline>
        </span>
        {report.steps.length > 0 && <Tally done={done} total={report.steps.length} label="stepsTally" />}
        {report.doneWhen.length > 0 && <Tally done={met} total={report.doneWhen.length} label="criteriaTally" />}
        <Badge tone="accent" title={t('reportIndicativeTitle')} className="ml-auto">{t('indicative')}</Badge>
      </div>
      {report.summary && (
        <p className="m-0 text-sm leading-relaxed text-text">
          <Inline>{report.summary}</Inline>
        </p>
      )}
      {report.steps.length > 0 && (
        <div className="overflow-hidden rounded-md border border-border">
          <div className="flex items-center gap-2 border-b border-border bg-raised px-3 py-1.5">
            <CheckIcon className="h-3.5 w-3.5 text-accent" />
            <SectionLabel>{t('stepsLabel')}</SectionLabel>
          </div>
          <ul className="m-0 flex flex-col divide-y divide-border">
            {report.steps.map((step, i) => (
              <CheckRow
                key={i}
                checked={step.checked}
                label={step.checked ? t('stepDone', { number: step.number }) : t('stepNotDone', { number: step.number })}
                text={step.text}
                detail={step.detail}
              />
            ))}
          </ul>
        </div>
      )}
      {report.doneWhen.length > 0 && (
        <div className="overflow-hidden rounded-md border border-border">
          <div className="flex items-center gap-2 border-b border-border bg-raised px-3 py-1.5">
            <CheckIcon className="h-3.5 w-3.5 text-accent" />
            <SectionLabel>{t('doneWhen')}</SectionLabel>
          </div>
          <ul className="m-0 flex flex-col divide-y divide-border">
            {report.doneWhen.map((item, i) => (
              <CheckRow
                key={i}
                checked={item.checked}
                label={item.checked ? t('criterionMet', { id: `C${item.number}` }) : t('criterionUnmet', { id: `C${item.number}` })}
                text={item.text}
                detail={item.detail}
              />
            ))}
          </ul>
        </div>
      )}
      {report.changed.length > 0 && (
        // Folded: on a large change the file list would outgrow the rest of the report.
        <details className="group overflow-hidden rounded-md border border-border">
          <summary className="flex cursor-pointer list-none items-center gap-2 bg-raised px-3 py-1.5 select-none group-open:border-b group-open:border-border">
            <ChevronRightIcon className="h-3 w-3 shrink-0 text-muted transition-transform group-hover:text-text group-open:rotate-90" />
            <FileIcon className="h-3.5 w-3.5 text-accent" />
            <SectionLabel>{t('panelModifiedFiles')}</SectionLabel>
            <Badge tone="neutral">{report.changed.length}</Badge>
          </summary>
          <ul className="m-0 flex flex-col divide-y divide-border">
            {report.changed.map((c, i) => (
              <ChangedFileRow key={i} path={c.path} what={c.what} />
            ))}
          </ul>
        </details>
      )}
      {report.test.length > 0 && (
        <div className="overflow-hidden rounded-md border border-border">
          <div className="flex items-center gap-2 border-b border-border bg-raised px-3 py-1.5">
            <EyeIcon className="h-3.5 w-3.5 text-accent" />
            <SectionLabel>{t('toTest')}</SectionLabel>
          </div>
          <ol className="m-0 flex flex-col gap-1.5 px-3 py-2.5">
            {report.test.map((item, i) => (
              <li key={i} className="flex items-start gap-2.5 text-sm text-text">
                <span className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-accent-soft text-[10px] font-semibold text-accent">{i + 1}</span>
                <span className="min-w-0"><Inline>{item}</Inline></span>
              </li>
            ))}
          </ol>
        </div>
      )}
      {report.todo.length > 0 && (
        <div className="overflow-hidden rounded-md border border-border">
          <div className="flex items-center gap-2 border-b border-border bg-raised px-3 py-1.5">
            <AlertIcon className="h-3.5 w-3.5 text-attention" />
            <SectionLabel>{t('toDo')}</SectionLabel>
          </div>
          <ul className="m-0 flex flex-col gap-1.5 px-3 py-2.5">
            {report.todo.map((item, i) => (
              <li key={i} className="flex items-start gap-2.5 text-sm text-text">
                <span aria-hidden="true" className="mt-0.5 h-3.5 w-3.5 shrink-0 rounded-sm border-[1.5px] border-attention bg-attention/10" />
                <span className="min-w-0"><Inline>{item}</Inline></span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </Card>
  );
}

export const IDEA_LABELS: Record<IdeaButton, MsgKey> = {
  none: 'ideaSaveToBacklog',
  framing: 'ideaReady',
  ready: 'ideaUpdate',
  saved: 'ideaInBacklog',
  sheet: 'applyToSheet',
  applied: 'sheetApplied',
};

/** The plan's backlog button: a plain idea is marked ready, a task gets the plan applied to its sheet. */
export type IdeaButton = 'none' | 'framing' | 'ready' | 'saved' | 'sheet' | 'applied';
