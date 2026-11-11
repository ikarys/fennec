// The session's decision bar: the blocking decisions, the plan and the agent's proposals, one row open at a time.
import type { SessionDetail } from '../../lib/api';
import type { pendingPlanOf } from '../../lib/plan';
import type { SessionDecisions } from '../../hooks/useSessionDecisions';
import type { useSessionActions } from '../../hooks/useSessionActions';
import type { useWorkflowActions } from '../../hooks/useWorkflowActions';
import { RunDecision } from '../workflow/session/RunDecision';
import { AgentsMdDecision } from './AgentsMdDecision';
import { AgentSuggestionDecision } from './AgentSuggestionDecision';
import { ApprovalDecision } from './ApprovalDecision';
import { AuditDecision } from './AuditDecision';
import { DecisionBar } from './DecisionBar';
import { IntegrationDecision } from './IntegrationDecision';
import { PlanDecision, type PlanDecisionActions } from './PlanDecision';
import { ProposedTasksDecision } from './ProposedTasksDecision';
import { QuestionDecision } from './QuestionDecision';
import { RetryDecision } from './RetryDecision';
import { RuleProposalDecision } from './RuleProposalDecision';
import { WaiverDecision } from './WaiverDecision';

interface SessionDecisionBarProps {
  decisions: SessionDecisions;
  sessionId: string | null;
  detail: SessionDetail | null;
  running: boolean;
  pendingPlan: ReturnType<typeof pendingPlanOf>;
  planActions: PlanDecisionActions | null;
  actions: ReturnType<typeof useSessionActions>;
  workflowActions: ReturnType<typeof useWorkflowActions>;
  runComment: string;
  onRunCommentChange: (comment: string) => void;
  /** Sends a quick answer as an ordinary message. */
  onAnswer: (answer: string) => void;
  onDetailChanged: () => void;
}

export function SessionDecisionBar({
  decisions,
  sessionId,
  detail,
  running,
  pendingPlan,
  planActions,
  actions,
  workflowActions,
  runComment,
  onRunCommentChange,
  onAnswer,
  onDetailChanged,
}: SessionDecisionBarProps) {
  const { blocking, runDecisionId, proposals, openDecision, toggleDecision } = decisions;
  return (
    <DecisionBar count={decisions.decisionCount} onEscape={decisions.closeDecisions}>
      {blocking.approval && (
        <ApprovalDecision
          approval={blocking.approval}
          onAnswer={actions.handleApproval}
          disabled={running}
          open={openDecision === blocking.approval.callId}
          onToggle={() => toggleDecision(blocking.approval!.callId)}
        />
      )}
      {blocking.waiver && (
        <WaiverDecision
          waiver={blocking.waiver}
          onAnswer={actions.handleWaiver}
          disabled={running}
          open={openDecision === blocking.waiver.callId}
          onToggle={() => toggleDecision(blocking.waiver!.callId)}
        />
      )}
      {blocking.question && (
        <QuestionDecision
          question={blocking.question}
          onAnswer={onAnswer}
          disabled={running}
          open={openDecision === blocking.question.callId}
          onToggle={() => toggleDecision(blocking.question!.callId)}
        />
      )}
      {blocking.run && runDecisionId && (
        <RunDecision
          waiting={blocking.run}
          onAnswer={workflowActions.handleAnswerWorkflow}
          onContinue={workflowActions.handleContinueWorkflow}
          onResume={workflowActions.handleResumeWorkflow}
          onStop={workflowActions.handleStopWorkflow}
          disabled={running}
          comment={runComment}
          onCommentChange={onRunCommentChange}
          open={openDecision === runDecisionId}
          onToggle={() => toggleDecision(runDecisionId)}
        />
      )}
      {blocking.retry && (
        <RetryDecision
          onRetry={actions.handleRetry}
          disabled={running}
          open={openDecision === "retry"}
          onToggle={() => toggleDecision("retry")}
        />
      )}
      {!detail?.parentId && pendingPlan && planActions && (
        <PlanDecision
          plan={pendingPlan.plan}
          preflight={pendingPlan.preflight}
          actions={planActions}
          open={openDecision === pendingPlan.id}
          onToggle={() => toggleDecision(pendingPlan.id)}
        />
      )}
      {proposals.integration && (
        <IntegrationDecision
          invite={proposals.integration}
          open={openDecision === "integration"}
          onToggle={() => toggleDecision("integration")}
        />
      )}
      {proposals.audit && sessionId && (
        <AuditDecision
          audit={proposals.audit}
          sessionId={sessionId}
          projectId={detail?.project.id ?? null}
          onCreated={onDetailChanged}
          open={openDecision === "audit"}
          onToggle={() => toggleDecision("audit")}
        />
      )}
      {proposals.tasks && sessionId && (
        <ProposedTasksDecision
          tasks={proposals.tasks}
          sessionId={sessionId}
          onAccepted={onDetailChanged}
          open={openDecision === "tasks"}
          onToggle={() => toggleDecision("tasks")}
        />
      )}
      {proposals.agentsMd && sessionId && (
        <AgentsMdDecision
          draft={proposals.agentsMd}
          sessionId={sessionId}
          onWritten={onDetailChanged}
          open={openDecision === "agents-md"}
          onToggle={() => toggleDecision("agents-md")}
        />
      )}
      {proposals.rules && sessionId && (
        <RuleProposalDecision
          proposal={proposals.rules}
          sessionId={sessionId}
          onAdded={onDetailChanged}
          open={openDecision === "rules"}
          onToggle={() => toggleDecision("rules")}
        />
      )}
      {proposals.suggestion && sessionId && detail?.project.id && (
        <AgentSuggestionDecision
          suggestion={proposals.suggestion}
          sessionId={sessionId}
          projectId={detail.project.id}
          open={openDecision === "suggestion"}
          onToggle={() => toggleDecision("suggestion")}
        />
      )}
    </DecisionBar>
  );
}
