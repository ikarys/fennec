import { useEffect, useRef, useState } from 'react';
import type { Agent, AttachmentInput, Config, Project, SessionDetail, SessionState, SlashCommand } from '../lib/api';
import { compactSession, createSession, getSession, listAgents, listCommands, replayMessage, sendMessage, setSessionMode, stopSession } from '../lib/api';
import { readImages } from '../lib/attachments';
import { stateLabel } from '../lib/format';
import { useSessionThread } from '../hooks/useSessionThread';
import { Composer } from '../components/session/Composer';
import { ContextPanel } from '../components/session/ContextPanel';
import { QuestionCard } from '../components/session/QuestionCard';
import { ThreadItemView } from '../components/session/Thread';

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
  // Optimistic mode while `setSessionMode` is in flight; cleared once the
  // server's mode (event stream or detail) matches it.
  const [pendingMode, setPendingMode] = useState<string | null>(null);
  const { view, hasMore, loadingOlder, loadOlder } = useSessionThread(sessionId);
  const running = view.running;
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottom = useRef(true);
  // Set by `handleLoadOlder`: the distance from the bottom when the older
  // page is requested. Prepending changes scrollHeight but not this
  // distance, so restoring it keeps the same messages in view.
  const restoreScroll = useRef<number | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const modeRequestRef = useRef(0);

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
  }, [sessionId]);

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
        if (!ignore) setAgents(data);
      })
      .catch(() => {
        if (!ignore) setAgents([]);
      });
    return () => {
      ignore = true;
    };
  }, [commandProjectId]);

  // A pending choice belongs to one session; do not leak it across tabs.
  useEffect(() => {
    setPendingMode(null);
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

    try {
      if (sessionId) {
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

  async function handleModeChange(id: string) {
    // A not-yet-created session keeps the choice locally; `handleSend` passes
    // it to `createSession`.
    if (!sessionId) {
      setPendingMode(id);
      return;
    }

    const request = ++modeRequestRef.current;
    setPendingMode(id);
    setSendError(null);

    try {
      await setSessionMode(sessionId, id);
    } catch (error) {
      // A newer request supersedes this one; only the latest may revert.
      if (request !== modeRequestRef.current) return;
      setPendingMode(null);
      setSendError(error instanceof Error ? error.message : 'Erreur lors du changement de mode');
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

        <div
          ref={scrollRef}
          onScroll={(event) => {
            const element = event.currentTarget;
            stickToBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80;
          }}
          className="min-h-0 grow overflow-y-auto p-6"
        >
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

        {view.pendingQuestion && !running && (
          <QuestionCard
            question={view.pendingQuestion}
            onAnswer={(answer) => handleSend(answer)}
            disabled={running}
          />
        )}
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
          placeholder={view.pendingQuestion && !running ? 'Ta réponse…' : undefined}
          commands={commands}
          agents={agents}
          mode={mode}
          onModeChange={handleModeChange}
          attachments={attachments}
          onAttach={handleAttach}
          onRemoveAttachment={handleRemoveAttachment}
        />
      </main>

      <ContextPanel
        view={view}
        currentTokens={currentTokens}
        contextPercent={contextPercent}
        compactDisabled={!sessionId || state === 'running' || state === 'waiting_for_user'}
        onCompact={handleCompact}
        compactError={compactError}
        partial={hasMore}
      />
    </div>
  );
}
