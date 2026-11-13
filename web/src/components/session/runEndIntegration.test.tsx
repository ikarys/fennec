// The finished run's work is offered once: in the decision bar's integration
// row. The run end card shows the outcome only, and the bar's row must put
// the integrate button in evidence.
import { describe, it, expect } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import type { ThreadItem } from '../../lib/threadView';
import type { IntegrationInvite } from './IntegrationInvite';
import { IntegrationDecision } from './IntegrationDecision';
import { TryOrIntegrate } from './TryOrIntegrate';
import { RunEndCard } from '../workflow/session/RunEndCard';

type WorkflowEndItem = Extract<ThreadItem, { kind: 'workflow-end' }>;

const invite: IntegrationInvite = { open: () => {}, tryIt: () => {} };

const doneItem: WorkflowEndItem = { kind: 'workflow-end', id: 'workflow-finished-0', runId: 'run-1', outcome: 'done' };

/** The <button> blocks of a static markup, in document order. */
function buttons(markup: string): string[] {
  return [...markup.matchAll(/<button[\s\S]*?<\/button>/g)].map((match) => match[0]);
}

/** The run end card, as the thread renders it: outcome only, no actions. */
function renderRunEndCard(): string {
  return renderToStaticMarkup(<RunEndCard item={doneItem} />);
}

describe('the finished run\'s try/integrate actions', () => {
  it('appear once: in the decision bar row, not in the run end card', () => {
    const barRow = renderToStaticMarkup(<IntegrationDecision invite={invite} open={false} onToggle={() => {}} />);
    const all = renderRunEndCard() + barRow;
    expect(all.split('Try it').length - 1).toBe(1);
    expect(all.split('Review and integrate').length - 1).toBe(1);
  });

  it('keep the run end card\'s outcome', () => {
    const card = renderRunEndCard();
    expect(card).toContain('Workflow finished');
    expect(card).toContain('verified');
  });
});

describe('TryOrIntegrate', () => {
  it('emphasizes the primary action: accented and bigger, the other stays quiet and small', () => {
    const [tryButton, reviewButton] = buttons(
      renderToStaticMarkup(<TryOrIntegrate invite={invite} />),
    );
    expect(tryButton).toContain('Try it');
    expect(reviewButton).toContain('Review and integrate');
    expect(reviewButton).toContain('bg-accent');
    expect(reviewButton).toContain('h-8');
    expect(tryButton).toContain('bg-raised');
    expect(tryButton).toContain('h-7');
  });
});
