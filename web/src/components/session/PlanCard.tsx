import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { Plan, Report } from '../../lib/plan';
import { CheckIcon, CloseIcon } from '../Icons';
import type { PlanActions } from './PlanActions';

const REMARK_PLUGINS = [remarkGfm];

// Inline code: Thread.tsx's inline style, on bg-code so it reads on the raised card.
const INLINE_CODE = 'rounded bg-code px-1 py-0.5 font-mono text-[0.7812rem] text-text';

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

function SectionHeading({ children }: { children: string }) {
  return <h3 className="m-0 text-[0.6875rem] font-semibold uppercase tracking-wide text-muted">{children}</h3>;
}

function Pill({ children }: { children: string }) {
  return <span className="rounded bg-accent-soft px-1.5 py-0.5 text-[0.6875rem] font-semibold text-accent-strong">{children}</span>;
}

function PathChip({ path }: { path: string }) {
  return <code className="rounded bg-code px-1.5 py-0.5 font-mono text-[0.7188rem] text-text">{path}</code>;
}

export function PlanCard({ plan, actions }: { plan: Plan; actions?: PlanActions | null }) {
  return (
    <div className="flex flex-col gap-3 rounded-xl border border-border bg-raised p-4">
      <div className="flex items-center gap-2">
        <Pill>Plan</Pill>
        <span className="min-w-0 font-semibold text-text">
          <Inline>{plan.title}</Inline>
        </span>
      </div>
      {plan.goal && (
        <p className="m-0 text-sm leading-relaxed text-muted">
          <Inline>{plan.goal}</Inline>
        </p>
      )}
      <ol className="m-0">
        {plan.steps.map((step, i) => (
          <li key={i} className="relative flex gap-3 pb-4 last:pb-0">
            {i < plan.steps.length - 1 && <span aria-hidden="true" className="absolute bottom-0 left-3 top-6 w-px bg-border" />}
            <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-accent-soft text-xs font-semibold text-accent-strong">
              {step.number}
            </span>
            <div className="flex min-w-0 flex-col gap-1.5">
              <div className="text-sm font-medium text-text">
                <Inline>{step.title}</Inline>
              </div>
              {step.files.length > 0 && (
                <div className="flex flex-wrap gap-1">
                  {step.files.map((f) => (
                    <PathChip key={f} path={f} />
                  ))}
                </div>
              )}
              {step.change && (
                <div className="text-sm leading-relaxed text-muted">
                  <Inline>{step.change}</Inline>
                </div>
              )}
              {step.verify && (
                <div className="flex items-start gap-1.5 text-sm text-muted">
                  <CheckIcon className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                  <span>
                    <Inline>{`Vérif. : ${step.verify}`}</Inline>
                  </span>
                </div>
              )}
            </div>
          </li>
        ))}
      </ol>
      {plan.doneWhen.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <SectionHeading>Terminé quand</SectionHeading>
          <ul className="m-0 flex flex-col gap-1">
            {plan.doneWhen.map((criterion, i) => (
              <li key={i} className="flex items-start gap-2 text-sm text-text">
                <span aria-hidden="true" className="mt-1 h-3.5 w-3.5 shrink-0 rounded-sm border border-border" />
                <span>
                  <Inline>{criterion}</Inline>
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
      {plan.risks.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <SectionHeading>Risques</SectionHeading>
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
      {actions && (
        <div className="flex gap-2">
          <button
            type="button"
            onClick={actions.onBuild}
            disabled={actions.disabled}
            title="Lance Plan, build, verify à partir de ce plan : critères vérifiés, vérification et review."
            className="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-on-accent disabled:opacity-60"
          >
            Construire
          </button>
          <button
            type="button"
            onClick={actions.onRevise}
            disabled={actions.disabled}
            className="rounded-lg border border-border bg-raised px-3 py-1.5 text-xs text-text disabled:opacity-60"
          >
            Revoir
          </button>
          <button
            type="button"
            onClick={actions.onBuildAlone}
            disabled={actions.disabled}
            title="Exécute le plan avec l'agent Builder seul, sans critères ni review."
            className="self-center text-xs text-muted hover:text-text hover:underline disabled:opacity-60"
          >
            Sans workflow
          </button>
        </div>
      )}
    </div>
  );
}

function Tally({ done, total }: { done: number; total: number }) {
  const all = total > 0 && done === total;
  return <span className={all ? 'text-ok' : 'text-err'}>{done}/{total} étapes</span>;
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
  const done = report.steps.filter((s) => s.checked).length;
  return (
    <div className="flex flex-col gap-3 rounded-xl border border-border bg-raised p-4">
      <div className="flex items-center gap-2">
        <Pill>Rapport</Pill>
        <span className="min-w-0 font-semibold text-text">
          <Inline>{report.title}</Inline>
        </span>
        {report.steps.length > 0 && <Tally done={done} total={report.steps.length} />}
      </div>
      {report.summary && (
        <p className="m-0 text-sm leading-relaxed text-text">
          <Inline>{report.summary}</Inline>
        </p>
      )}
      {report.steps.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <SectionHeading>Étapes</SectionHeading>
          <ul className="m-0 flex flex-col gap-1.5">
            {report.steps.map((step, i) => (
              <CheckRow key={i} checked={step.checked} text={`Étape ${step.number} · ${step.text}`} detail={step.detail} />
            ))}
          </ul>
        </div>
      )}
      {report.doneWhen.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <SectionHeading>Terminé quand</SectionHeading>
          <ul className="m-0 flex flex-col gap-1.5">
            {report.doneWhen.map((item, i) => (
              <CheckRow key={i} checked={item.checked} text={item.text} detail={item.detail} />
            ))}
          </ul>
        </div>
      )}
      {report.changed.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <SectionHeading>Fichiers modifiés</SectionHeading>
          <ul className="m-0 flex flex-col gap-1.5">
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
        </div>
      )}
      {report.test && (
        <div className="flex flex-col gap-1.5">
          <SectionHeading>À tester</SectionHeading>
          <div className="text-sm text-accent-strong">
            <Inline>{report.test}</Inline>
          </div>
        </div>
      )}
      {report.todo && (
        <div className="flex flex-col gap-1.5">
          <SectionHeading>À faire</SectionHeading>
          <div className="text-sm text-text">
            <Inline>{report.todo}</Inline>
          </div>
        </div>
      )}
      <p className="m-0 text-[0.6875rem] text-faint">Coches déclarées par le modèle, non vérifiées par fennec.</p>
    </div>
  );
}
