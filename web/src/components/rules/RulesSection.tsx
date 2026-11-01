import { useEffect, useState, type KeyboardEvent } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { FieldRow, Select, TextInput } from '../ui/Field';
import { Section } from '../ui/Section';
import { TrashIcon } from '../Icons';
import { addGlobalRule, addProjectRule, deleteRule, getGlobalRules, getProjectRules, type NewRule, type RuleAction, type RuleKind, type RuleView } from '../../lib/api/rules';

const KIND_LABELS: Record<RuleKind, string> = { command: 'commande', tool: 'outil', path: 'chemin' };
const ACTION_LABELS: Record<RuleAction, string> = { block: 'bloquer', ask: 'demander' };
const INTRO = "fennec applique ces règles lui-même avant chaque appel d'outil d'un agent et chaque commande d'un workflow : le modèle ne peut pas les contourner en le demandant. Une commande est comparée mot à mot (programme, mots, options), à travers sudo, sh -c, && et |. Une règle « demander » bloque aussi pour l'instant.";

function RuleRow({ rule, editable, deleting, onDelete }: { rule: RuleView; editable: boolean; deleting: boolean; onDelete: (id: string) => void }) {
  return (
    <li className="flex items-center gap-2 rounded-md border border-border p-3">
      <Badge tone="neutral" className="shrink-0">{KIND_LABELS[rule.kind]}</Badge>
      <div className="min-w-0 flex-1">
        <div className="truncate font-mono text-sm">{rule.pattern}</div>
        <div className="truncate font-mono text-xs text-muted">{rule.understood}</div>
        {rule.note ? <div className="truncate text-xs text-muted">{rule.note}</div> : null}
      </div>
      <Badge tone={rule.action === 'block' ? 'err' : 'accent'} className="shrink-0">{ACTION_LABELS[rule.action]}</Badge>
      {editable ? <Button variant="ghost" iconOnly title="Supprimer" aria-label="Supprimer" disabled={deleting} onClick={() => onDelete(rule.id)}><TrashIcon className="h-4 w-4" /></Button> : null}
    </li>
  );
}

function RuleList({ rules, editable, deletingId, onDelete }: { rules: RuleView[]; editable: boolean; deletingId: string | null; onDelete: (id: string) => void }) {
  if (rules.length === 0) return <p className="text-xs text-muted">Aucune règle.</p>;
  return (
    <ul className="flex flex-col gap-2">
      {rules.map((rule) => <RuleRow key={rule.id} rule={rule} editable={editable} deleting={deletingId === rule.id} onDelete={onDelete} />)}
    </ul>
  );
}

export function RulesSection({ projectId }: { projectId?: string }) {
  const [rules, setRules] = useState<RuleView[]>([]);
  const [globalRules, setGlobalRules] = useState<RuleView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [kind, setKind] = useState<RuleKind>('command');
  const [pattern, setPattern] = useState('');
  const [action, setAction] = useState<RuleAction>('block');
  const [note, setNote] = useState('');

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);
    const load = projectId
      ? getProjectRules(projectId).then((res) => { if (active) { setRules(res.rules); setGlobalRules(res.global); } })
      : getGlobalRules().then((res) => { if (active) setRules(res.rules); });
    load.catch((err: unknown) => { if (active) setError(err instanceof Error ? err.message : String(err)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [projectId]);

  const handleAdd = async () => {
    if (!pattern.trim() || busy) return;
    setBusy(true);
    setFormError(null);
    try {
      const payload: NewRule = { kind, pattern: pattern.trim(), action };
      if (note.trim()) payload.note = note.trim();
      if (projectId) {
        const res = await addProjectRule(projectId, payload);
        setRules(res.rules);
        setGlobalRules(res.global);
      } else {
        setRules((await addGlobalRule(payload)).rules);
      }
      setPattern('');
      setNote('');
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (id: string) => {
    setDeletingId(id);
    setFormError(null);
    try {
      await deleteRule(id);
      setRules((prev) => prev.filter((rule) => rule.id !== id));
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeletingId(null);
    }
  };

  // Settings wraps its sections in a form: Enter adds the rule instead of saving the settings.
  const addOnEnter = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      void handleAdd();
    }
  };

  const patternPlaceholder = kind === 'command' ? 'rm -rf' : kind === 'tool' ? 'web_search' : 'secrets/';
  const addForm = (
    <div className="flex flex-wrap items-end gap-3">
      <FieldRow label="Type" htmlFor="rule-kind">
        <Select id="rule-kind" value={kind} onChange={(e) => setKind(e.target.value as RuleKind)}>
          <option value="command">commande</option>
          <option value="tool">outil</option>
          <option value="path">chemin</option>
        </Select>
      </FieldRow>
      <FieldRow label="Motif" htmlFor="rule-pattern">
        <TextInput id="rule-pattern" value={pattern} placeholder={patternPlaceholder} onChange={(e) => setPattern(e.target.value)} onKeyDown={addOnEnter} />
      </FieldRow>
      <FieldRow label="Action" htmlFor="rule-action">
        <Select id="rule-action" value={action} onChange={(e) => setAction(e.target.value as RuleAction)}>
          <option value="block">bloquer</option>
          <option value="ask">demander</option>
        </Select>
      </FieldRow>
      <FieldRow label="Note" htmlFor="rule-note">
        <TextInput id="rule-note" value={note} placeholder="Pourquoi (optionnel)" onChange={(e) => setNote(e.target.value)} onKeyDown={addOnEnter} />
      </FieldRow>
      <Button variant="primary" onClick={handleAdd} disabled={!pattern.trim() || busy}>Ajouter</Button>
    </div>
  );

  return (
    <>
      <Section title={projectId ? 'Règles du projet' : 'Règles'} description={INTRO}>
        {loading ? <p className="text-xs text-muted">Chargement…</p> : null}
        {!loading && error ? <p className="text-xs text-err">{error}</p> : null}
        {!loading && !error ? <RuleList rules={rules} editable deletingId={deletingId} onDelete={handleDelete} /> : null}
        {formError ? <p className="text-xs text-err">{formError}</p> : null}
        {addForm}
      </Section>
      {projectId && !loading && !error ? (
        <Section title="Règles globales (dans les Réglages)">
          <RuleList rules={globalRules} editable={false} deletingId={null} onDelete={() => undefined} />
        </Section>
      ) : null}
    </>
  );
}
