import { useCallback, useEffect, useRef, useState } from "react";
import type { Project, SessionState } from "../lib/api";
import { runWaiting } from "../lib/runView";
import { sessionStatus } from "../lib/sessionStatus";
import { useLang, useT } from "../i18n";
import { useSessionThread } from "../hooks/useSessionThread";
import { useSessionDetail } from "../hooks/useSessionDetail";
import { useSessionTabSync } from "../hooks/useSessionTabSync";
import { useProjectOptions } from "../hooks/useProjectOptions";
import { useModeAndModel } from "../hooks/useModeAndModel";
import { useAgentChoice } from "../hooks/useAgentChoice";
import { useScrollFollow } from "../hooks/useScrollFollow";
import { useSessionActions } from "../hooks/useSessionActions";
import { useWorkflowActions } from "../hooks/useWorkflowActions";
import { useSessionMcp } from "../hooks/useSessionMcp";
import { useComposerDraft } from "../hooks/useComposerDraft";
import { useComposerPrefill } from "../hooks/useComposerPrefill";
import { useSessionSend } from "../hooks/useSessionSend";
import { useSessionPlan } from "../hooks/useSessionPlan";
import { useSessionDecisions } from "../hooks/useSessionDecisions";
import { useJumpToNode } from "../hooks/useJumpToNode";
import { Composer } from "../components/session/Composer";
import { RunningStrip } from "../components/session/RunningStrip";
import { modelShortName } from "../components/session/ModelPicker";
import { ContextPanel } from "../components/session/ContextPanel";
import { QueuedMessages } from "../components/session/QueuedMessages";
import { HandedPlanNote } from "../components/session/HandedPlanNote";
import { SessionDecisionBar } from "../components/session/SessionDecisionBar";
import { SessionHeader } from "../components/session/SessionHeader";
import { SessionOverlays } from "../components/session/SessionOverlays";
import { SessionProviders } from "../components/session/SessionProviders";
import { SessionThreadPane } from "../components/session/SessionThreadPane";
import { WorkflowNodeNotice } from "../components/session/WorkflowNodeNotice";
import type { PreviewTarget } from "../components/preview/PreviewContext";
import { RunRail } from "../components/workflow/session/RunRail";

interface SessionPageProps {
  sessionId: string | null;
  projectId: string | null;
  /** A workflow opened from the editor's test button: preselected, step by step. */
  testWorkflowId: string | null;
  /** A backlog idea framed from the project page: its text prefills the composer, the first send links the session to it. */
  frameIdeaId: string | null;
  /** A session whose agent suggested this one: its brief prefills the composer and its agent is preselected. */
  fromSessionId: string | null;
  pseudoTabId: string | null;
  projects: Project[];
  onSessionCreated: (newId: string, pseudoId: string | null) => void;
  /** Tabs and the palette show session states from the projects list. */
  onRunningChanged: () => void;
  /** The session's name or chat link changed; the tabs and the chats page read them from projects. */
  onRenamed: () => void;
  /** Set in split view: closes this pane, the other one stays. */
  onClose?: () => void;
}

