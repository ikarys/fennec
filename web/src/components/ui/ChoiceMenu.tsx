/**
 * A popover that picks one value among grouped choices, for the composer's
 * toolbar. Not a native `<select>`: the browser draws a select's list itself
 * and ignores the theme tokens, so a dark theme got a white menu.
 *
 * A group's title sits on a tinted band, like a `Section`'s header, so it
 * never reads as a choice.
 */
import { useCallback, useRef, useState, type ReactNode } from 'react';
import { Button } from './Button';
import { Card } from './Card';
import { useDismissable } from '../../hooks/useDismissable';

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
  trigger,
  align = 'left',
  width = 'w-56',
}: {
  /** Names the menu for screen readers. */
  label: string;
  groups: ChoiceGroup[];
  value: string | null;
  onChange: (value: string) => void;
  disabled?: boolean;
  /** The closed button's content. */
  trigger: ReactNode;
  align?: 'left' | 'right';
  width?: string;
}) {
  const [open, setOpen] = useState(false);
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
        title={selected?.description}
        onClick={() => setOpen((isOpen) => !isOpen)}
      >
        {trigger}
      </Button>
      {open && (
        <Card
          role="menu"
          aria-label={label}
          className={`absolute bottom-full z-10 mb-1 max-h-80 overflow-y-auto p-1 ${width} ${
            align === 'right' ? 'right-0' : 'left-0'
          }`}
        >
          {groups.map((group, index) => (
            <div key={group.label ?? `group-${index}`}>
              {group.label ? (
                <h3 className="m-0 bg-raised px-2.5 py-1 text-xs font-semibold tracking-wide text-faint uppercase">
                  {group.label}
                </h3>
              ) : null}
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
                    <span className="flex w-full items-center gap-1.5">
                      <span className="w-3 shrink-0 text-left">{choice.value === value ? '✓' : ''}</span>
                      {choice.color !== undefined ? <ChoiceDot color={choice.color} /> : null}
                      <span className="min-w-0 grow">{choice.label}</span>
                    </span>
                  </Button>
                ))}
              </div>
            </div>
          ))}
        </Card>
      )}
    </div>
  );
}
