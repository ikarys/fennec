// The contexts a session's subtree reads: previews, canvas, file links, pending plan, integration invite and usage.
import { useMemo, type ReactNode } from 'react';
import type { SessionDetail, WorkflowUsage } from '../../lib/api';
import type { CallUsage } from '../../lib/api';
import { changesPath, editorPath } from '../../lib/route';
import { CanvasOpenerContext, CanvasSessionContext } from '../canvas/CanvasContext';
import {
  PreviewContext,
  PreviewWorkdirContext,
  type PreviewTarget,
} from '../preview/PreviewContext';
import { CallUsageContext, WorkflowUsageContext } from './CallUsage';
import { FileLinkContext } from './FileLinkContext';
import { IntegrationInviteContext, type IntegrationInvite } from './IntegrationInvite';
import { PendingPlanContext } from './PendingPlan';

interface SessionProvidersProps {
  sessionId: string | null;
  /** The project the page was opened on, before the detail arrives. */
  linkProjectId: string | null;
  detail: SessionDetail | null;
  isChat: boolean;
  onPreview: (target: PreviewTarget) => void;
  onOpenCanvas: (artifactId: number) => void;
  pendingPlanCallId: string | null;
  integrationInvite: IntegrationInvite | null;
  calls: ReadonlyMap<string, CallUsage>;
  workflowUsage: WorkflowUsage;
  children: ReactNode;
}

export function SessionProviders({
  sessionId,
  linkProjectId,
  detail,
  isChat,
  onPreview,
  onOpenCanvas,
  pendingPlanCallId,
  integrationInvite,
  calls,
  workflowUsage,
  children,
}: SessionProvidersProps) {
  const hasCommits = detail?.worktree?.commits;
  const fileLink = useMemo(
    () =>
      sessionId && linkProjectId && !isChat
        ? (path: string) =>
            hasCommits ? changesPath(sessionId, path) : editorPath(linkProjectId, path)
        : null,
    [sessionId, linkProjectId, isChat, hasCommits],
  );
  return (
    <PreviewContext.Provider value={onPreview}>
      <CanvasOpenerContext.Provider value={onOpenCanvas}>
        <CanvasSessionContext.Provider value={sessionId ?? undefined}>
          <FileLinkContext.Provider value={fileLink}>
            {/* The agent writes in its worktree when it has one: paths are relative to it, and the preview route resolves them there too. */}
            <PreviewWorkdirContext.Provider value={detail?.worktree?.path ?? detail?.project.workdir}>
              <PendingPlanContext.Provider value={pendingPlanCallId}>
                <IntegrationInviteContext.Provider value={integrationInvite}>
                  <CallUsageContext.Provider value={calls}>
                    <WorkflowUsageContext.Provider value={workflowUsage}>
                      {children}
                    </WorkflowUsageContext.Provider>
                  </CallUsageContext.Provider>
                </IntegrationInviteContext.Provider>
              </PendingPlanContext.Provider>
            </PreviewWorkdirContext.Provider>
          </FileLinkContext.Provider>
        </CanvasSessionContext.Provider>
      </CanvasOpenerContext.Provider>
    </PreviewContext.Provider>
  );
}