export function SessionPage({
  sessionId,
  projectId,
  testWorkflowId,
  frameIdeaId,
  fromSessionId,
  pseudoTabId,
  projects,
  onSessionCreated,
  onRunningChanged,
  onRenamed,
  onClose,
}: SessionPageProps) {
  const lang = useLang();
  const t = useT();
  const {
    view,
    hasMore,
    loadingOlder,
    loadOlder,
    skills,
    usage,
    calls,
    workflowUsage,
    context,
    vision,
    served,
    refreshThread,
  } = useSessionThread(sessionId);
  const running = view.running;
  const run = view.run;
  const waiting = runWaiting(run, running, view.lastDone);
  const { detail, detailVersion, reloadDetail } = useSessionDetail(sessionId, running);
  useSessionTabSync(running, view.name, onRunningChanged, onRenamed);

  const [sendError, setSendError] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewTarget | null>(null);
  const [reveal, setReveal] = useState({ id: 'session', key: 0 });
  const [canvasId, setCanvasId] = useState<number | null>(null);
  const [runComment, setRunComment] = useState("");
  const [integrationOpen, setIntegrationOpen] = useState(false);
  const [integrationVersion, setIntegrationVersion] = useState(0);
  const openIntegration = useCallback(() => setIntegrationOpen(true), []);
  const revealPanel = useCallback((id: string) => setReveal((r) => ({ id, key: r.key + 1 })), []);
  const revealServers = useCallback(() => revealPanel('servers'), [revealPanel]);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // The detail is fetched once; the event stream says how a turn ended.
  const state: SessionState = running
    ? "running"
    : ((view.lastDone as SessionState | undefined) ?? detail?.state ?? "idle");
  const status = sessionStatus(lang, state, run, waiting);
  // A session framing a backlog idea only talks: its messages must not start
  // the default build workflow (a "go on" there once ran a whole build).
  const framingIdea = detail?.idea?.state === "framing";
  const { commands, agents, workflows, workflowSelection, setWorkflowSelection } =
    useProjectOptions({
      projectId: detail?.project.id ?? projectId,
      testWorkflowId,
      frameIdeaId,
      framingIdea,
    });
  // A chat has no folder: it starts in `builder` (talk, act on request), not
  // in the project's framing agent, and the header shows "Chat".
  const isChat = detail?.project.chat ?? projectId === "chat";
  const {
    modelChoices,
    mode,
    currentModel,
    pendingModel,
    pendingEffort,
    effort,
    acceptedEfforts,
    defaultEffort,
    handleModeChange,
    handleModelChange,
    handleEffortChange,
  } = useModeAndModel({
    sessionId,
    viewMode: view.mode,
    detailMode: detail?.mode ?? null,
    fallbackMode: isChat
      ? (agents.find((agent) => agent.id === "builder")?.id ?? agents[0]?.id ?? null)
      : (agents[0]?.id ?? null),
    detailModel: detail?.model ?? null,
    onError: setSendError,
    onModelApplied: reloadDetail,
  });
  const chooseAgent = useAgentChoice({
    agents,
    workflows,
    setWorkflowSelection,
    handleModeChange,
  });
  const { scrollRef, contentRef, awayFromBottom, setFollow, onScroll, handleLoadOlder } =
    useScrollFollow({ sessionId, view, detail, loadOlder });
  const jumpToNode = useJumpToNode({ view, hasMore, loadOlder, setFollow });
  const actions = useSessionActions({
    sessionId,
    pendingApproval: served.pendingApproval,
    pendingWaiver: served.pendingWaiver,
    onError: setSendError,
  });
  const workflowActions = useWorkflowActions({ sessionId, run });
  const { mcpView, draftMcpDisabled, toggleMcp } = useSessionMcp(sessionId, detailVersion);

  const { text, setText, attachments, setAttachments } = useComposerDraft(
    sessionId ?? `new:${projectId ?? ""}`,
  );
  useComposerPrefill({
    sessionId,
    projectId,
    frameIdeaId,
    fromSessionId,
    chooseAgent,
    setText,
    setAttachments,
  });
  const { handleSend, handleAttach, handleRemoveAttachment } = useSessionSend({
    sessionId,
    projectId,
    frameIdeaId,
    pseudoTabId,
    isChat,
    running,
    run,
    hasPendingQuestion: Boolean(view.pendingQuestion),
    mode,
    pendingModel,
    pendingEffort,
    draftMcpDisabled,
    workflows,
    workflowSelection,
    setWorkflowSelection,
    text,
    setText,
    attachments,
    setAttachments,
    setSendError,
    onSessionCreated,
  });
  const { pendingPlan, planActions, handleTakeBackPlan } = useSessionPlan({
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
  });
  const decisions = useSessionDecisions({
    sessionId,
    detail,
    view,
    served,
    running,
    waiting,
    pendingPlan,
    planActions,
    openIntegration,
    tryIt: revealServers,
  });

  useEffect(() => {
    if (!sessionId) {
      window.setTimeout(() => textareaRef.current?.focus(), 0);
    }
  }, [sessionId]);

  useEffect(() => {
    if (view.pendingQuestion && !running) {
      textareaRef.current?.focus();
    }
  }, [view.pendingQuestion, running]);

  // A preview belongs to one session; do not leak it across tabs.
  useEffect(() => {
    setPreview(null);
    setCanvasId(null);
    setRunComment("");
  }, [sessionId]);

  const title =
    view.name ?? detail?.title ?? (sessionId ? t('session') : isChat ? t('paletteNewChat') : t('newSession'));
  const projectName =
    detail?.project.name ??
    projects.find((project) => project.id === projectId)?.name ??
    projectId ??
    "—";
  const pendingCallId =
    served.pendingApproval?.callId ?? served.pendingWaiver?.callId;

  return (
    <SessionProviders
      sessionId={sessionId}
      linkProjectId={detail?.project.id ?? projectId}
      detail={detail}
      isChat={isChat}
      onPreview={setPreview}
      onOpenCanvas={setCanvasId}
      pendingPlanCallId={served.pendingPlan?.callId ?? null}
      integrationInvite={decisions.proposals.integration}
      calls={calls}
      workflowUsage={workflowUsage}
    >
      <div className="flex min-h-0 min-w-0 grow">
        {run && <RunRail run={run} onJump={jumpToNode} />}
        <main className="flex min-h-0 min-w-0 flex-[999_1_560px] flex-col">
          <SessionHeader
            title={title}
            status={status}
            sessionId={sessionId}
            isChat={isChat}
            projectName={projectName}
            detail={detail}
            projects={projects}
            context={context}
            compactDisabled={!sessionId || state === "running" || state === "waiting_for_user"}
            onCompact={actions.handleCompact}
            compactError={actions.compactError}
            onOpenContext={() => revealPanel('session')}
            onRenamed={onRenamed}
            onClose={onClose}
          />
          <SessionThreadPane
            scrollRef={scrollRef}
            contentRef={contentRef}
            onScroll={onScroll}
            sessionId={sessionId}
            view={view}
            hasMore={hasMore}
            loadingOlder={loadingOlder}
            onLoadOlder={handleLoadOlder}
            pendingCallId={pendingCallId}
            model={currentModel ? modelShortName(currentModel.model) : t('sessionPageModelFallback')}
            running={running}
            replayable={served.replayable}
            onReplay={actions.handleReplay}
          />
          {/* A node's session is driven by its workflow: the human answers in the parent, or here when the step asked. */}
          {detail?.parentId && <WorkflowNodeNotice parentId={detail.parentId} running={running} asks={detail.workflowAsks} />}
          <RunningStrip
            running={running}
            run={run}
            following={!awayFromBottom}
            onFollowChange={setFollow}
          />
          {!detail?.parentId && (
            <QueuedMessages queued={served.queued} onCancel={actions.handleCancelQueued} />
          )}
          {decisions.decisionCount > 0 && (
            <SessionDecisionBar
              decisions={decisions}
              sessionId={sessionId}
              detail={detail}
              running={running}
              pendingPlan={pendingPlan}
              planActions={planActions}
              actions={actions}
              workflowActions={workflowActions}
              runComment={runComment}
              onRunCommentChange={setRunComment}
              onAnswer={(answer) => void handleSend(answer)}
              onDetailChanged={reloadDetail}
            />
          )}
          {!detail?.parentId && detail && served.handedPlan && (
            <HandedPlanNote
              projectId={detail.project.id}
              onUndo={served.handedPlan.undoable ? () => void handleTakeBackPlan(served.handedPlan!.ideaId) : undefined}
            />
          )}
          {(!detail?.parentId || detail.workflowAsks) && (
            <Composer
              text={text}
              onTextChange={setText}
              onSend={() => handleSend()}
              onStop={actions.handleStop}
              onPause={run && !run.finished && !run.paused ? workflowActions.handlePauseWorkflow : undefined}
              pauseRequested={run?.pauseRequested}
              running={running}
              currentModel={currentModel}
              modelChoices={modelChoices}
              onModelChange={handleModelChange}
              effort={effort}
              acceptedEfforts={acceptedEfforts}
              defaultEffort={defaultEffort}
              onEffortChange={(next) => void handleEffortChange(next)}
              error={sendError}
              textareaRef={textareaRef}
              placeholder={
                workflowSelection.workflowId && !isChat
                  ? t('sessionPageWorkflowTaskPlaceholder')
                  : view.pendingQuestion && !running
                    ? t('sessionPageAnswerPlaceholder')
                    : undefined
              }
              commands={commands}
              // A chat talks to builder only: no agent nor workflow to pick.
              agents={agents}
              mode={mode}
              onModeChange={chooseAgent}
              attachments={attachments}
              onAttach={handleAttach}
              onRemoveAttachment={handleRemoveAttachment}
              workflows={isChat ? undefined : workflows}
              workflowSelection={workflowSelection}
              onWorkflowSelectionChange={setWorkflowSelection}
              mcpView={mcpView}
              onMcpToggle={toggleMcp}
            />
          )}
        </main>

        <ContextPanel
          key={sessionId ?? ''}
          view={view}
          skills={skills}
          usage={usage}
          context={context}
          reveal={reveal}
          onTry={revealServers}
          hasSession={sessionId !== null}
          worktree={detail?.worktree ?? null}
          editorUrl={detail?.editorUrl ?? null}
          sessionId={sessionId}
          onWorktreeChanged={reloadDetail}
          integrationVersion={integrationVersion}
          vision={vision}
          served={served}
          onOpenCanvas={setCanvasId}
          onNewCanvas={(id) => {
            revealPanel('canvas');
            setCanvasId(id);
          }}
        />
        <SessionOverlays
          sessionId={sessionId}
          projectId={detail?.project.id ?? projectId}
          view={view}
          integrationOpen={integrationOpen}
          onCloseIntegration={() => {
            setIntegrationOpen(false);
            setIntegrationVersion((version) => version + 1);
          }}
          preview={preview}
          onClosePreview={() => setPreview(null)}
          canvasId={canvasId}
          onCloseCanvas={() => setCanvasId(null)}
          onAnnotate={(message) => {
            // The run waiting on a choice or a pause refuses the composer: the note goes to its comment.
            const append = (current: string) =>
              current.trim() ? `${current}\n\n${message}` : message;
            if (waiting?.kind === "human" || waiting?.kind === "pause") setRunComment(append);
            else setText(append);
          }}
        />
      </div>
    </SessionProviders>
  );
}
