import type { ServedPendingPlan } from './api/sessions';
import type { PreflightResultView, ThreadItem } from './threadView';

/** The plan and report contracts of the plan and builder agents (crates/fennec/agents/*.agent.md), read back for display. */

export interface PlanStep {
  number: number;
  title: string;
  files: string[];
  change: string;
  verify: string;
}

export interface PlanCriterion {
  text: string;
  check: string;
}

export interface Plan {
  kind: 'plan';
  title: string;
  goal: string;
  steps: PlanStep[];
  doneWhen: PlanCriterion[];
  risks: string[];
}

export interface Checked {
  checked: boolean;
  text: string;
  detail: string;
}

/** The `plan` payload of a `plan.proposed` event (docs/api.md). */
export interface PlanProposal {
  title?: string;
  goal?: string;
  steps?: { title: string; files?: string[]; change?: string }[];
  doneWhen?: { text: string; check?: string }[];
  risks?: string[];
}

/** The `report` payload of a `report.submitted` event (docs/api.md). */
export interface ReportProposal {
  title?: string;
  summary?: string;
  steps?: { title: string; done?: boolean; note?: string }[];
  doneWhen?: { text: string; met?: boolean; how?: string }[];
  changed?: { path: string; what: string }[];
  test?: string;
  todo?: string;
}

/**
 * The plan a plan agent recorded with `propose_plan`, as the thread shows it:
 * the steps numbered in order, each criterion with its check command apart.
 */
export function planFromProposal(proposal: PlanProposal): Plan {
  const steps = (proposal.steps ?? []).map((step, i) => ({
    number: i + 1,
    title: step.title,
    files: step.files ?? [],
    change: step.change ?? '',
    verify: ''
  }));
  const doneWhen = (proposal.doneWhen ?? []).map((criterion) => ({ text: criterion.text, check: criterion.check ?? '' }));
  return {
    kind: 'plan',
    title: proposal.title ?? '',
    goal: proposal.goal ?? '',
    steps,
    doneWhen,
    risks: proposal.risks ?? []
  };
}

export interface NumberedCheck extends Checked {
  number: number;
}

// The unmet criteria first: what needs the human's eye opens the list.
function unmetFirst(items: NumberedCheck[]): NumberedCheck[] {
  return [...items.filter((item) => !item.checked), ...items.filter((item) => item.checked)];
}

export interface Report {
  kind: 'report';
  title: string;
  summary: string;
  steps: NumberedCheck[];
  doneWhen: NumberedCheck[];
  changed: { path: string; what: string }[];
  test: string[];
  todo: string[];
}

/**
 * The report a builder recorded with `submit_report`, as the thread shows it:
 * the same shape the legacy text report parses to — the steps numbered from 1,
 * the note and the "how" in the detail.
 */
export function reportFromSubmission(proposal: ReportProposal): Report {
  const steps = (proposal.steps ?? []).map((step, i) => ({
    number: i + 1,
    checked: step.done ?? false,
    text: step.title,
    detail: step.note ?? ''
  }));
  const doneWhen = unmetFirst(
    (proposal.doneWhen ?? []).map((criterion, i) => ({
      number: i + 1,
      checked: criterion.met ?? false,
      text: criterion.text,
      detail: criterion.how ?? ''
    }))
  );
  return {
    kind: 'report',
    title: proposal.title ?? '',
    summary: proposal.summary ?? '',
    steps,
    doneWhen,
    changed: proposal.changed ?? [],
    test: itemsOf(proposal.test ?? ''),
    todo: itemsOf(proposal.todo ?? '')
  };
}

