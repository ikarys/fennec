/** Theme preview tiles for choosing an interface theme. */
import { themes, type ThemeId } from '../../lib/theme';
import { CheckIcon } from '../ui/Icons';
import { useT } from '../../i18n';

export function ThemePicker({ value, onChange }: { value: ThemeId; onChange: (theme: ThemeId) => void }) {
  const t = useT();

  return (
    <div role="radiogroup" aria-label={t('theme')} className="grid grid-cols-2 gap-3 p-4 sm:grid-cols-4">
      {themes.map((theme) => {
        const selected = theme.id === value;

        return (
          <button
            key={theme.id}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => onChange(theme.id)}
            className={`rounded-lg border bg-bg p-2.5 text-left transition hover:-translate-y-0.5 focus-visible:outline-2 focus-visible:outline-accent ${
              selected ? 'border-accent ring-3 ring-accent-soft' : 'border-border hover:border-border-strong'
            }`}
          >
            <div data-theme={theme.id} aria-hidden="true" className="h-[74px] overflow-hidden rounded-md border border-black/40 bg-bg">
              <div className="flex h-[15px] items-center gap-1 border-b border-border bg-panel px-1.5">
                <span className="size-[5px] rounded-full bg-accent" />
                <span className="size-[5px] rounded-full bg-accent opacity-45" />
                <span className="size-[5px] rounded-full bg-accent opacity-45" />
              </div>
              <div className="mx-2 mt-[7px] h-[5px] w-[62%] rounded-sm bg-muted/30" />
              <div className="mx-2 mt-[7px] h-[5px] w-[38%] rounded-sm bg-accent" />
              <div className="mx-2 mt-[7px] h-[5px] w-[80%] rounded-sm bg-muted/30" />
            </div>
            <span className="mt-2 flex items-center justify-between text-xs font-semibold text-text">
              {theme.label}
              <CheckIcon className={`h-3.5 w-3.5 text-accent transition ${selected ? 'opacity-100' : 'opacity-0 scale-50'}`} />
            </span>
          </button>
        );
      })}
    </div>
  );
}
