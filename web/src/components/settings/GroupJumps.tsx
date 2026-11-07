/** Long tabs are hard to scan; read groups from the DOM because many components render them. */
import { useCallback, useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import { useT } from '../../i18n';

type Group = { label: string; element: HTMLElement };

export function GroupJumps({ container, deps }: { container: RefObject<HTMLElement | null>; deps: unknown }) {
  const t = useT();
  const [groups, setGroups] = useState<Group[]>([]);
  const [activeIndex, setActiveIndex] = useState(0);
  const navRef = useRef<HTMLElement | null>(null);
  const groupsRef = useRef<Group[]>([]);
  // A clicked group stays active until the human scrolls: near the bottom it cannot reach the top, and the spy would name the one above it.
  const pinnedRef = useRef(false);

  useEffect(() => {
    groupsRef.current = groups;
  }, [groups]);

  const collect = useCallback(() => {
    const root = container.current;
    if (!root) return;

    const sections = Array.from(root.querySelectorAll('section')).filter((section) => {
      return !section.parentElement?.closest('section') && Boolean(section.querySelector(':scope > header h2'));
    });

    const next: Group[] = sections.map((section) => {
      const heading = section.querySelector(':scope > header h2');
      return {
        label: heading?.textContent?.trim() ?? '',
        element: section,
      };
    });

    next.forEach((group) => {
      group.element.style.scrollMarginTop = '56px';
    });

    setGroups((previous) => {
      const previousLabels = previous.map((group) => group.label).join('\u0000');
      const nextLabels = next.map((group) => group.label).join('\u0000');
      return previousLabels === nextLabels ? previous : next;
    });
  }, [container]);

  useEffect(() => {
    collect();
  }, [collect, deps]);

  useEffect(() => {
    setActiveIndex(0);
  }, [deps]);

  useEffect(() => {
    const root = container.current;
    if (!root) return;

    const observer = new MutationObserver(() => {
      collect();
    });
    observer.observe(root, { childList: true, subtree: true });

    return () => {
      observer.disconnect();
    };
  }, [collect, container]);

  useEffect(() => {
    const handleScroll = () => {
      const nav = navRef.current;
      const currentGroups = groupsRef.current;
      if (pinnedRef.current || !nav || currentGroups.length === 0) return;

      const threshold = nav.getBoundingClientRect().bottom + 24;
      let nextIndex = 0;

      currentGroups.forEach((group, index) => {
        if (group.element.getBoundingClientRect().top <= threshold) {
          nextIndex = index;
        }
      });

      setActiveIndex(nextIndex);
    };

    const unpin = () => {
      pinnedRef.current = false;
    };
    const intents = ['wheel', 'touchstart', 'keydown', 'pointerdown'] as const;

    window.addEventListener('scroll', handleScroll, { capture: true, passive: true });
    intents.forEach((name) => window.addEventListener(name, unpin, { capture: true, passive: true }));
    return () => {
      window.removeEventListener('scroll', handleScroll, { capture: true });
      intents.forEach((name) => window.removeEventListener(name, unpin, { capture: true }));
    };
  }, []);

  if (groups.length < 2) return null;

  const safeActiveIndex = Math.min(activeIndex, groups.length - 1);

  return (
    <nav
      ref={navRef}
      aria-label={t('settingsTabGroups')}
      className="sticky top-0 z-10 -mx-5 -mt-5 border-b border-border bg-panel px-5 pb-2 pt-3"
    >
      <div className="flex flex-wrap gap-1">
        {groups.map((group, index) => {
          const isActive = index === safeActiveIndex;
          return (
            <button
              key={index}
              type="button"
              aria-current={isActive ? 'true' : undefined}
              className={
                isActive
                  ? 'rounded-md bg-accent-soft px-2.5 py-1 text-xs font-medium text-accent-strong transition-colors'
                  : 'rounded-md px-2.5 py-1 text-xs text-muted transition-colors hover:bg-raised hover:text-text'
              }
              onClick={() => {
                pinnedRef.current = true;
                setActiveIndex(index);
                group.element.scrollIntoView({ behavior: 'smooth', block: 'start' });
              }}
            >
              {group.label}
            </button>
          );
        })}
      </div>
    </nav>
  );
}
