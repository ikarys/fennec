/**
 * A popover that picks one value among grouped choices, for the composer's
 * toolbar. Not a native `<select>`: the browser draws a select's list itself
 * and ignores the theme tokens, so a dark theme got a white menu.
 *
 * A group's title sits on a tinted band, like a `Section`'s header, so it
 * never reads as a choice; a `collapsed` group folds its choices behind a
 * toggle in that band.
 */
import { useCallback, useRef, useState, type ReactNode } from 'react';
import { Button } from './Button';
import { Card } from './Card';
import { useDismissable } from '../../hooks/useDismissable';
import { ChevronDownIcon } from './Icons';

export interface Choice {
  value: string;
  label: string;
  /** Shown as the choice's tooltip. */
  description?: string;
  /** A coloured dot before the label (an agent's colour). */
  color?: string | null;
}

export interface ChoiceGroup {
  /** No title: the choices sit flush, with no band. */
  label?: string;
  choices: Choice[];
  /** When true the band is a toggle and the choices start hidden. */
  collapsed?: boolean;
}

export function ChoiceDot({ color }: { color?: string | null }) {
  return (
    <span
      className="h-1.5 w-1.5 shrink-0 rounded-full bg-muted"
      style={color ? { backgroundColor: color } : undefined}
    />
  );
}

export function ChoiceMenu({
  label,
  groups,
  value,
  onChange,
  disabled,
  title,
  trigger,
  align = 'left',
  width = 'w-56',
  direction = 'up',
  className = '',
}: {
  /** Names the menu for screen readers. */
  label: string;
  groups: ChoiceGroup[];
  value: string | null;
  onChange: (value: string) => void;
  disabled?: boolean;
  /** The trigger's tooltip; the selected choice's description when absent. */
  title?: string;
  /** The closed button's content. */
  trigger: ReactNode;
  align?: 'left' | 'right';
  width?: string;
  /** The menu opens above the trigger (the composer's toolbar); 'down' when it sits at the top of a column. */
  direction?: 'up' | 'down';
  /** Layout classes for the trigger button (`w-full` in a fixed-width column). */
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const [openGroups, setOpenGroups] = useState<Set<string>>(new Set());
  const containerRef = useRef<HTMLDivElement>(null);
  const close = useCallback(() => setOpen(false), []);
  useDismissable(open, containerRef, close);

  const selected = groups.flatMap((group) => group.choices).find((choice) => choice.value === value);

  return (
    <div ref={containerRef} className="relative">
      <Button
        variant="ghost"
        size="sm"
        disabled={disabled}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={label}
        title={title ?? selected?.description}
        className={className}
        onClick={() => setOpen((isOpen) => !isOpen)}
      >
        {trigger}
        {/* The chevron says it opens: without it the trigger reads as a label. */}
        <ChevronDownIcon
          className={`h-3.5 w-3.5 shrink-0 text-faint transition-transform ${
            (direction === 'up') !== open ? 'rotate-180' : ''
          }`}
        />
      </Button>
      {open && (
        <Card
          role="menu"
          aria-label={label}
          className={`absolute z-20 max-h-80 overflow-y-auto p-1 ${width} ${
            align === 'right' ? 'right-0' : 'left-0'
          } ${direction === 'down' ? 'top-full mt-1' : 'bottom-full mb-1'}`}
        >
          {groups.map((group, index) => {
            const groupKey = group.label ?? `group-${index}`;
            const isCollapsed = group.collapsed === true;
            const isGroupOpen = !isCollapsed || openGroups.has(groupKey) || group.choices.some((choice) => choice.value === value);
            return (
              <div key={groupKey}>
                {group.label ? (
                  isCollapsed ? (
                    <button
                      type="button"
                      className="m-0 flex w-full items-center gap-1 bg-raised px-2.5 py-1 text-xs font-semibold tracking-wide text-faint uppercase"
                      aria-expanded={isGroupOpen}
                      onClick={() => {
                        setOpenGroups((prev) => {
                          const next = new Set(prev);
                          if (next.has(groupKey)) {
                            next.delete(groupKey);
                          } else {
                            next.add(groupKey);
                          }
                          return next;
                        });
                      }}
                    >
                      <ChevronDownIcon
                        className={`h-3 w-3 shrink-0 transition-transform ${isGroupOpen ? '' : '-rotate-90'}`}
                      />
                      {group.label}
                    </button>
                  ) : (
                    <h3 className="m-0 bg-raised px-2.5 py-1 text-xs font-semibold tracking-wide text-faint uppercase">
                      {group.label}
                    </h3>
                  )
                ) : null}
                {isGroupOpen && (
                  <div className="flex flex-col py-0.5">
                    {group.choices.map((choice) => (
                      <Button
                        key={choice.value}
                        role="menuitem"
                        variant="ghost"
                        size="sm"
                        multiline
                        className="w-full"
                        title={choice.description}
                        onClick={() => {
                          onChange(choice.value);
                          close();
                        }}
                      >
                        <span className="flex w-full flex-col">
                          <span className="flex w-full items-center gap-1.5">
                            <span className="w-3 shrink-0 text-left">{choice.value === value ? '✓' : ''}</span>
                            {choice.color !== undefined ? <ChoiceDot color={choice.color} /> : null}
                            <span className="min-w-0 grow">{choice.label}</span>
                          </span>
                          {choice.description ? (
                            <span className="pl-[1.125rem] text-xs font-normal text-muted">{choice.description}</span>
                          ) : null}
                        </span>
                      </Button>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </Card>
      )}
    </div>
  );
}
