import type { WorkflowSummary } from "../lib/workflow";
import { useEffect, useMemo, useRef, useState } from "react";
import { addPlannedIdea, frameIdea, listIdeas, markIdeaReady } from "../lib/ideas";
import type {
  Agent,
  AttachmentInput,
  ModelChoice,
  ModelChoices,
  ModelRef,
  Project,
  SessionDetail,
  SessionState,
  SlashCommand,
} from "../lib/api";
import type { AssistantItem, Segment } from "../lib/fold";
import { splitContract } from "../lib/plan";
import {
  answerWorkflow,
  compactSession,
  continueWorkflow,
  createSession,
  getSession,
  getModelChoices,
  listAgents,
  listCommands,
  listWorkflows,
  pauseWorkflow,
  replayMessage,
  resumeWorkflow,
  sendMessage,
  setSessionMode,
  setSessionModel,
  startWorkflow,
  stopSession,
  stopWorkflow,
} from "../lib/api";
import { readImages } from "../lib/attachments";
import { sessionStatus } from "../lib/sessionStatus";
import { runWaiting } from "../lib/workflowRun";
import { useSessionThread } from "../hooks/useSessionThread";
import { Composer } from "../components/session/Composer";
import { RunningStrip } from "../components/session/RunningStrip";
import { modelShortName } from "../components/session/ModelPicker";
import { Badge } from "../components/ui/Badge";
import { ContextPanel } from "../components/session/ContextPanel";
import { ContextGauge } from "../components/session/ContextGauge";
import { QuestionCard } from "../components/session/QuestionCard";
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
import {
  PreviewContext,
  PreviewWorkdirContext,
  type PreviewTarget,
} from "../components/preview/PreviewContext";
import { PreviewPane } from "../components/preview/PreviewPane";
import { RunRail } from "../components/workflow/session/RunRail";
import { RunActionCard } from "../components/workflow/session/RunActionCard";
import { Button, buttonClass } from "../components/ui/Button";
import { ChevronDownIcon } from "../components/Icons";
import {
  DEFAULT_WORKFLOW_ID,
  defaultWorkflowSelection,
  type WorkflowSelection,
} from "../components/workflow/session/WorkflowPicker";

