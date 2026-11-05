import { useEffect, useMemo, useRef, useState } from "react";
import { addPlannedIdea, frameIdea, framingPrefill, listIdeas, markIdeaReady } from "../lib/ideas";
import type {
  AttachmentInput,
  McpSessionView,
  Project,
  SessionDetail,
  SessionState,
} from "../lib/api";
import type { AssistantItem, Segment } from "../lib/fold";
import { splitContract } from "../lib/plan";
import {
  createSession,
  getSession,
  getSessionMcp,
  sendMessage,
  setSessionMcp,
  startWorkflow,
} from "../lib/api";
import { readImages } from "../lib/attachments";
import { formatDateTime } from "../lib/format";
import { sessionStatus } from "../lib/sessionStatus";
import { runWaiting } from "../lib/workflowRun";
import { useLang, useT } from "../i18n";
import { useSessionThread } from "../hooks/useSessionThread";
import { useProjectOptions } from "../hooks/useProjectOptions";
import { useModeAndModel } from "../hooks/useModeAndModel";
import { useScrollFollow } from "../hooks/useScrollFollow";
import { useSessionActions } from "../hooks/useSessionActions";
import { useWorkflowActions } from "../hooks/useWorkflowActions";
import { Composer } from "../components/session/Composer";
import { RunningStrip } from "../components/session/RunningStrip";
import { modelShortName } from "../components/session/ModelPicker";
import { Badge } from "../components/ui/Badge";
import { AuditCard } from "../components/session/AuditCard";
import { ProposedTasksCard } from "../components/session/ProposedTasksCard";
import { AgentsMdCard } from "../components/session/AgentsMdCard";
import { RuleProposalCard } from "../components/session/RuleProposalCard";
import { AgentSuggestionCard } from "../components/session/AgentSuggestionCard";
import { ContextPanel } from "../components/session/ContextPanel";
import { ContextGauge } from "../components/session/ContextGauge";
import { QuestionCard } from "../components/session/QuestionCard";
import { RuleApprovalCard } from "../components/session/RuleApprovalCard";
import { WaiverCard } from "../components/session/WaiverCard";
import { QueuedMessages } from "../components/session/QueuedMessages";
import { RetryCard } from "../components/session/RetryCard";
import { ThreadItemView } from "../components/session/Thread";
import { PlanActionsContext, type PlanActions } from "../components/session/PlanActions";
import {
  CallUsageContext,
  WorkflowUsageContext,
} from "../components/session/CallUsage";
import {
  IntegrationInviteContext,
  type IntegrationInvite,
} from "../components/session/IntegrationInvite";
import { IntegrationDialog } from "../components/session/IntegrationDialog";
import { TryDialog } from "../components/session/TryDialog";
import { PlayIcon } from "../components/Icons";
import {
  PreviewContext,
  PreviewWorkdirContext,
  type PreviewTarget,
} from "../components/preview/PreviewContext";
import { PreviewPane } from "../components/preview/PreviewPane";
import { CanvasPane } from "../components/canvas/CanvasPane";
import { SessionCanvasMenu } from "../components/canvas/SessionCanvasMenu";
import { CanvasOpenerContext, CanvasSessionContext } from "../components/canvas/CanvasContext";
import { RunRail } from "../components/workflow/session/RunRail";
import { RunActionCard } from "../components/workflow/session/RunActionCard";
import { Button, buttonClass } from "../components/ui/Button";
import {
  DEFAULT_WORKFLOW_ID,
  defaultWorkflowSelection,
} from "../components/workflow/session/WorkflowPicker";

