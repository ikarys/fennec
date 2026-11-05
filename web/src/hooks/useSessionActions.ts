import { useState } from 'react';
import { useT } from '../i18n';
import type { ApprovalChoice, PendingApproval, PendingWaiver } from '../lib/api/sessions';
import {
  answerRuleApproval,
  answerWaiver,
  cancelQueued,
  compactSession,
  replayMessage,
  retrySession,
  stopSession,
} from '../lib/api';

interface UseSessionActionsArgs {
  /** The session to act on, or null when no session is selected. */
  sessionId: string | null;
  /** The pending rule approval, if any. */
  pendingApproval: PendingApproval | null;
  /** The pending waiver, if any. */
  pendingWaiver: PendingWaiver | null;
  /** The page's send-error state: action failures are shown there. */
  onError: (message: string | null) => void;
}

/**
 * Session actions: stop, compact, replay, approvals, queue, waiver, retry.
 */
export function useSessionActions({
  sessionId,
  pendingApproval,
  pendingWaiver,
  onError,
}: UseSessionActionsArgs) {
  const t = useT();
  const [compactError, setCompactError] = useState<string | null>(null);

  async function report(label: string, action: () => Promise<void>) {
    onError(null);
    try {
      await action();
    } catch (error) {
      onError(error instanceof Error ? error.message : label);
    }
  }

  async function handleStop() {
    if (!sessionId) return;
    try {
      await stopSession(sessionId);
    } catch {
      // Stop is best-effort; the event stream reports the final state.
    }
  }

  async function handleCompact() {
    if (!sessionId) return;
    setCompactError(null);
    try {
      await compactSession(sessionId);
    } catch (error) {
      setCompactError(error instanceof Error ? error.message : t('sessionCompactError'));
    }
  }

  async function handleReplay(messageId: string) {
    if (!sessionId) return;
    await report(t('sessionReplayError'), () => replayMessage(sessionId, messageId));
  }

  async function handleApproval(choice: ApprovalChoice, pattern?: string) {
    if (!sessionId || !pendingApproval) return;
    await report(t('sessionApprovalError'), () =>
      answerRuleApproval(sessionId, pendingApproval.callId, choice, pattern),
    );
  }

  async function handleCancelQueued(queueId: string) {
    if (!sessionId) return;
    await report(t('sessionCancelQueuedError'), () => cancelQueued(sessionId, queueId));
  }

  async function handleWaiver(hours: number | null) {
    if (!sessionId || !pendingWaiver) return;
    await report(t('sessionWaiverError'), () => answerWaiver(sessionId, pendingWaiver.callId, hours));
  }

  async function handleRetry(): Promise<void> {
    if (!sessionId) return;
    await retrySession(sessionId);
  }

  return {
    compactError,
    handleStop,
    handleCompact,
    handleReplay,
    handleApproval,
    handleCancelQueued,
    handleWaiver,
    handleRetry,
  };
}
