// Sending the composer's message: an ordinary message, a workflow start, or the first turn that creates the session; and attaching images.
import { useRef } from 'react';
import {
  createSession,
  sendMessage,
  startWorkflow,
  type AttachmentInput,
} from '../lib/api';
import { readImages } from '../lib/attachments';
import { frameIdea } from '../lib/ideas';
import { useLang, useT } from '../i18n';
import {
  defaultWorkflowSelection,
  type WorkflowSelection,
} from '../components/workflow/session/WorkflowPicker';
import type { ThreadView } from '../lib/fold';
import type { useModeAndModel } from './useModeAndModel';
import type { useProjectOptions } from './useProjectOptions';

type ProjectOptions = ReturnType<typeof useProjectOptions>;
type ModeAndModel = ReturnType<typeof useModeAndModel>;

interface UseSessionSendArgs {
  sessionId: string | null;
  projectId: string | null;
  frameIdeaId: string | null;
  pseudoTabId: string | null;
  isChat: boolean;
  running: boolean;
  run: ThreadView['run'];
  hasPendingQuestion: boolean;
  mode: string | null;
  pendingModel: ModeAndModel['pendingModel'];
  pendingEffort: ModeAndModel['pendingEffort'];
  draftMcpDisabled: string[] | null;
  workflows: ProjectOptions['workflows'];
  workflowSelection: WorkflowSelection;
  setWorkflowSelection: ProjectOptions['setWorkflowSelection'];
  text: string;
  setText: (text: string) => void;
  attachments: AttachmentInput[];
  setAttachments: (update: (current: AttachmentInput[]) => AttachmentInput[]) => void;
  setSendError: (error: string | null) => void;
  onSessionCreated: (newId: string, pseudoId: string | null) => void;
}

export function useSessionSend(args: UseSessionSendArgs) {
  const {
    sessionId,
    projectId,
    frameIdeaId,
    pseudoTabId,
    isChat,
    running,
    run,
    hasPendingQuestion,
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
  } = args;
  const lang = useLang();
  const t = useT();

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
      !hasPendingQuestion
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
            draftMcpDisabled ?? undefined,
            pendingEffort?.value ?? undefined,
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
          draftMcpDisabled ?? undefined,
          pendingEffort?.value ?? undefined,
        );
        if (frameIdeaId) {
          await frameIdea(projectId, frameIdeaId, created.id);
        }
        onSessionCreated(created.id, pseudoTabId);
      }
      if (fromComposer) {
        setText("");
        setAttachments(() => []);
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

  return { handleSend, handleAttach, handleRemoveAttachment };
}