// One unsent draft per session, kept while the app lives: switching tabs must
// never carry a draft to another agent, and coming back must find it again.
const drafts = new Map<string, string>();

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
  /** The generated session name changed; the tabs read titles from projects. */
  onRenamed: () => void;
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
}: SessionPageProps) {
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [text, setText] = useState("");
  const draftKey = sessionId ?? `new:${projectId ?? ""}`;
  const textRef = useRef(text);
  textRef.current = text;
  useEffect(() => {
    setText(drafts.get(draftKey) ?? "");
    // Saved when the key changes: the cleanup still sees the old key.
    return () => {
      drafts.set(draftKey, textRef.current);
    };
  }, [draftKey]);
  const [attachments, setAttachments] = useState<AttachmentInput[]>([]);
  const [sendError, setSendError] = useState<string | null>(null);
  // A session framing a backlog idea only talks: its messages must not start
  // the default build workflow (a "go on" there once ran a whole build).
  const framingIdea = detail?.idea?.state === "framing";
  const [planBuildInFlight, setPlanBuildInFlight] = useState(false);
  const [preview, setPreview] = useState<PreviewTarget | null>(null);
  const [canvasId, setCanvasId] = useState<number | null>(null);
  const [runComment, setRunComment] = useState("");
  const [detailVersion, setDetailVersion] = useState(0);
  // The plan last put in the backlog: its button says so until a new plan comes.
  const [backlogSegment, setBacklogSegment] = useState<string | null>(null);
  const [integrationOpen, setIntegrationOpen] = useState(false);
  const [tryOpen, setTryOpen] = useState(false);
  const [integrationVersion, setIntegrationVersion] = useState(0);
  const [mcpView, setMcpView] = useState<McpSessionView | null>(null);
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
  } = useSessionThread(sessionId);
  const lang = useLang();
  const t = useT();
  const running = view.running;
  const pendingCallId =
    served.pendingApproval?.callId ?? served.pendingWaiver?.callId;
  const run = view.run;
  const waiting = runWaiting(run, running, view.lastDone);
  // The detail is fetched once; the event stream says how a turn ended.
  const state: SessionState = running
    ? "running"
    : ((view.lastDone as SessionState | undefined) ?? detail?.state ?? "idle");
  const status = sessionStatus(lang, state, run, waiting);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const previousRunning = useRef(running);
  const {
    commands,
    agents,
    workflows,
    workflowSelection,
    setWorkflowSelection,
  } = useProjectOptions({
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
    handleModeChange,
    handleModelChange,
  } = useModeAndModel({
    sessionId,
    viewMode: view.mode,
    detailMode: detail?.mode ?? null,
    fallbackMode: isChat
      ? (agents.find((agent) => agent.id === "builder")?.id ?? agents[0]?.id ?? null)
      : (agents[0]?.id ?? null),
    detailModel: detail?.model ?? null,
    onError: setSendError,
    onModelApplied: () => setDetailVersion((version) => version + 1),
  });
  const {
    scrollRef,
    contentRef,
    awayFromBottom,
    setFollow,
    onScroll,
    handleLoadOlder,
  } = useScrollFollow({ sessionId, view, detail, loadOlder });
  const {
    compactError,
    handleStop,
    handleCompact,
    handleReplay,
    handleApproval,
    handleCancelQueued,
    handleWaiver,
    handleRetry,
  } = useSessionActions({
    sessionId,
    pendingApproval: served.pendingApproval,
    pendingWaiver: served.pendingWaiver,
    onError: setSendError,
  });
  const {
    handleStopWorkflow,
    handlePauseWorkflow,
    handleAnswerWorkflow,
    handleContinueWorkflow,
    handleResumeWorkflow,
  } = useWorkflowActions({ sessionId, run });

  useEffect(() => {
    onRunningChanged();
  }, [running, onRunningChanged]);

  const previousName = useRef<string | undefined>(undefined);

  useEffect(() => {
    if (view.name && view.name !== previousName.current) {
      onRenamed();
    }
    previousName.current = view.name;
  }, [view.name, onRenamed]);

  useEffect(() => {
    if (!sessionId) {
      setDetail(null);
      return;
    }
    let active = true;
    getSession(sessionId)
      .then((data) => {
        if (active) setDetail(data);
      })
      .catch(() => {
        // The event stream is the source of truth for the thread.
      });
    return () => {
      active = false;
    };
  }, [sessionId, detailVersion]);

  // The composer's MCP dropdown: the session's effective MCP state, refetched
  // when the session changes or a turn ends (a global flip on the Settings
  // page shows up on the next turn), the toggle handler updates it directly.
  useEffect(() => {
    if (!sessionId) {
      setMcpView(null);
      return;
    }
    let active = true;
    getSessionMcp(sessionId)
      .then((data) => {
        if (active) setMcpView(data);
      })
      .catch(() => {
        if (active) setMcpView(null);
      });
    return () => {
      active = false;
    };
  }, [sessionId, detailVersion]);

  function handleMcpToggle(name: string, active: boolean) {
    if (!sessionId || !mcpView) return;
    // The disabled set is the full set (last wins), so it must include hidden
    // servers too: a hidden server left out would silently turn on.
    const disabled = mcpView.servers
      .filter((server) => (server.name === name ? !active : !server.active))
      .map((server) => server.name);
    setSessionMcp(sessionId, disabled)
      .then(setMcpView)
      .catch(() => {
        // Keep the optimistic view; the next fetch corrects it.
      });
  }

  useEffect(() => {
    // A workflow's first turn creates the worktree: re-read the detail when
    // a turn ends, not only on a reload.
    if (previousRunning.current && !running) {
      setDetailVersion((version) => version + 1);
    }
    previousRunning.current = running;
  }, [running]);

  useEffect(() => {
    if (!sessionId) {
      window.setTimeout(() => textareaRef.current?.focus(), 0);
    }
  }, [sessionId]);

  useEffect(() => {
    if (!sessionId && projectId && frameIdeaId) {
      const ideaId = frameIdeaId;
      let ignore = false;
      listIdeas(projectId)
        .then((board) => {
          if (ignore) return;
          const idea = board.groups
            .flatMap((group) => group.ideas)
            .find((candidate) => candidate.id === ideaId);
          if (idea) {
            const prefill = framingPrefill(idea);
            setText(prefill.text);
            setAttachments(prefill.attachments);
          }
        })
        .catch(() => {
          // Ignore: the composer stays editable with an empty message.
        });
      return () => {
        ignore = true;
      };
    }
  }, [projectId, frameIdeaId, sessionId]);

  // Read through a ref: the effect below runs once per source session, not
  // on every render that recreates the handler.
  const modeChangeRef = useRef(handleModeChange);
  modeChangeRef.current = handleModeChange;
  useEffect(() => {
    if (!sessionId && fromSessionId) {
      let ignore = false;
      getSession(fromSessionId)
        .then((source) => {
          if (ignore || !source.agentSuggestion) return;
          setText(source.agentSuggestion.brief);
          void modeChangeRef.current(source.agentSuggestion.agent);
        })
        .catch(() => {
          // Ignore: the composer stays editable with an empty message.
        });
      return () => {
        ignore = true;
      };
    }
  }, [fromSessionId, sessionId]);

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

  const lastAssistant = [...view.items]
    .reverse()
    .find((item): item is AssistantItem => item.kind === "assistant");
  const lastTextSegment = lastAssistant
    ? [...lastAssistant.segments]
        .reverse()
        .find(
          (segment): segment is Extract<Segment, { kind: "text" }> =>
            segment.kind === "text",
        )
    : undefined;
  const planContract = lastTextSegment
    ? splitContract(lastTextSegment.content)
    : null;
  const planActions: PlanActions | null =
    planContract?.contract.kind === "plan" &&
    lastTextSegment &&
    !running &&
    !view.pendingQuestion &&
    (!run || run.finished) &&
    mode === "plan"
      ? {
          segmentId: lastTextSegment.id,
          onBuild: handlePlanBuild,
          onBuildAlone: handlePlanBuildAlone,
          onRevise: handlePlanRevise,
          ideaReady: detail
            ? {
                onMark: () => void handleIdeaReady(),
                idea:
                  backlogSegment === lastTextSegment.id
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
  const lastRunEndId = useMemo(() => {
    for (let i = view.items.length - 1; i >= 0; i -= 1) {
      const item = view.items[i];
      // Once integrated, the run's work is in the human's branch: no invite left.
      if (item?.id.startsWith("integration-applied-")) return null;
      if (item && item.kind === "workflow-end") return item.id;
    }
    return null;
  }, [view.items]);
  const integrationInvite = useMemo<IntegrationInvite | null>(() => {
    // No commit on the agent's branch: nothing to review, no invitation.
    if (!sessionId || !detail?.worktree?.commits) return null;
    return { lastRunEndId, open: () => setIntegrationOpen(true) };
  }, [sessionId, detail?.worktree?.commits, lastRunEndId]);
  const projectName =
    detail?.project.name ??
    projects.find((project) => project.id === projectId)?.name ??
    projectId ??
    "—";

  // A send in flight: a second Enter or click before the server answers would
  // create a second session (or send the message twice).
  const sending = useRef(false);

  async function handleSend(content?: string) {
    if (sending.current) return;
    sending.current = true;
    try {
      await send(content);
    } finally {
      sending.current = false;
    }
  }

  async function send(content?: string) {
    // Attachments belong to the composer; a quick answer (`content`) sends none.
    const fromComposer = content === undefined;
    const value = (content ?? text).trim();
    const images = fromComposer ? attachments : [];
    if (!value && images.length === 0) return;
    setSendError(null);

    // Ask for notification permission in the user gesture that starts a turn.
    if ('Notification' in window && Notification.permission === 'default') {
      void Notification.requestPermission();
    }

    // A quick answer or a message to a running/waiting run is an ordinary
    // message, not a workflow start.
    const workflowId =
      fromComposer &&
      !isChat &&
      workflowSelection.workflowId &&
      !running &&
      (!run || run.finished) &&
      !view.pendingQuestion
        ? workflowSelection.workflowId
        : null;

    try {
      if (workflowId) {
        if (sessionId) {
          await startWorkflow(
            sessionId,
            workflowId,
            value,
            workflowSelection.stepByStep,
            workflowSelection.worktree,
            images,
          );
        } else if (projectId) {
          const created = await createSession(
            projectId,
            value,
            mode ?? undefined,
            images,
            {
              workflowId,
              stepByStep: workflowSelection.stepByStep,
              worktree: workflowSelection.worktree,
            },
            pendingModel ?? undefined,
          );
          onSessionCreated(created.id, pseudoTabId);
        }
        // The next message gets the default workflow again.
        setWorkflowSelection(defaultWorkflowSelection(workflows));
      } else if (sessionId) {
        await sendMessage(sessionId, value, images);
      } else if (projectId) {
        // A mode chosen before creation is applied to the first turn.
        const created = await createSession(
          projectId,
          value,
          frameIdeaId ? "plan" : (mode ?? undefined),
          images,
          undefined,
          pendingModel ?? undefined,
        );
        if (frameIdeaId) {
          await frameIdea(projectId, frameIdeaId, created.id);
        }
        onSessionCreated(created.id, pseudoTabId);
      }
      if (fromComposer) {
        setText("");
        setAttachments([]);
      }
    } catch (error) {
      setSendError(
        error instanceof Error ? error.message : t('sessionPageSendError'),
      );
    }
  }

  async function handleAttach(files: File[]) {
    const { attachments: read, error } = await readImages(
      lang,
      files,
      attachments.length,
    );
    setAttachments((current) => [...current, ...read]);
    setSendError(error);
  }

  function handleRemoveAttachment(index: number) {
    setAttachments((current) => current.filter((_, i) => i !== index));
  }

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
        lastTextSegment
      ) {
        const task =
          [...view.items]
            .reverse()
            .find(
              (item): item is Extract<typeof item, { kind: "user" }> =>
                item.kind === "user" && !item.isSystemGenerated,
            )?.content ?? t('sessionPageExecuteApprovedPlan');
        await startWorkflow(
          sessionId,
          DEFAULT_WORKFLOW_ID,
          task,
          false,
          worktree,
          [],
          lastTextSegment.content,
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

  async function handleIdeaReady() {
    if (!detail || !lastTextSegment) return;
    setSendError(null);
    try {
      if (detail.idea) {
        await markIdeaReady(detail.project.id, detail.idea.id, lastTextSegment.content);
      } else {
        await addPlannedIdea(detail.project.id, detail.title, detail.id, lastTextSegment.content);
      }
      setBacklogSegment(lastTextSegment.id);
      setDetailVersion((version) => version + 1);
    } catch (err) {
      setSendError(err instanceof Error ? err.message : t('unknownError'));
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

  function handlePlanRevise() {
    // The revision is an ordinary message to the planner, not a workflow start.
    setWorkflowSelection((selection) => ({ ...selection, workflowId: null }));
    textareaRef.current?.focus();
  }

  return (
    <PreviewContext.Provider value={setPreview}>
     <CanvasOpenerContext.Provider value={setCanvasId}>
      <CanvasSessionContext.Provider value={sessionId ?? undefined}>
      {/* The agent writes in its worktree when it has one: paths are relative to it, and the preview route resolves them there too. */}
      <PreviewWorkdirContext.Provider value={detail?.worktree?.path ?? detail?.project.workdir}>
        <PlanActionsContext.Provider value={planActions}>
          <IntegrationInviteContext.Provider value={integrationInvite}>
            <CallUsageContext.Provider value={calls}>
              <WorkflowUsageContext.Provider value={workflowUsage}>
                <div className="flex min-h-0 min-w-0 grow">
                  {run && <RunRail run={run} />}
                  <main className="flex min-h-0 min-w-0 flex-[999_1_560px] flex-col">
                    {/* A shadow under the title: the thread fades under it instead of meeting a hard edge. */}
                    <div className="relative z-10 flex items-center gap-2 px-4 md:px-6 py-2.5 after:pointer-events-none after:absolute after:inset-x-0 after:top-full after:h-5 after:bg-linear-to-b after:from-black/45 after:to-transparent">
                      <div className="mx-auto flex w-full max-w-[960px] items-center gap-2">
                        <h1
                          className="min-w-0 truncate text-base font-semibold"
                          title={title}
                        >
                          {title}
                        </h1>
                        <Badge
                          tone={status.tone}
                          dot={status.live}
                          pulse={status.live}
                        >
                          {status.label}
                        </Badge>
                        <span
                          className="shrink-0 text-xs text-muted"
                          title={isChat ? undefined : detail?.project.workdir}
                        >
                          {isChat ? t('sessionPageChat') : projectName}
                        </span>
                        {detail && (
                          <span
                            className="shrink-0 text-xs text-muted tabular-nums"
                            title={t('sessionPageCreatedTitle')}
                          >
                            {formatDateTime(lang, detail.createdAt)}
                          </span>
                        )}
                        {sessionId && !isChat && (
                          <Button
                            variant="ghost"
                            size="sm"
                            iconOnly
                            title={t('tryChange')}
                            aria-label={t('tryChange')}
                            onClick={() => setTryOpen(true)}
                          >
                            <PlayIcon className="h-3.5 w-3.5" />
                          </Button>
                        )}
                        {sessionId && (
                          <SessionCanvasMenu sessionId={sessionId} refreshKey={view} onOpen={setCanvasId} />
                        )}
                        <div className="ml-auto">
                          <ContextGauge
                            context={context}
                            compactDisabled={
                              !sessionId ||
                              state === "running" ||
                              state === "waiting_for_user"
                            }
                            onCompact={handleCompact}
                            compactError={compactError}
                          />
                        </div>
                      </div>
                    </div>

                    <div className="relative min-h-0 grow">
                      <div
                        ref={scrollRef}
                        onScroll={onScroll}
                        className="h-full overflow-y-auto p-4 md:p-6"
                      >
                        <div
                          ref={contentRef}
                          className="mx-auto flex max-w-[960px] flex-col gap-4.5"
                        >
                          {sessionId && hasMore && (
                            <Button
                              className="mx-auto"
                              onClick={handleLoadOlder}
                              disabled={loadingOlder}
                            >
                              {loadingOlder
                                ? t('loading')
                                : t('sessionPageLoadOlderMessages')}
                            </Button>
                          )}
                          {view.items.length === 0 && !sessionId && (
                            <div className="py-10 text-center text-sm text-muted">
                              {t('sessionPageNewSessionHint')}
                            </div>
                          )}
                          {view.items.map((item, index) => (
                            <ThreadItemView
                              key={item.id}
                              item={item}
                              pendingCallId={pendingCallId}
                              model={
                                currentModel
                                  ? modelShortName(currentModel.model)
                                  : t('sessionPageModelFallback')
                              }
                              running={running}
                              showHeader={
                                view.items[index - 1]?.kind !== "assistant"
                              }
                              onReplay={sessionId && served.replayable.includes(item.id) ? handleReplay : undefined}
                            />
                          ))}
                          {detail && detail.audit && detail.audit.findings.length > 0 && !running ? (
                            <AuditCard
                              audit={detail.audit}
                              sessionId={sessionId}
                              projectId={detail.project.id ?? null}
                              onCreated={() => setDetailVersion((version) => version + 1)}
                            />
                          ) : null}
                          {detail && detail.proposedTasks.length > 0 && !running ? (
                            <ProposedTasksCard
                              tasks={detail.proposedTasks}
                              sessionId={sessionId}
                              projectId={detail.project.id ?? null}
                              onAccepted={() => setDetailVersion((version) => version + 1)}
                            />
                          ) : null}
                          {detail && detail.agentsMdDraft && !running ? (
                            <AgentsMdCard
                              draft={detail.agentsMdDraft}
                              sessionId={sessionId}
                              onWritten={() => setDetailVersion((version) => version + 1)}
                            />
                          ) : null}
                          {detail && detail.ruleProposal && !running ? (
                            <RuleProposalCard
                              proposal={detail.ruleProposal}
                              sessionId={sessionId}
                              onAdded={() => setDetailVersion((version) => version + 1)}
                            />
                          ) : null}
                          {detail && detail.agentSuggestion && sessionId && detail.project.id && !running ? (
                            <AgentSuggestionCard
                              suggestion={detail.agentSuggestion}
                              sessionId={sessionId}
                              projectId={detail.project.id}
                            />
                          ) : null}
                        </div>
                      </div>
                    </div>

                    {served.pendingApproval &&
                      !running &&
                      !waiting &&
                      !detail?.parentId && (
                        <RuleApprovalCard
                          approval={served.pendingApproval}
                          onAnswer={handleApproval}
                          disabled={running}
                        />
                      )}
                    {served.pendingWaiver &&
                      !running &&
                      !waiting &&
                      !detail?.parentId && (
                        <WaiverCard
                          waiver={served.pendingWaiver}
                          onAnswer={handleWaiver}
                          disabled={running}
                        />
                      )}
                    {view.pendingQuestion &&
                      !running &&
                      !waiting &&
                      !detail?.parentId && (
                        <QuestionCard
                          question={view.pendingQuestion}
                          onAnswer={(answer) => handleSend(answer)}
                          disabled={running}
                        />
                      )}
                    {waiting && !detail?.parentId && (
                      <RunActionCard
                        waiting={waiting}
                        onAnswer={handleAnswerWorkflow}
                        onContinue={handleContinueWorkflow}
                        onResume={handleResumeWorkflow}
                        onStop={handleStopWorkflow}
                        disabled={running}
                        comment={runComment}
                        onCommentChange={setRunComment}
                      />
                    )}
                    {view.lastDone === "error" &&
                      !running &&
                      !view.pendingQuestion &&
                      !waiting &&
                      !detail?.parentId && (
                        <RetryCard onRetry={handleRetry} disabled={running} />
                      )}
                    {/* A node's session is driven by its workflow: the human answers in the parent. */}
                    {detail?.parentId && (
                      <div className="mx-auto flex max-w-[960px] items-center gap-3 border-t border-border px-1 py-2">
                        <span
                          className={`h-2 w-2 shrink-0 rounded-full ${running ? "bg-accent" : "bg-ok"}`}
                        />
                        <div className="min-w-0 grow">
                          <div className="text-sm font-medium text-text">
                            {running
                              ? t('sessionPageWorkflowNodeRunning')
                              : t('sessionPageWorkflowNodeFinished')}
                          </div>
                          <div className="text-xs text-muted">
                            {t('sessionPageWorkflowDrivenSessionHint')}
                          </div>
                        </div>
                        <a
                          href={`#/s/${detail.parentId}`}
                          className={buttonClass(
                            { variant: "primary", size: "sm" },
                            "shrink-0",
                          )}
                        >
                          {t('sessionPageBackToWorkflow')}
                        </a>
                      </div>
                    )}
                    <RunningStrip
                      running={running}
                      run={run}
                      following={!awayFromBottom}
                      onFollowChange={setFollow}
                    />
                    {!detail?.parentId && (
                      <QueuedMessages queued={served.queued} onCancel={handleCancelQueued} />
                    )}
                    {!detail?.parentId && (
                      <Composer
                        text={text}
                        onTextChange={setText}
                        onSend={() => handleSend()}
                        onStop={handleStop}
                        onPause={run && !run.finished && !run.paused ? handlePauseWorkflow : undefined}
                        pauseRequested={run?.pauseRequested}
                        running={running}
                        currentModel={currentModel}
                        modelChoices={modelChoices}
                        onModelChange={handleModelChange}
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
                        agents={isChat ? undefined : agents}
                        mode={mode}
                        onModeChange={handleModeChange}
                        attachments={attachments}
                        onAttach={handleAttach}
                        onRemoveAttachment={handleRemoveAttachment}
                        workflows={isChat ? undefined : workflows}
                        workflowSelection={workflowSelection}
                        onWorkflowSelectionChange={setWorkflowSelection}
                        mcpView={mcpView}
                        onMcpToggle={handleMcpToggle}
                      />
                    )}
                  </main>

                  <ContextPanel
                    view={view}
                    skills={skills}
                    usage={usage}
                    request={detail?.request ?? null}
                    hasSession={sessionId !== null}
                    worktree={detail?.worktree ?? null}
                    sessionId={sessionId}
                    onWorktreeChanged={() =>
                      setDetailVersion((version) => version + 1)
                    }
                    integrationVersion={integrationVersion}
                    vision={vision}
                    served={served}
                  />
                  {sessionId && integrationOpen && (
                    <IntegrationDialog
                      sessionId={sessionId}
                      projectId={detail?.project.id ?? projectId}
                      open
                      onClose={() => {
                        setIntegrationOpen(false);
                        setIntegrationVersion((version) => version + 1);
                      }}
                    />
                  )}
                  {sessionId && tryOpen && (
                    <TryDialog sessionId={sessionId} open onClose={() => setTryOpen(false)} />
                  )}
                  {preview && sessionId && (
                    <PreviewPane
                      sessionId={sessionId}
                      target={preview}
                      onClose={() => setPreview(null)}
                    />
                  )}
                  {canvasId != null && (
                    <CanvasPane
                      artifactId={canvasId}
                      refreshKey={view}
                      onClose={() => setCanvasId(null)}
                      onAnnotate={(message) => {
                        // The run waiting on a choice or a pause refuses the composer: the note goes to its comment.
                        const append = (current: string) =>
                          current.trim() ? `${current}\n\n${message}` : message;
                        if (waiting?.kind === "human" || waiting?.kind === "pause") setRunComment(append);
                        else setText(append);
                      }}
                    />
                  )}
                </div>
              </WorkflowUsageContext.Provider>
            </CallUsageContext.Provider>
          </IntegrationInviteContext.Provider>
        </PlanActionsContext.Provider>
      </PreviewWorkdirContext.Provider>
      </CanvasSessionContext.Provider>
     </CanvasOpenerContext.Provider>
    </PreviewContext.Provider>
  );
}
