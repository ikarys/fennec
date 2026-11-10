// The composer's text and image attachments, with one unsent draft kept per session.
import { useEffect, useRef, useState } from 'react';
import type { AttachmentInput } from '../lib/api';

// One unsent draft per session, kept while the app lives: switching tabs must
// never carry a draft to another agent, and coming back must find it again.
const drafts = new Map<string, string>();

export function useComposerDraft(draftKey: string) {
  const [text, setText] = useState('');
  const [attachments, setAttachments] = useState<AttachmentInput[]>([]);
  const textRef = useRef(text);
  textRef.current = text;
  useEffect(() => {
    setText(drafts.get(draftKey) ?? '');
    // Saved when the key changes: the cleanup still sees the old key.
    return () => {
      drafts.set(draftKey, textRef.current);
    };
  }, [draftKey]);
  return { text, setText, attachments, setAttachments };
}
