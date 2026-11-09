// A vertical rail tab for the session context rail.
import type { PuzzleIcon } from '../ui/Icons';

interface RailTabProps {
  title: string;
  label: string;
  Icon: typeof PuzzleIcon;
  active: boolean;
  status?: string;
  statusClass?: string;
  alert?: boolean;
  onClick: () => void;
}

export function RailTab({
  title,
  label,
  Icon,
  active,
  status,
  statusClass,
  alert,
  onClick,
}: RailTabProps) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      aria-pressed={active}
      onClick={onClick}
      className={`relative flex h-16 w-full flex-col items-center justify-center gap-1 ${active ? 'bg-accent-soft text-accent' : 'text-muted hover:bg-raised hover:text-text'}`}
    >
      {active ? <span aria-hidden className="absolute inset-y-0 left-0 w-0.5 bg-accent" /> : null}
      {alert ? <span aria-hidden className="absolute right-2 top-2 size-1.5 rounded-full bg-err" /> : null}
      <Icon className="h-4 w-4" />
      <span className="max-w-full truncate px-1 text-[10px] leading-none">{label}</span>
      <span className={`text-[10px] leading-none font-mono tabular-nums ${statusClass ?? 'text-muted'}`}>{status ?? ''}</span>
    </button>
  );
}