type Token = { kind: 'heading'; text: string } | { kind: 'field'; text: string };

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// The models drift between ## and ### for the same section.
function headingOf(line: string): string | null {
  const m = line.match(/^#{2,3}\s+(.+?)\s*$/);
  return m ? m[1] : null;
}

// The models drift between - and * bullets.
function bulletOf(line: string): string | null {
  const m = line.match(/^\s*[-*]\s+(.+?)\s*$/);
  return m ? m[1] : null;
}

// [x], [X], or [ ]; null when the line is not a checkbox.
function checkboxOf(text: string): { checked: boolean; rest: string } | null {
  const m = text.match(/^\[([ xX])\]\s*(.*)$/);
  return m ? { checked: m[1] !== ' ', rest: m[2].trim() } : null;
}

// A label that may be bolded around it: **Files:**
function labelOf(text: string, label: string): string | null {
  const re = new RegExp('^\\s*(?:\\*\\*)?\\s*' + escapeRegExp(label) + '\\s*:(?:\\*\\*)?\\s*(.*)$', 'i');
  const m = text.match(re);
  return m ? m[1].trim() : null;
}

// The —, –, or - separator after a step title; detail is '' when there is none.
function splitDetail(text: string): { text: string; detail: string } {
  const dash = text.match(/^(.*?)\s[—–]\s+(.*)$/);
  if (dash) return { text: dash[1].trim(), detail: dash[2].trim() };
  const hyphen = text.match(/^(.*?)\s-\s+(.*)$/);
  if (hyphen) return { text: hyphen[1].trim(), detail: hyphen[2].trim() };
  return { text: text.trim(), detail: '' };
}

// A legacy criterion line: the text, and the trailing check command when present.
function criterionOf(text: string): PlanCriterion {
  const m = text.match(/^(.*?)\s+[—–-]\s*check:\s*`([^`]+)`\s*\.?$/i);
  return m ? { text: m[1].trim(), check: m[2].trim() } : { text: text.trim(), check: '' };
}

// The backticked items of a Files line; fall back to comma-splitting, keeping (new).
function filesOf(text: string): string[] {
  const backticked = text.match(/`([^`]+)`/g);
  if (backticked && backticked.length > 0) return backticked.map((s) => s.slice(1, -1).trim());
  return text
    .split(',')
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

function nothingOf(text: string): string {
  return /^(nothing|rien)\.?$/i.test(text.trim()) ? '' : text.trim();
}

// A free-text field as list items: its lines (bullets stripped) or, on one line, its sentences.
function itemsOf(text: string): string[] {
  const kept = nothingOf(text);
  if (kept === '') return [];
  const lines = kept
    .split('\n')
    .map((line) => line.replace(/^\s*(?:[-*]|\d+[.)])\s+/, '').trim())
    .filter((line) => line.length > 0);
  if (lines.length > 1) return lines;
  return kept
    .split(/(?<=[.!?])\s+(?=[A-ZÀ-ÖØ-Þ«"`])/)
    .map((sentence) => sentence.trim())
    .filter((sentence) => sentence.length > 0);
}

/** A path as its folder (with the trailing slash, '' at the root) and its file name. */
export function pathParts(path: string): { dir: string; name: string } {
  const cut = path.lastIndexOf('/') + 1;
  return { dir: path.slice(0, cut), name: path.slice(cut) };
}

function sectionOf(text: string): string {
  const t = text.toLowerCase().trim();
  if (t.startsWith('done when')) return 'done-when';
  if (t.startsWith('risks')) return 'risks';
  if (t.startsWith('steps')) return 'steps';
  if (t.startsWith('changed')) return 'changed';
  if (t.startsWith('next')) return 'next';
  return t;
}

// Headings and fields; a field swallows its following non-bullet lines (wrapped text).
function tokenize(lines: string[]): Token[] {
  const tokens: Token[] = [];
  for (const line of lines) {
    const h = headingOf(line);
    if (h !== null) {
      tokens.push({ kind: 'heading', text: h });
      continue;
    }
    const b = bulletOf(line);
    if (b !== null) {
      tokens.push({ kind: 'field', text: b });
      continue;
    }
    const trimmed = line.trim();
    if (trimmed === '') continue;
    const last = tokens[tokens.length - 1];
    if (last && last.kind === 'field') last.text += ' ' + trimmed;
    else tokens.push({ kind: 'field', text: trimmed });
  }
  return tokens;
}

function parsePlan(title: string, lines: string[]): Plan | null {
  const tokens = tokenize(lines);
  let goal = '';
  let section = '';
  let current: PlanStep | null = null;
  const steps: PlanStep[] = [];
  const doneWhen: PlanCriterion[] = [];
  const risks: string[] = [];

  for (const token of tokens) {
    if (token.kind === 'heading') {
      const step = token.text.match(/^Step\s+(\d+)\s*:\s*(.*)$/i);
      if (step) {
        current = { number: Number(step[1]), title: step[2].trim(), files: [], change: '', verify: '' };
        steps.push(current);
      } else {
        section = sectionOf(token.text);
        current = null;
      }
      continue;
    }
    if (current) {
      const files = labelOf(token.text, 'Files');
      if (files !== null) {
        current.files = filesOf(files);
        continue;
      }
      const change = labelOf(token.text, 'Change');
      if (change !== null) {
        current.change = change;
        continue;
      }
      const verify = labelOf(token.text, 'Verify');
      if (verify !== null) current.verify = verify;
      continue;
    }
    const g = labelOf(token.text, 'Goal');
    if (g !== null) {
      goal = g;
      continue;
    }
    if (section === 'done-when') {
      const cb = checkboxOf(token.text);
      doneWhen.push(criterionOf(cb ? cb.rest : token.text));
      continue;
    }
    if (section === 'risks') {
      risks.push(token.text);
    }
  }

  if (steps.length === 0) return null;
  return {
    kind: 'plan',
    title,
    goal,
    steps,
    doneWhen,
    risks: risks.filter((r) => !/^(none|aucun|aucune)\.?$/i.test(r.trim())),
  };
}

function parseReport(title: string, lines: string[]): Report | null {
  const tokens = tokenize(lines);
  let summary = '';
  let section = '';
  let test: string[] = [];
  let todo: string[] = [];
  const steps: NumberedCheck[] = [];
  const doneWhen: NumberedCheck[] = [];
  const changed: { path: string; what: string }[] = [];

  for (const token of tokens) {
    if (token.kind === 'heading') {
      section = sectionOf(token.text);
      continue;
    }
    const s = labelOf(token.text, 'Summary');
    if (s !== null) {
      summary = s;
      continue;
    }
    if (section === 'steps') {
      const cb = checkboxOf(token.text);
      const rest = cb ? cb.rest : token.text;
      const m = rest.match(/^Step\s+(\d+)\s*:\s*(.*)$/i);
      const number = m ? Number(m[1]) : steps.length + 1;
      const { text, detail } = splitDetail(m ? m[2] : rest);
      steps.push({ number, checked: cb ? cb.checked : false, text, detail });
      continue;
    }
    if (section === 'done-when') {
      const cb = checkboxOf(token.text);
      const rest = cb ? cb.rest : token.text;
      const { text, detail } = splitDetail(rest);
      doneWhen.push({ number: doneWhen.length + 1, checked: cb ? cb.checked : false, text, detail });
      continue;
    }
    if (section === 'changed') {
      const m = token.text.match(/^`([^`]+)`\s*:\s*(.*)$/);
      if (m) {
        changed.push({ path: m[1].trim(), what: m[2].trim() });
      } else {
        const idx = token.text.indexOf(':');
        if (idx > 0) changed.push({ path: token.text.slice(0, idx).trim(), what: token.text.slice(idx + 1).trim() });
      }
      continue;
    }
    if (section === 'next') {
      const t = labelOf(token.text, 'Test');
      if (t !== null) {
        test = itemsOf(t);
        continue;
      }
      const d = labelOf(token.text, 'To do');
      if (d !== null) todo = itemsOf(d);
    }
  }

  if (steps.length === 0 && doneWhen.length === 0) return null;
  return { kind: 'report', title, summary, steps, doneWhen: unmetFirst(doneWhen), changed, test, todo };
}

/** The text before the contract and the contract itself; null when the text holds none. */
export function splitContract(text: string): { before: string; contract: Plan | Report } | null {
  const lines = text.split('\n');
  let start = -1;
  let kind: 'plan' | 'report' = 'plan';
  let title = '';
  // The contract is the last such heading: the models may restate it mid-message.
  // A short qualifier before the colon ("Plan révisé :", "Revised Plan:") is the model's
  // wording, not another heading: the revised plan must render as the first one did.
  for (let i = lines.length - 1; i >= 0; i--) {
    const m = lines[i].match(/^#{2,3}\s+(?:[^\s:]+\s+)?(Plan|Report)(?:\s+[^:]{1,20}?)?\s*:\s*(.+?)\s*$/);
    if (m) {
      start = i;
      kind = m[1] === 'Plan' ? 'plan' : 'report';
      title = m[2].trim();
      break;
    }
  }
  if (start < 0) return null;
  const before = lines.slice(0, start).join('\n').trim();
  const contract = kind === 'plan' ? parsePlan(title, lines.slice(start + 1)) : parseReport(title, lines.slice(start + 1));
  if (!contract) return null;
  return { before, contract };
}

/** The served pending plan as the decision bar shows it; null when none waits. */
export function pendingPlanOf(
  served: ServedPendingPlan | null
): { id: string; plan: Plan; markdown: string; preflight: PreflightResultView[] } | null {
  if (served === null) return null;
  return {
    id: served.callId,
    plan: planFromProposal(served.plan),
    markdown: served.markdown,
    preflight: served.preflight.map(({ exitCode, ...result }) => ({ ...result, exitCode: exitCode ?? undefined }))
  };
}

/** The last message the human typed (not one fennec sent on their behalf). */
export function lastHumanMessage(items: ThreadItem[]): string | undefined {
  return [...items]
    .reverse()
    .find(
      (item): item is Extract<typeof item, { kind: 'user' }> =>
        item.kind === 'user' && !item.isSystemGenerated
    )?.content;
}