interface SessionPageProps {
  sessionId: string | null;
  projectId: string | null;
  /** A workflow opened from the editor's test button: preselected, step by step. */
  testWorkflowId: string | null;
  /** A backlog idea framed from the project page: its text prefills the composer, the first send links the session to it. */
  frameIdeaId: string | null;
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
  pseudoTabId,
  projects,
  onSessionCreated,
  onRunningChanged,
  onRenamed,
}: SessionPageProps) {
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [text, setText] = useState("");
  const [attachments, setAttachments] = useState<AttachmentInput[]>([]);
  const [sendError, setSendError] = useState<string | null>(null);
  const [compactError, setCompactError] = useState<string | null>(null);
  const [commands, setCommands] = useState<SlashCommand[]>([]);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [workflows, setWorkflows] = useState<WorkflowSummary[]>([]);
  const [workflowSelection, setWorkflowSelection] = useState<WorkflowSelection>(
    { workflowId: null, stepByStep: false, worktree: true },
  );
  // A session framing a backlog idea only talks: its messages must not start
  // the default build workflow (a "go on" there once ran a whole build).
  // Depends on `workflows` too, so a late workflow list does not reselect it.
  const framingIdea = detail?.idea?.state === "framing";
  useEffect(() => {
    if (framingIdea) {
      setWorkflowSelection((selection) => ({ ...selection, workflowId: null }));
    }
  }, [framingIdea, workflows]);
  // Optimistic mode while `setSessionMode` is in flight; cleared once the
  // server's mode (event stream or detail) matches it.
  const [pendingMode, setPendingMode] = useState<string | null>(null);
  const [modelChoices, setModelChoices] = useState<ModelChoices | null>(null);
  const [pendingModel, setPendingModel] = useState<ModelRef | null>(null);
  const modelRequestRef = useRef(0);
  const [planBuildInFlight, setPlanBuildInFlight] = useState(false);
  const [preview, setPreview] = useState<PreviewTarget | null>(null);
  const [detailVersion, setDetailVersion] = useState(0);
  // The plan last put in the backlog: its button says so until a new plan comes.
  const [backlogSegment, setBacklogSegment] = useState<string | null>(null);
  const [integrationOpen, setIntegrationOpen] = useState(false);
  const [integrationVersion, setIntegrationVersion] = useState(0);
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
  } = useSessionThread(sessionId);
  const running = view.running;
  const run = view.run;
  const waiting = runWaiting(run, running, view.lastDone);
  // The detail is fetched once; the event stream says how a turn ended.
  const state: SessionState = running
    ? "running"
    : ((view.lastDone as SessionState | undefined) ?? detail?.state ?? "idle");
  const status = sessionStatus(state, run, waiting);
  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);
  // The last scrollTop seen by onScroll: follow disengages only when it drops.
  const lastScrollTop = useRef(0);
  // Mirrors stickToBottom for rendering: the button that resumes it.
  const [awayFromBottom, setAwayFromBottom] = useState(false);
  // Set by `handleLoadOlder`: the distance from the bottom when the older
  // page is requested. Prepending changes scrollHeight but not this
  // distance, so restoring it keeps the same messages in view.
  const restoreScroll = useRef<number | null>(null);
  // Turning follow on jumps to the bottom at once: a smooth scroll chases a
  // bottom that moves while the agent writes and never re-engages follow.
  const setFollow = (on: boolean) => {
    stickToBottom.current = on;
    setAwayFromBottom(!on);
    const element = scrollRef.current;
    if (on && element) element.scrollTop = element.scrollHeight;
  };
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const modeRequestRef = useRef(0);
  const previousRunning = useRef(running);

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

  useEffect(() => {
    let active = true;
    getModelChoices()
      .then((data) => {
        if (active) setModelChoices(data);
      })
      .catch(() => {
        // The composer falls back to the session's model or '…'.
      });
    return () => {
      active = false;
    };
  }, []);

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
          if (idea) setText(idea.text);
        })
        .catch(() => {
          // Ignore: the composer stays editable with an empty message.
        });
      return () => {
        ignore = true;
      };
    }
  }, [projectId, frameIdeaId, sessionId]);

  useEffect(() => {
    if (view.pendingQuestion && !running) {
      textareaRef.current?.focus();
    }
  }, [view.pendingQuestion, running]);

  useEffect(() => {
    const element = scrollRef.current;
    if (element && stickToBottom.current) {
      element.scrollTop = element.scrollHeight;
    }
  }, [view, detail]);

  useEffect(() => {
    // Content that grows after a render (images, re-wrap) widens the distance
    // from the bottom with no view change, so the pin above never fires:
    // watch the content and re-pin while following.
    const element = scrollRef.current;
    const content = contentRef.current;
    if (!element || !content) return;
    const observer = new ResizeObserver(() => {
      if (stickToBottom.current) {
        element.scrollTop = element.scrollHeight;
      }
    });
    observer.observe(content);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const element = scrollRef.current;
    const previous = restoreScroll.current;
    if (element && previous != null) {
      element.scrollTop = element.scrollHeight - previous;
      restoreScroll.current = null;
    }
  }, [view]);

  const commandProjectId = detail?.project.id ?? projectId;

  useEffect(() => {
    if (!commandProjectId) {
      setCommands([]);
      return;
    }
    let ignore = false;
    listCommands(commandProjectId)
      .then((data) => {
        if (!ignore) setCommands(data);
      })
      .catch(() => {
        if (!ignore) setCommands([]);
      });
    return () => {
      ignore = true;
    };
  }, [commandProjectId]);

  useEffect(() => {
    if (!commandProjectId) {
      setAgents([]);
      return;
    }
    let ignore = false;
    listAgents(commandProjectId)
      .then((data) => {
        if (!ignore) setAgents(data.filter((agent) => !agent.subagent));
      })
      .catch(() => {
        if (!ignore) setAgents([]);
      });
    return () => {
      ignore = true;
    };
  }, [commandProjectId]);

  useEffect(() => {
    if (!commandProjectId) {
      setWorkflows([]);
      return;
    }
    let ignore = false;
    listWorkflows(commandProjectId)
      .then((data) => {
        if (!ignore) {
          setWorkflows(data);
          if (
            testWorkflowId &&
            data.some((workflow) => workflow.id === testWorkflowId)
          ) {
            setWorkflowSelection({
              workflowId: testWorkflowId,
              stepByStep: true,
              worktree: true,
            });
          } else if (frameIdeaId) {
            setWorkflowSelection({
              ...defaultWorkflowSelection(data),
              workflowId: null,
            });
          } else {
            setWorkflowSelection(defaultWorkflowSelection(data));
          }
        }
      })
      .catch(() => {
        if (!ignore) setWorkflows([]);
      });
    return () => {
      ignore = true;
    };
  }, [commandProjectId, testWorkflowId, frameIdeaId]);

  // A pending choice and preview belong to one session; do not leak them
  // across tabs.
  useEffect(() => {
    setPendingMode(null);
    setPendingModel(null);
    setPreview(null);
  }, [sessionId]);

  // A new-session tab has no detail yet: the server lists its default agent
  // first (the embedded defaults come before any file).
  const serverMode = view.mode ?? detail?.mode ?? agents[0]?.id ?? null;
  const mode = pendingMode ?? serverMode;

  useEffect(() => {
    // The pending value is only for the request gap; the server's mode is the
    // source of truth once it catches up.
    if (pendingMode && serverMode === pendingMode) {
      setPendingMode(null);
    }
  }, [pendingMode, serverMode]);

  const serverModel = detail?.model ?? modelChoices?.default ?? null;
  const currentModel: ModelChoice | null = pendingModel
    ? {
        provider: pendingModel.provider,
        model: pendingModel.model,
        providerName:
          modelChoices?.favorites.find((favorite) =>
            sameModelRef(favorite, pendingModel),
          )?.providerName ??
          (modelChoices && sameModelRef(modelChoices.default, pendingModel)
            ? modelChoices.default.providerName
            : pendingModel.provider),
      }
    : serverModel;

  useEffect(() => {
    if (
      pendingModel &&
      serverModel &&
      serverModel.provider === pendingModel.provider &&
      serverModel.model === pendingModel.model
    ) {
      setPendingModel(null);
    }
  }, [pendingModel, serverModel]);

  const title =
    view.name ?? detail?.title ?? (sessionId ? "Session" : "Nouvelle session");

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
    if (!sessionId || !detail?.worktree) return null;
    return { lastRunEndId, open: () => setIntegrationOpen(true) };
  }, [sessionId, detail?.worktree, lastRunEndId]);
  const projectName =
    detail?.project.name ??
    projects.find((project) => project.id === projectId)?.name ??
    projectId ??
    "—";

  async function handleSend(content?: string) {
    // Attachments belong to the composer; a quick answer (`content`) sends none.
    const fromComposer = content === undefined;
    const value = (content ?? text).trim();
    const images = fromComposer ? attachments : [];
    if ((!value && images.length === 0) || running) return;
    setSendError(null);

    // A quick answer or a message to a running/waiting run is an ordinary
    // message, not a workflow start.
    const workflowId =
      fromComposer &&
      workflowSelection.workflowId &&
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
        error instanceof Error ? error.message : "Erreur lors de l’envoi",
      );
    }
  }

  async function handleAttach(files: File[]) {
    const { attachments: read, error } = await readImages(
      files,
      attachments.length,
    );
    setAttachments((current) => [...current, ...read]);
    setSendError(error);
  }

  function handleRemoveAttachment(index: number) {
    setAttachments((current) => current.filter((_, i) => i !== index));
  }

  function sameModelRef(a: ModelRef, b: ModelRef): boolean {
    return a.provider === b.provider && a.model === b.model;
  }

  async function handleModeChange(id: string): Promise<boolean> {
    // A not-yet-created session keeps the choice locally; `handleSend` passes
    // it to `createSession`.
    if (!sessionId) {
      setPendingMode(id);
      return true;
    }

    const request = ++modeRequestRef.current;
    setPendingMode(id);
    setSendError(null);

    try {
      await setSessionMode(sessionId, id);
      return true;
    } catch (error) {
      // A newer request supersedes this one; only the latest may revert.
      if (request !== modeRequestRef.current) return true;
      setPendingMode(null);
      setSendError(
        error instanceof Error
          ? error.message
          : "Erreur lors du changement de mode",
      );
      return false;
    }
  }

  async function handleModelChange(ref: ModelRef): Promise<boolean> {
    // A not-yet-created session keeps the choice locally; `handleSend` passes
    // it to `createSession`.
    if (!sessionId) {
      setPendingModel(ref);
      return true;
    }

    const request = ++modelRequestRef.current;
    setPendingModel(ref);
    setSendError(null);

    try {
      await setSessionModel(sessionId, ref);
      setDetailVersion((version) => version + 1);
      setPendingModel(null);
      return true;
    } catch (error) {
      // A newer request supersedes this one; only the latest may revert.
      if (request !== modelRequestRef.current) return true;
      setPendingModel(null);
      setSendError(
        error instanceof Error
          ? error.message
          : "Erreur lors du changement de modèle",
      );
      return false;
    }
  }

  async function runPlanBuildAlone() {
    const modeChanged = await handleModeChange("builder");
    // Sent directly, not through handleSend: a workflow picked in the
    // composer would otherwise take this message as its task.
    if (modeChanged && sessionId) {
      await sendMessage(sessionId, "Exécute le plan ci-dessus.");
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
            )?.content ?? "Exécute le plan approuvé.";
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
        err instanceof Error ? err.message : "Impossible d’envoyer le message",
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
      setSendError(err instanceof Error ? err.message : "Erreur inconnue");
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
        err instanceof Error ? err.message : "Impossible d’envoyer le message",
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

  async function handleStop() {
    if (!sessionId) return;
    try {
      await stopSession(sessionId);
    } catch {
      // Stop is best-effort; the event stream reports the final state.
    }
  }

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

  async function handleCompact() {
    if (!sessionId) return;
    setCompactError(null);
    try {
      await compactSession(sessionId);
    } catch (error) {
      setCompactError(
        error instanceof Error ? error.message : "Erreur lors de la compaction",
      );
    }
  }

  async function handleReplay(messageId: string) {
    if (!sessionId) return;
    setSendError(null);
    try {
      await replayMessage(sessionId, messageId);
    } catch (error) {
      setSendError(
        error instanceof Error ? error.message : "Erreur lors de la relance",
      );
    }
  }

  async function handleLoadOlder() {
    const element = scrollRef.current;
    if (!element) return;
    restoreScroll.current = element.scrollHeight - element.scrollTop;
    // Clear the armed restore when the view did not change (failed or empty
    // page), so a later unrelated view change cannot apply a stale distance.
    if (!(await loadOlder())) restoreScroll.current = null;
  }

  return (
    <PreviewContext.Provider value={setPreview}>
      <PreviewWorkdirContext.Provider value={detail?.project.workdir}>
        <PlanActionsContext.Provider value={planActions}>
          <IntegrationInviteContext.Provider value={integrationInvite}>
            <CallUsageContext.Provider value={calls}>
              <WorkflowUsageContext.Provider value={workflowUsage}>
                <div className="flex min-h-0 min-w-0 grow">
                  {run && <RunRail run={run} />}
                  <main className="flex min-h-0 min-w-0 flex-[999_1_560px] flex-col">
                    {/* A shadow under the title: the thread fades under it instead of meeting a hard edge. */}
                    <div className="relative z-10 flex items-center gap-2 px-6 py-2.5 after:pointer-events-none after:absolute after:inset-x-0 after:top-full after:h-5 after:bg-linear-to-b after:from-black/45 after:to-transparent">
                      <div className="mx-auto flex w-full max-w-[960px] items-center gap-2">
                        <h1
                          className="min-w-0 truncate text-[0.9375rem] font-semibold"
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
                          title={detail?.project.workdir}
                        >
                          {projectName}
                        </span>
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
                        onScroll={(event) => {
                          const element = event.currentTarget;
                          const distance =
                            element.scrollHeight -
                            element.scrollTop -
                            element.clientHeight;
                          // Only a move up disengages: content growing during a run widens the
                          // distance without the human scrolling. Hysteresis: disengage beyond
                          // 120px, re-engage within 40px, so a small move does not toggle follow.
                          const movedUp = element.scrollTop < lastScrollTop.current;
                          lastScrollTop.current = element.scrollTop;
                          if (stickToBottom.current) {
                            if (movedUp && distance > 120) {
                              stickToBottom.current = false;
                              setAwayFromBottom(true);
                            }
                          } else if (distance <= 40) {
                            stickToBottom.current = true;
                            setAwayFromBottom(false);
                          }
                        }}
                        className="h-full overflow-y-auto p-6"
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
                                ? "Chargement…"
                                : "Charger les messages précédents"}
                            </Button>
                          )}
                          {view.items.length === 0 && !sessionId && (
                            <div className="py-10 text-center text-sm text-muted">
                              Nouvelle session · écris ton premier message.
                            </div>
                          )}
                          {view.items.map((item, index) => (
                            <ThreadItemView
                              key={item.id}
                              item={item}
                              model={
                                currentModel
                                  ? modelShortName(currentModel.model)
                                  : "modèle"
                              }
                              running={running}
                              showHeader={
                                view.items[index - 1]?.kind !== "assistant"
                              }
                              onReplay={sessionId ? handleReplay : undefined}
                            />
                          ))}
                        </div>
                      </div>
                      {awayFromBottom && !running && (
                        <Button
                          type="button"
                          variant="secondary"
                          size="sm"
                          iconOnly
                          title="Aller en bas et suivre la suite"
                          onClick={() => setFollow(true)}
                          aria-label="Aller en bas"
                          className="absolute bottom-4 right-6 rounded-full shadow-md backdrop-blur"
                        >
                          <ChevronDownIcon className="h-4 w-4" />
                        </Button>
                      )}
                    </div>

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
                      />
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
                              ? "Ce nœud du workflow tourne…"
                              : "Ce nœud du workflow est terminé."}
                          </div>
                          <div className="text-xs text-muted">
                            Session pilotée par le workflow, en lecture seule :
                            la suite se passe dans la session parente.
                          </div>
                        </div>
                        <a
                          href={`#/s/${detail.parentId}`}
                          className={buttonClass(
                            { variant: "primary", size: "sm" },
                            "shrink-0",
                          )}
                        >
                          ← Retour au workflow
                        </a>
                      </div>
                    )}
                    {running && !detail?.parentId && (
                      <RunningStrip
                        run={run}
                        following={!awayFromBottom}
                        onFollowChange={setFollow}
                      />
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
                          workflowSelection.workflowId
                            ? "Décris la tâche du workflow…"
                            : view.pendingQuestion && !running
                              ? "Ta réponse…"
                              : undefined
                        }
                        commands={commands}
                        agents={agents}
                        mode={mode}
                        onModeChange={handleModeChange}
                        attachments={attachments}
                        onAttach={handleAttach}
                        onRemoveAttachment={handleRemoveAttachment}
                        workflows={workflows}
                        workflowSelection={workflowSelection}
                        onWorkflowSelectionChange={setWorkflowSelection}
                      />
                    )}
                  </main>

                  <ContextPanel
                    view={view}
                    skills={skills}
                    usage={usage}
                    request={detail?.request ?? null}
                    hasSession={sessionId !== null}
                    partial={hasMore}
                    worktree={detail?.worktree ?? null}
                    sessionId={sessionId}
                    onWorktreeChanged={() =>
                      setDetailVersion((version) => version + 1)
                    }
                    integrationVersion={integrationVersion}
                  />
                  {sessionId && integrationOpen && (
                    <IntegrationDialog
                      sessionId={sessionId}
                      open
                      onClose={() => {
                        setIntegrationOpen(false);
                        setIntegrationVersion((version) => version + 1);
                      }}
                    />
                  )}
                  {preview && sessionId && (
                    <PreviewPane
                      sessionId={sessionId}
                      target={preview}
                      onClose={() => setPreview(null)}
                    />
                  )}
                </div>
              </WorkflowUsageContext.Provider>
            </CallUsageContext.Provider>
          </IntegrationInviteContext.Provider>
        </PlanActionsContext.Provider>
      </PreviewWorkdirContext.Provider>
    </PreviewContext.Provider>
  );
}
