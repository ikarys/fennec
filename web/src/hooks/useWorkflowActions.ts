import type { RunView } from '../lib/runView';
import {
  answerWorkflow,
  continueWorkflow,
  pauseWorkflow,
  resumeWorkflow,
  stopWorkflow,
} from '../lib/api';

interface UseWorkflowActionsArgs {
  /** The session to act on, or null when no session is selected. */
  sessionId: string | null;
  /** The workflow run to act on, if any. */
  run: RunView | null | undefined;
}

/**
 * Workflow actions: stop, pause, answer, continue, resume.
 */
export function useWorkflowActions({
  sessionId,
  run,
}: UseWorkflowActionsArgs) {
  async function handleStopWorkflow() {
    if (!sessionId || !run) return;
    try {
      await stopWorkflow(sessionId, run.runId);
    } catch {
      // Stop is best-effort; the event stream reports the final state.
    }
  }

  async function handlePauseWorkflow() {
    if (!sessionId || !run) return;
    try {
      await pauseWorkflow(sessionId, run.runId);
    } catch {
      // Pause is best-effort; the event stream reports the final state.
    }
  }

  async function handleAnswerWorkflow(choice: string, comment: string) {
    if (!sessionId || !run) return;
    await answerWorkflow(sessionId, run.runId, choice, comment);
  }

  async function handleContinueWorkflow(comment: string) {
    if (!sessionId || !run) return;
    await continueWorkflow(sessionId, run.runId, comment);
  }

  async function handleResumeWorkflow() {
    if (!sessionId || !run) return;
    await resumeWorkflow(sessionId, run.runId);
  }

  return {
    handleStopWorkflow,
    handlePauseWorkflow,
    handleAnswerWorkflow,
    handleContinueWorkflow,
    handleResumeWorkflow,
  };
}
