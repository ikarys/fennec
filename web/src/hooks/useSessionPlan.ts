// The plan that waits for the human: the actions the decision bar offers on it (launch, revise, mark the idea ready) and taking a handed plan back.
import { useMemo, useState } from 'react';
import { sendMessage, startWorkflow } from '../lib/api';
import type { SessionDetail } from '../lib/api';
import type { ThreadView } from '../lib/fold';
import type { ServedThread } from '../lib/api';
import { addPlannedIdea, applySheet, frameIdea, markIdeaReady } from '../lib/ideas';
import { lastHumanMessage, pendingPlanOf } from '../lib/plan';
import { useT } from '../i18n';
import { planAnchorId } from '../components/session/PendingPlan';
import type { PlanDecisionActions } from '../components/session/PlanDecision';
import {
  DEFAULT_WORKFLOW_ID,
  defaultWorkflowSelection,
} from '../components/workflow/session/WorkflowPicker';
import type { useProjectOptions } from './useProjectOptions';

type ProjectOptions = ReturnType<typeof useProjectOptions>;

interface UseSessionPlanArgs {
  sessionId: string | null;
  detail: SessionDetail | null;
  view: ThreadView;
  served: ServedThread;
  isChat: boolean;
  running: boolean;
  workflows: ProjectOptions['workflows'];
  setWorkflowSelection: ProjectOptions['setWorkflowSelection'];
  textareaRef: React.RefObject<HTMLTextAreaElement | null>;
  /** Switches the agent; resolves true when it changed. */
  handleModeChange: (agent: string) => Promise<boolean>;
  setSendError: (error: string | null) => void;
  reloadDetail: () => void;
  refreshThread: () => void;
}

export function useSessionPlan({
  sessionId,
  detail,
  view,
  served,
  isChat,
  running,
  workflows,
  setWorkflowSelection,
  textareaRef,
  handleModeChange,
  setSendError,
  reloadDetail,
  refreshThread,
}: UseSessionPlanArgs) {
  const t = useT();
  const run = view.run;
  const [planBuildInFlight, setPlanBuildInFlight] = useState(false);
  // The plan last put in the backlog: its button says so until a new plan comes.
  const [backlogSegment, setBacklogSegment] = useState<string | null>(null);

  // The server serves the plan that waits whole: the thread holds only the log's tail, an
  // older plan would be missing from it. A question after the plan keeps it offered.
  const pendingPlan = useMemo(() => pendingPlanOf(served.pendingPlan), [served.pendingPlan]);
  const planSource = pendingPlan ? { id: pendingPlan.id, content: pendingPlan.markdown } : null;

  async function runPlanBuildAlone() {
    const modeChanged = await handleModeChange("builder");
    // Sent directly, not through handleSend: a workflow picked in the
    // composer would otherwise take this message as its task.
    if (modeChanged && sessionId) {
      await sendMessage(sessionId, t('sessionPageExecutePlanAbove'));
    }
  }

  async function handlePlanBuild(worktree: boolean) {
    if (planBuildInFlight) return;
    setPlanBuildInFlight(true);
    setSendError(null);
    try {
      if (
        sessionId &&
        workflows.some((workflow) => workflow.id === DEFAULT_WORKFLOW_ID) &&
        planSource
      ) {
        const task = lastHumanMessage(view.items) ?? t('sessionPageExecuteApprovedPlan');
        await startWorkflow(
          sessionId,
          DEFAULT_WORKFLOW_ID,
          task,
          false,
          worktree,
          [],
          planSource.content,
        );
        // The next message gets the default workflow again.
        setWorkflowSelection(defaultWorkflowSelection(workflows));
      } else {
        await runPlanBuildAlone();
      }
    } catch (err) {
      setSendError(
        err instanceof Error ? err.message : t('sessionPageSendMessageError'),
      );
    } finally {
      setPlanBuildInFlight(false);
    }
  }

  async function handlePlanBuildAlone() {
    if (planBuildInFlight) return;
    setPlanBuildInFlight(true);
    setSendError(null);
    try {
      await runPlanBuildAlone();
    } catch (err) {
      setSendError(
        err instanceof Error ? err.message : t('sessionPageSendMessageError'),
      );
    } finally {
      setPlanBuildInFlight(false);
    }
  }

  async function handleIdeaReady() {
    if (!detail || !planSource) return;
    setSendError(null);
    try {
      if (detail.idea?.hasSheet) {
        await applySheet(detail.project.id, detail.idea.id);
      } else if (detail.idea) {
        await markIdeaReady(detail.project.id, detail.idea.id, planSource.content);
      } else {
        await addPlannedIdea(detail.project.id, detail.title, detail.id, planSource.content);
      }
      setBacklogSegment(planSource.id);
      reloadDetail();
      // A backlog event is not in the session's log: the bar learns of it by asking again.
      refreshThread();
    } catch (err) {
      setSendError(err instanceof Error ? err.message : t('unknownError'));
    }
  }

  async function handleTakeBackPlan(ideaId: string) {
    if (!detail) return;
    setSendError(null);
    try {
      await frameIdea(detail.project.id, ideaId, detail.id);
      reloadDetail();
      refreshThread();
    } catch (err) {
      setSendError(err instanceof Error ? err.message : t('unknownError'));
    }
  }

  function handlePlanRevise() {
    // The revision is an ordinary message to the planner, not a workflow start.
    setWorkflowSelection((selection) => ({ ...selection, workflowId: null }));
    textareaRef.current?.focus();
  }

  const planActions: PlanDecisionActions | null =
    pendingPlan && !view.pendingQuestion && (!run || run.finished)
      ? {
          onBuild: handlePlanBuild,
          onBuildAlone: handlePlanBuildAlone,
          onRevise: handlePlanRevise,
          onShowInThread: () =>
            document.getElementById(planAnchorId(pendingPlan.id))?.scrollIntoView({ behavior: "smooth", block: "start" }),
          // Without the default workflow, Launch falls back to the builder alone, in the folder.
          worktreeAvailable: !isChat && workflows.some((workflow) => workflow.id === DEFAULT_WORKFLOW_ID),
          ideaReady: detail
            ? {
                onMark: () => void handleIdeaReady(),
                idea: detail.idea?.hasSheet
                  ? backlogSegment === pendingPlan.id
                    ? "applied"
                    : "sheet"
                  : backlogSegment === pendingPlan.id
                    ? "saved"
                    : detail.idea?.state === "ready"
                      ? "ready"
                      : detail.idea
                        ? "framing"
                        : "none",
              }
            : undefined,
          disabled: running || planBuildInFlight,
        }
      : null;

  return { pendingPlan, planActions, handleTakeBackPlan };
}
