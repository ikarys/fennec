import { useEffect, useRef, useState, type UIEvent } from 'react';
import type { SessionDetail } from '../lib/api';
import type { ThreadView } from '../lib/fold';

interface UseScrollFollowArgs {
  /** The session shown: the page stays mounted across sessions, so a new one re-engages follow. */
  sessionId: string | null;
  /** The thread view: a new view re-pins the bottom and restores a pending offset. */
  view: ThreadView;
  /** The session detail: its worktree path changes the content height. */
  detail: SessionDetail | null;
  /** Loads the older page; resolves to false when there is nothing to prepend. */
  loadOlder: () => Promise<boolean>;
}

/**
 * The thread's scroll: follows the bottom while the agent writes, disengages
 * on a move up (with hysteresis), and keeps the same messages in view when an
 * older page is prepended.
 */
export function useScrollFollow({ sessionId, view, detail, loadOlder }: UseScrollFollowArgs) {
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
  function setFollow(on: boolean) {
    stickToBottom.current = on;
    setAwayFromBottom(!on);
    const element = scrollRef.current;
    if (on && element) element.scrollTop = element.scrollHeight;
  }

  function onScroll(event: UIEvent<HTMLDivElement>) {
    const element = event.currentTarget;
    const distance =
      element.scrollHeight - element.scrollTop - element.clientHeight;
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
  }

  useEffect(() => {
    stickToBottom.current = true;
    setAwayFromBottom(false);
    restoreScroll.current = null;
  }, [sessionId]);

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

  async function handleLoadOlder() {
    const element = scrollRef.current;
    if (!element) return;
    restoreScroll.current = element.scrollHeight - element.scrollTop;
    // Clear the armed restore when the view did not change (failed or empty
    // page), so a later unrelated view change cannot apply a stale distance.
    if (!(await loadOlder())) restoreScroll.current = null;
  }

  return {
    scrollRef,
    contentRef,
    awayFromBottom,
    setFollow,
    onScroll,
    handleLoadOlder,
  };
}
