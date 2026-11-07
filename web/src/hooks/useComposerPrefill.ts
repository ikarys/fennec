import { useEffect, useRef } from 'react';
import { framingPrefill, listIdeas } from '../lib/ideas';
import { getSession } from '../lib/api';
import type { AttachmentInput } from '../lib/api';

interface UseComposerPrefillArgs {
  sessionId: string | null;
  projectId: string | null;
  frameIdeaId: string | null;
  fromSessionId: string | null;
  chooseAgent: (agent: string) => Promise<boolean>;
  setText: (text: string) => void;
  setAttachments: (attachments: AttachmentInput[]) => void;
}

export function useComposerPrefill({
  sessionId,
  projectId,
  frameIdeaId,
  fromSessionId,
  chooseAgent,
  setText,
  setAttachments
}: UseComposerPrefillArgs): void {
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
  const modeChangeRef = useRef(chooseAgent);
  modeChangeRef.current = chooseAgent;
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
}
