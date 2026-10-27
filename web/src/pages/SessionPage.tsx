import type { WorkflowSummary } from '../lib/workflow';
import { useEffect, useRef, useState } from 'react';
import type { Agent, AttachmentInput, Config, Project, SessionDetail, SessionState, SlashCommand } from '../lib/api';
import type { AssistantItem, Segment } from '../lib/fold';
import { splitContract } from '../lib/plan';
import {
  answerWorkflow,
  compactSession,
  continueWorkflow,
  createSession,
  getSession,
  listAgents,
  listCommands,
  listWorkflows,
  replayMessage,
  resumeWorkflow,
  sendMessage,
  setSessionMode,
  startWorkflow,
  stopSession,
  stopWorkflow
} from '../lib/api';
import { readImages } from '../lib/attachments';
import { stateLabel } from '../lib/format';
import { runWaiting } from '../lib/workflowRun';
import { useSessionThread } from '../hooks/useSessionThread';
import { Composer } from '../components/session/Composer';
import { ContextPanel } from '../components/session/ContextPanel';
import { QuestionCard } from '../components/session/QuestionCard';
import { ThreadItemView } from '../components/session/Thread';
import { PlanActionsContext } from '../components/session/PlanActions';
import { PreviewContext, PreviewWorkdirContext, type PreviewTarget } from '../components/preview/PreviewContext';
import { PreviewPane } from '../components/preview/PreviewPane';
import { RunStrip } from '../components/workflow/session/RunStrip';
import { RunActionCard } from '../components/workflow/session/RunActionCard';
import {
  DEFAULT_WORKFLOW_ID,
  defaultWorkflowSelection,
  type WorkflowSelection
} from '../components/workflow/session/WorkflowPicker';

interface SessionPageProps {
  sessionId: string | null;
  projectId: string | null;
  pseudoTabId: string | null;
  projects: Project[];
  config: Config | null;
  onSessionCreated: (newId: string, pseudoId: string | null) => void;
  /** Tabs and the palette show session states from the projects list. */
  onRunningChanged: () => void;
}


