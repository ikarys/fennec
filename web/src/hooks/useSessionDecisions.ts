// What waits for the human in the decision bar: the decisions that block the agent, the proposals that do not, and which row is open.
import { useEffect, useMemo, useState } from 'react';
import type { ServedThread, SessionDetail } from '../lib/api';
import type { ThreadView } from '../lib/fold';
import type { pendingPlanOf } from '../lib/plan';
import type { runWaiting } from '../lib/runView';
import type { IntegrationInvite } from '../components/session/IntegrationInvite';
import type { PlanDecisionActions } from '../components/session/PlanDecision';

interface UseSessionDecisionsArgs {
  sessionId: string | null;
  detail: SessionDetail | null;
  view: ThreadView;
  served: ServedThread;
  running: boolean;
  waiting: ReturnType<typeof runWaiting>;
  pendingPlan: ReturnType<typeof pendingPlanOf>;
  planActions: PlanDecisionActions | null;
  openIntegration: () => void;
  tryIt: () => void;
}

export function useSessionDecisions({
  sessionId,
  detail,
  view,
  served,
  running,
  waiting,
  pendingPlan,
  planActions,
  openIntegration,
  tryIt,
}: UseSessionDecisionsArgs) {
  // The decision bar opens one row at a time; the id is the decision's (a plan's call id).
  const [openDecision, setOpenDecision] = useState<string | null>(null);

  // What blocks the agent until the human answers; a node's session is answered in its parent, unless its step asked the human.
  const answerable = !running && !waiting && (!detail?.parentId || !!detail.workflowAsks);
  const blocking = {
    approval: answerable ? served.pendingApproval : null,
    waiver: answerable ? served.pendingWaiver : null,
    question: answerable ? view.pendingQuestion : null,
    run: waiting && !detail?.parentId ? waiting : null,
    retry: answerable && view.lastDone === "error" && !view.pendingQuestion,
  };
  const runDecisionId = blocking.run
    ? `run-${blocking.run.kind === "human" ? blocking.run.nodeId : blocking.run.kind}`
    : null;
  // The last run ended done and its work is not integrated yet: the bar invites to review it.
  const runWorkReady = useMemo(() => {
    for (let i = view.items.length - 1; i >= 0; i -= 1) {
      const item = view.items[i];
      if (item?.id.startsWith("integration-applied-")) return false;
      if (item && item.kind === "workflow-end") return item.outcome === "done";
    }
    return false;
  }, [view.items]);
  const integrationInvite = useMemo<IntegrationInvite | null>(() => {
    // No commit on the agent's branch: nothing to review, no invitation.
    if (!sessionId || !detail?.worktree?.commits) return null;
    return { open: openIntegration, tryIt };
  }, [sessionId, detail?.worktree?.commits, openIntegration, tryIt]);
  // The agent's proposals wait in the bar until taken; none of them blocks the agent.
  const offered = detail && sessionId && !running ? detail : null;
  const proposals = {
    audit: offered?.audit && offered.audit.findings.some((finding) => !offered.audit!.taken.includes(finding.number)) ? offered.audit : null,
    tasks: offered?.proposedTasks.some((task) => task.ideaId === null) ? offered.proposedTasks : null,
    agentsMd: offered?.agentsMdDraft && !offered.agentsMdDraft.written ? offered.agentsMdDraft : null,
    rules: offered?.ruleProposal && !offered.ruleProposal.added && offered.ruleProposal.lines.length > 0 ? offered.ruleProposal : null,
    suggestion: offered?.agentSuggestion && offered.project.id ? offered.agentSuggestion : null,
    integration: !running && runWorkReady ? integrationInvite : null,
  };
  const decisionCount =
    [blocking.approval, blocking.waiver, blocking.question, blocking.run, blocking.retry].filter(Boolean).length +
    (!detail?.parentId && pendingPlan && planActions ? 1 : 0) +
    Object.values(proposals).filter(Boolean).length;
  const blockingId =
    blocking.approval?.callId ??
    blocking.waiver?.callId ??
    blocking.question?.callId ??
    runDecisionId ??
    (blocking.retry ? "retry" : null);
  // A blocking decision opens on arrival: the agent waits on it.
  useEffect(() => {
    if (blockingId) setOpenDecision(blockingId);
  }, [blockingId]);
  const toggleDecision = (id: string) => setOpenDecision((open) => (open === id ? null : id));

  return {
    blocking,
    runDecisionId,
    proposals,
    decisionCount,
    integrationInvite,
    openDecision,
    closeDecisions: () => setOpenDecision(null),
    toggleDecision,
  };
}

export type SessionDecisions = ReturnType<typeof useSessionDecisions>;
