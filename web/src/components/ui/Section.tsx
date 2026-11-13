/**
 * A group of a page: a small capitals label (an optional one-line description under it,
 * actions on the right) then the body, with no frame of its own: the page's card holds
 * its groups. A group whose edits are saved on their own takes `save`: a dot before the
 * label while it has unsaved edits. An auto-saved group says it is saving in its header;
 * the others (a change that needs a restart) keep their own "Enregistrer" under the body.
 * A group given an `Icon` at top level is framed: a card with a tinted header band.
 */
import { createContext, useContext, useState, type ReactNode } from 'react';
import { useT } from '../../i18n';
import { Button } from './Button';
import { SectionLabel } from './SectionLabel';
import type { PuzzleIcon } from './Icons';

// A Section inside a Section is a sub-group: its title steps down to the small label and extra space above it
// sets it apart from the previous sub-group's last row, so the levels read apart.
const Nested = createContext(false);

/** A group's own save: whether it has unsaved edits, and how to store them. */
export interface SectionSave {
  dirty: boolean;
  onSave: () => Promise<void>;
  /** When the saved value applies; the next turn when absent. */
  note?: string;
  /** Saved by the page shortly after an edit: no button, a status in the header. */
  auto?: { saving: boolean; error: string | null };
}

export interface SectionProps {
  title: ReactNode;
  description?: ReactNode;
  /** A top-level group with an icon is framed: a card with a tinted header band. */
  Icon?: typeof PuzzleIcon;
  /** Header buttons (`Button size="sm"`, often `iconOnly`): what the group lets the human add. */
  actions?: ReactNode;
  className?: string;
  /** The body has no gap: rows (a list) run one under the other. */
  flush?: boolean;
  save?: SectionSave;
  children?: ReactNode;
}

export function Section({ title, description, Icon, actions, className = '', flush = false, save, children }: SectionProps) {
  const nested = useContext(Nested);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

  async function handleSave() {
    if (!save || saving) return;
    setSaving(true);
    setError(null);
    try {
      await save.onSave();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('sectionSaveError'));
    } finally {
      setSaving(false);
    }
  }

  const dirtyDot = save?.dirty ? (
    <span className="h-1.5 w-1.5 rounded-full bg-accent" title={t('sectionUnsavedChanges')} />
  ) : null;

  const autoStatus =
    save?.auto && (save.auto.saving || save.auto.error) ? (
      <span className={`text-xs ${save.auto.error ? 'text-err' : 'text-muted'}`}>
        {save.auto.error ?? t('sectionSaving')}
      </span>
    ) : null;

  const saveFooter =
    save?.dirty && !save.auto ? (
      <>
        <Button type="button" variant="primary" size="sm" disabled={saving} onClick={() => void handleSave()}>
          {saving ? t('sectionSaving') : t('agentSave')}
        </Button>
        <span className="text-xs text-muted">{save.note ?? t('sectionNextTurn')}</span>
        {error && <span className="text-xs text-err">{error}</span>}
      </>
    ) : null;

  if (Icon && !nested) {
    return (
      <section className={`overflow-hidden rounded-xl border border-border bg-panel ${className}`.trim()}>
        <header className="flex min-h-11 items-center gap-2.5 border-b border-border bg-linear-to-r from-accent-soft to-transparent to-75% px-3 py-2">
          <span className="grid size-6 shrink-0 place-items-center rounded-md bg-accent-soft text-accent">
            <Icon className="h-3.5 w-3.5" />
          </span>
          <div className="flex min-w-0 flex-col">
            <h2 className="m-0 flex items-center gap-1.5 text-[13px] font-semibold text-text">
              {dirtyDot}
              {title}
            </h2>
            {description && <p className="m-0 text-xs text-muted">{description}</p>}
          </div>
          {autoStatus}
          {actions && <div className="ml-auto flex shrink-0 items-center gap-1">{actions}</div>}
        </header>
        {children && (
          <Nested.Provider value>
            <div className={flush ? 'flex flex-col' : 'flex flex-col gap-4 p-4'}>{children}</div>
          </Nested.Provider>
        )}
        {saveFooter && <div className="flex items-center gap-3 px-4 pb-3 pt-1">{saveFooter}</div>}
      </section>
    );
  }

  return (
    <section className={`flex flex-col gap-3 ${nested ? 'pt-4' : ''} ${className}`.trim()}>
      <header className={`flex min-h-7 items-center gap-3 ${nested ? '' : 'border-b border-border pb-2'}`.trim()}>
        <div className="flex min-w-0 flex-col gap-0.5">
          <SectionLabel as={nested ? 'h3' : 'h2'} className="flex items-center gap-1.5">
            {dirtyDot}
            {title}
          </SectionLabel>
          {description && <p className="m-0 text-xs text-muted">{description}</p>}
        </div>
        {autoStatus}
        {actions && <div className="ml-auto flex shrink-0 items-center gap-1">{actions}</div>}
      </header>
      {children && (
        <Nested.Provider value>
          <div className={flush ? 'flex flex-col' : 'flex flex-col gap-4'}>{children}</div>
        </Nested.Provider>
      )}
      {saveFooter && <div className="flex items-center gap-3 pt-1">{saveFooter}</div>}
    </section>
  );
}
