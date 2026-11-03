/**
 * A group of a page: a small capitals label (an optional one-line description under it,
 * actions on the right) then the body, with no frame of its own: the page's card holds
 * its groups. A group whose edits are saved on their own takes `save`: a dot before the
 * label while it has unsaved edits, and its own "Enregistrer" under the body.
 */
import { createContext, useContext, useState, type ReactNode } from 'react';
import { Button } from './Button';
import { SectionLabel } from './SectionLabel';

// A Section inside a Section is a sub-group: its title steps down to the small label and extra space above it
// sets it apart from the previous sub-group's last row, so the levels read apart.
const Nested = createContext(false);

/** A group's own save: whether it has unsaved edits, and how to store them. */
export interface SectionSave {
  dirty: boolean;
  onSave: () => Promise<void>;
}

export interface SectionProps {
  title: ReactNode;
  description?: ReactNode;
  /** Header buttons (`Button size="sm"`, often `iconOnly`): what the group lets the human add. */
  actions?: ReactNode;
  className?: string;
  /** The body has no gap: rows (a list) run one under the other. */
  flush?: boolean;
  save?: SectionSave;
  children?: ReactNode;
}

export function Section({ title, description, actions, className = '', flush = false, save, children }: SectionProps) {
  const nested = useContext(Nested);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSave() {
    if (!save || saving) return;
    setSaving(true);
    setError(null);
    try {
      await save.onSave();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Impossible d’enregistrer');
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className={`flex flex-col gap-3 ${nested ? 'pt-4' : ''} ${className}`.trim()}>
      <header className={`flex min-h-7 items-center gap-3 ${nested ? '' : 'border-b border-border pb-2'}`.trim()}>
        <div className="flex min-w-0 flex-col gap-0.5">
          <SectionLabel as={nested ? 'h3' : 'h2'} className="flex items-center gap-1.5">
            {save?.dirty && <span className="h-1.5 w-1.5 rounded-full bg-accent" title="Modifications non enregistrées" />}
            {title}
          </SectionLabel>
          {description && <p className="m-0 text-xs text-muted">{description}</p>}
        </div>
        {actions && <div className="ml-auto flex shrink-0 items-center gap-1">{actions}</div>}
      </header>
      {children && (
        <Nested.Provider value>
          <div className={flush ? 'flex flex-col' : 'flex flex-col gap-4'}>{children}</div>
        </Nested.Provider>
      )}
      {save?.dirty && (
        <div className="flex items-center gap-3 pt-1">
          <Button type="button" variant="primary" size="sm" disabled={saving} onClick={() => void handleSave()}>
            {saving ? 'Enregistrement…' : 'Enregistrer'}
          </Button>
          <span className="text-xs text-muted">Au prochain tour</span>
          {error && <span className="text-xs text-err">{error}</span>}
        </div>
      )}
    </section>
  );
}