export function SessionPage({ sessionId, projectId, pseudoTabId, projects, config, onSessionCreated, onRunningChanged }: SessionPageProps) {
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [text, setText] = useState('');
  const [attachments, setAttachments] = useState<AttachmentInput[]>([]);
  const [sendError, setSendError] = useState<string | null>(null);
  const [compactError, setCompactError] = useState<string | null>(null);
  const [commands, setCommands] = useState<SlashCommand[]>([]);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [workflows, setWorkflows] = useState<WorkflowSummary[]>([]);
  const [workflowSelection, setWorkflowSelection] = useState<WorkflowSelection>({ workflowId: null, stepByStep: false, worktree: true });
  // Optimistic mode while `setSessionMode` is in flight; cleared once the
  // server's mode (event stream or detail) matches it.
  const [pendingMode, setPendingMode] = useState<string | null>(null);
  const [planBuildInFlight, setPlanBuildInFlight] = useState(false);
  const [preview, setPreview] = useState<PreviewTarget | null>(null);
  const [detailVersion, setDetailVersion] = useState(0);
  const { view, hasMore, loadingOlder, loadOlder, skills } = useSessionThread(sessionId);
  const running = view.running;
  const run = view.run;
  const waiting = runWaiting(run, running, view.lastDone);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);
  // Mirrors stickToBottom for rendering: the button that resumes it.
  const [awayFromBottom, setAwayFromBottom] = useState(false);
  // Set by `handleLoadOlder`: the distance from the bottom when the older
  // page is requested. Prepending changes scrollHeight but not this
  // distance, so restoring it keeps the same messages in view.
  const restoreScroll = useRef<number | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const modeRequestRef = useRef(0);
  const previousRunning = useRef(running);

  useEffect(() => {
    onRunningChanged();
  }, [running, onRunningChanged]);

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
          setWorkflowSelection(defaultWorkflowSelection(data));
        }
      })
      .catch(() => {
        if (!ignore) setWorkflows([]);
      });
    return () => {
      ignore = true;
    };
  }, [commandProjectId]);

  // A pending choice and preview belong to one session; do not leak them
  // across tabs.
  useEffect(() => {
    setPendingMode(null);
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

  // The detail is fetched once; the event stream says how a turn ended.
  const state: SessionState = running ? 'running' : ((view.lastDone as SessionState | undefined) ?? detail?.state ?? 'idle');
  const title = detail?.title ?? (sessionId ? 'Session' : 'Nouvelle session');

  const lastAssistant = [...view.items].reverse().find((item): item is AssistantItem => item.kind === 'assistant');
  const lastTextSegment = lastAssistant
    ? [...lastAssistant.segments].reverse().find((segment): segment is Extract<Segment, { kind: 'text' }> => segment.kind === 'text')
    : undefined;
  const planContract = lastTextSegment ? splitContract(lastTextSegment.content) : null;
  const planActions =
    planContract?.contract.kind === 'plan' &&
    lastTextSegment &&
    !running &&
    !view.pendingQuestion &&
    (!run || run.finished) &&
    mode === 'plan'
      ? {
          segmentId: lastTextSegment.id,
          onBuild: handlePlanBuild,
          onBuildAlone: handlePlanBuildAlone,
          onRevise: handlePlanRevise,
          disabled: running || planBuildInFlight
        }
      : null;
  const projectName =
    detail?.project.name ?? projects.find((project) => project.id === projectId)?.name ?? projectId ?? '—';
  const currentTokens = view.contextTokens ?? 0;
  const maxTokens = view.contextMax ?? config?.contextWindow ?? 0;
  const contextPercent = maxTokens > 0 ? Math.min(100, Math.round((currentTokens / maxTokens) * 100)) : 0;

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

    // A workflow start refuses attachments.
    if (workflowId && images.length > 0) {
      setSendError('Les pièces jointes ne sont pas acceptées pour un workflow.');
      return;
    }

    try {
      if (workflowId) {
        if (sessionId) {
          await startWorkflow(
            sessionId,
            workflowId,
            value,
            workflowSelection.stepByStep,
            workflowSelection.worktree
          );
        } else if (projectId) {
          const created = await createSession(projectId, value, mode ?? undefined, images, {
            workflowId,
            stepByStep: workflowSelection.stepByStep,
            worktree: workflowSelection.worktree
          });
          onSessionCreated(created.id, pseudoTabId);
        }
        // The next message gets the default workflow again.
        setWorkflowSelection(defaultWorkflowSelection(workflows));
      } else if (sessionId) {
        await sendMessage(sessionId, value, images);
      } else if (projectId) {
        // A mode chosen before creation is applied to the first turn.
        const created = await createSession(projectId, value, mode ?? undefined, images);
        onSessionCreated(created.id, pseudoTabId);
      }
      if (fromComposer) {
        setText('');
        setAttachments([]);
      }
    } catch (error) {
      setSendError(error instanceof Error ? error.message : 'Erreur lors de l’envoi');
    }
  }

  async function handleAttach(files: File[]) {
    const { attachments: read, error } = await readImages(files, attachments.length);
    setAttachments((current) => [...current, ...read]);
    setSendError(error);
  }

  function handleRemoveAttachment(index: number) {
    setAttachments((current) => current.filter((_, i) => i !== index));
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
      setSendError(error instanceof Error ? error.message : 'Erreur lors du changement de mode');
      return false;
    }
  }

  async function runPlanBuildAlone() {
    const modeChanged = await handleModeChange('builder');
    // Sent directly, not through handleSend: a workflow picked in the
    // composer would otherwise take this message as its task.
    if (modeChanged && sessionId) {
      await sendMessage(sessionId, 'Exécute le plan ci-dessus.');
    }
  }

  async function handlePlanBuild() {
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
            .find((item): item is Extract<typeof item, { kind: 'user' }> => item.kind === 'user' && !item.isSystemGenerated)
            ?.content ?? 'Exécute le plan approuvé.';
        await startWorkflow(
          sessionId,
          DEFAULT_WORKFLOW_ID,
          task,
          false,
          workflowSelection.worktree,
          lastTextSegment.content
        );
        // The next message gets the default workflow again.
        setWorkflowSelection(defaultWorkflowSelection(workflows));
      } else {
        await runPlanBuildAlone();
      }
    } catch (err) {
      setSendError(err instanceof Error ? err.message : 'Impossible d’envoyer le message');
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
      setSendError(err instanceof Error ? err.message : 'Impossible d’envoyer le message');
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
      setCompactError(error instanceof Error ? error.message : 'Erreur lors de la compaction');
    }
  }

  async function handleReplay(messageId: string) {
    if (!sessionId) return;
    setSendError(null);
    try {
      await replayMessage(sessionId, messageId);
    } catch (error) {
      setSendError(error instanceof Error ? error.message : 'Erreur lors de la relance');
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
        <div className="flex min-h-0 min-w-0 grow">
          <main className="flex min-h-0 min-w-0 flex-[999_1_560px] flex-col">
        <div className="flex flex-wrap items-center gap-2.5 px-6 pt-3.5">
          <div className="mx-auto flex w-full max-w-[820px] flex-wrap items-center gap-2.5">
            <h1 className="text-base font-semibold tracking-[-0.01em]">{title}</h1>
            <span className="inline-flex items-center gap-1.5 rounded-full bg-accent-soft px-2.5 py-0.5 text-xs font-medium text-accent-strong">
              <span className="h-1.5 w-1.5 rounded-full bg-accent" />
              {stateLabel(state)}
            </span>
            <span className="text-xs text-muted">{projectName}{detail ? ` · ${detail.project.workdir}` : ''}</span>
          </div>
        </div>

        {run && (
          <RunStrip run={run} running={running} onStop={handleStopWorkflow} />
        )}
        <div
          ref={scrollRef}
          onScroll={(event) => {
            const element = event.currentTarget;
            stickToBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80;
            setAwayFromBottom(!stickToBottom.current);
          }}
          className="min-h-0 grow overflow-y-auto p-6"
        >
          {awayFromBottom && (
            <div className="pointer-events-none sticky top-[calc(100%-3rem)] z-10 flex h-0 items-start justify-center">
              <button
                type="button"
                title="Aller en bas et suivre la suite"
                onClick={() => {
                  const element = scrollRef.current;
                  if (element) element.scrollTo({ top: element.scrollHeight, behavior: 'smooth' });
                }}
                className="pointer-events-auto rounded-full bg-accent px-3.5 py-1.5 text-xs font-medium text-on-accent shadow-lg hover:opacity-90"
              >
                ↓ En bas
              </button>
            </div>
          )}
          <div className="mx-auto flex max-w-[820px] flex-col gap-4.5">
            {sessionId && hasMore && (
              <button
                type="button"
                onClick={handleLoadOlder}
                disabled={loadingOlder}
                className="mx-auto rounded-lg border border-border bg-raised px-3 py-1.5 text-xs text-muted transition-colors hover:text-text disabled:opacity-60"
              >
                {loadingOlder ? 'Chargement…' : 'Charger les messages précédents'}
              </button>
            )}
            {view.items.length === 0 && !sessionId && (
              <div className="py-10 text-center text-sm text-muted">Nouvelle session · écris ton premier message.</div>
            )}
            {view.items.map((item, index) => (
              <ThreadItemView
                key={item.id}
                item={item}
                model={config?.model ?? 'modèle'}
                running={running}
                showHeader={view.items[index - 1]?.kind !== 'assistant'}
                onReplay={sessionId ? handleReplay : undefined}
              />
            ))}
          </div>
        </div>

        {view.pendingQuestion && !running && !waiting && !detail?.parentId && (
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
          <div className="px-6 py-3">
            <div className="mx-auto flex max-w-[820px] items-center gap-3 rounded-[14px] border border-border bg-panel px-4 py-3">
              <span
                className={`h-2 w-2 shrink-0 rounded-full ${running ? 'animate-pulse bg-accent' : 'bg-ok'}`}
              />
              <div className="min-w-0 grow">
                <div className="text-sm font-medium text-text">
                  {running ? 'Ce nœud du workflow tourne…' : 'Ce nœud du workflow est terminé.'}
                </div>
                <div className="text-xs text-muted">
                  Session pilotée par le workflow, en lecture seule : la suite se passe dans la session parente.
                </div>
              </div>
              <a
                href={`#/s/${detail.parentId}`}
                className="shrink-0 rounded-lg bg-accent px-3 py-1.5 text-sm font-medium text-on-accent hover:opacity-90"
              >
                ← Retour au workflow
              </a>
            </div>
          </div>
        )}
        {!detail?.parentId && (
          <Composer
            text={text}
            onTextChange={setText}
            onSend={() => handleSend()}
            onStop={handleStop}
            running={running}
            model={config?.model}
            currentTokens={currentTokens}
            maxTokens={maxTokens}
            error={sendError}
            textareaRef={textareaRef}
            placeholder={
              workflowSelection.workflowId
                ? 'Décris la tâche du workflow…'
                : view.pendingQuestion && !running
                  ? 'Ta réponse…'
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
            currentTokens={currentTokens}
            contextPercent={contextPercent}
            skills={skills}
            hasSession={sessionId !== null}
            compactDisabled={!sessionId || state === 'running' || state === 'waiting_for_user'}
            onCompact={handleCompact}
            compactError={compactError}
            partial={hasMore}
            worktree={detail?.worktree ?? null}
            sessionId={sessionId}
            onWorktreeChanged={() => setDetailVersion((version) => version + 1)}
          />
          {preview && sessionId && (
            <PreviewPane sessionId={sessionId} target={preview} onClose={() => setPreview(null)} />
          )}
        </div>
        </PlanActionsContext.Provider>
      </PreviewWorkdirContext.Provider>
    </PreviewContext.Provider>
  );
}
