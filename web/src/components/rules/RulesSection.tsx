import { useEffect, useState, type KeyboardEvent, type ReactNode } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { FieldRow, Select, TextInput } from '../ui/Field';
import { Section } from '../ui/Section';
import { ExternalIcon, TrashIcon } from '../Icons';
import {
  addRule,
  deleteRule,
  getProjectRules,
  getRules,
  type NewRule,
  type RuleAccess,
  type RuleAction,
  type RuleKind,
  type RuleView,
  type RulesOverview,
} from '../../lib/api/rules';
import { getProjects } from '../../lib/api/projects';

type ProjectOption = Awaited<ReturnType<typeof getProjects>>[number];

const KIND_LABELS: Record<RuleKind, string> = {
  command: 'commande',
  tool: 'outil',
  path: 'chemin',
};

const ACTION_LABELS: Record<RuleAction, string> = {
  allow: 'autoriser',
  ask: 'demander',
  block: 'bloquer',
};

const ACTION_TONES: Record<RuleAction, 'ok' | 'accent' | 'err'> = {
  allow: 'ok',
  ask: 'accent',
  block: 'err',
};

const ACCESS_LABELS: Record<RuleAccess, string> = {
  read: 'lecture',
  write: 'écriture',
  readWrite: 'lecture et écriture',
};

function IconLink({
  href,
  title,
  label,
  children,
}: {
  href: string;
  title: string;
  label: string;
  children: ReactNode;
}) {
  return (
    <a
      href={href}
      title={title}
      aria-label={label}
      className="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted transition-colors hover:bg-raised hover:text-text"
    >
      {children}
    </a>
  );
}

function RuleRow({
  rule,
  editable,
  deleting,
  onDelete,
}: {
  rule: RuleView;
  editable: boolean;
  deleting: boolean;
  onDelete?: () => void;
}) {
  return (
    <li className="grid grid-cols-[5.5rem_minmax(0,1fr)_auto_2rem] items-center gap-3 px-4 py-2.5">
      <Badge tone="neutral">{KIND_LABELS[rule.kind]}</Badge>
      <div className="min-w-0">
        <div className="flex items-baseline gap-2">
          <span className="truncate font-mono text-sm" title={rule.pattern}>
            {rule.pattern}
          </span>
          {rule.kind === 'path' && rule.access ? (
            <Badge tone="neutral">{ACCESS_LABELS[rule.access]}</Badge>
          ) : null}
        </div>
        <div className="truncate text-xs text-faint" title={rule.understood}>
          {rule.understood}
        </div>
        {rule.note ? <div className="truncate text-xs text-muted italic">{rule.note}</div> : null}
      </div>
      <Badge tone={ACTION_TONES[rule.action]}>{ACTION_LABELS[rule.action]}</Badge>
      {editable ? (
        <Button
          variant="ghost"
          iconOnly
          title="Supprimer"
          aria-label="Supprimer"
          disabled={deleting}
          onClick={onDelete ?? (() => undefined)}
        >
          <TrashIcon className="h-4 w-4" />
        </Button>
      ) : (
        <span />
      )}
    </li>
  );
}

function RuleList({
  rules,
  editable,
  deletingId,
  onDelete,
  emptyMessage = 'Aucune règle.',
}: {
  rules: RuleView[];
  editable: boolean;
  deletingId: number | null;
  onDelete?: (id: number) => void;
  emptyMessage?: string;
}) {
  if (rules.length === 0) return <p className="px-4 py-3 text-xs text-muted">{emptyMessage}</p>;

  return (
    <ul className="divide-y divide-border">
      {rules.map((rule) => (
        <RuleRow
          key={rule.id}
          rule={rule}
          editable={editable}
          deleting={deletingId === rule.id}
          onDelete={onDelete ? () => onDelete(rule.id) : undefined}
        />
      ))}
    </ul>
  );
}

function RuleForm({ projects, onAdded }: { projects: ProjectOption[]; onAdded: () => void }) {
  const [scope, setScope] = useState('global');
  const [kind, setKind] = useState<RuleKind>('command');
  const [pattern, setPattern] = useState('');
  const [access, setAccess] = useState<RuleAccess>('readWrite');
  const [action, setAction] = useState<RuleAction>('block');
  const [note, setNote] = useState('');
  const [busy, setBusy] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const submit = async () => {
    if (!pattern.trim() || busy) return;
    setBusy(true);
    setFormError(null);
    try {
      const payload: NewRule = {
        scope: scope === 'global' ? 'global' : { project: scope },
        kind,
        pattern: pattern.trim(),
        action,
      };
      if (kind === 'path') payload.access = access;
      if (note.trim()) payload.note = note.trim();
      await addRule(payload);
      setPattern('');
      setNote('');
      onAdded();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const addOnEnter = (e: KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      void submit();
    }
  };

  const placeholder = kind === 'command' ? 'rm -rf' : kind === 'tool' ? 'web_search' : '~/.ssh/';

  return (
    <div>
      <div
        className={
          kind === 'path'
            ? 'grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-[10rem_8rem_minmax(0,1fr)_auto_8rem]'
            : 'grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-[10rem_8rem_minmax(0,1fr)_8rem]'
        }
      >
        <FieldRow label="Portée" htmlFor="rule-scope">
          <Select id="rule-scope" className="w-full" value={scope} onChange={(e) => setScope(e.target.value)}>
            <option value="global">Globale</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name ?? project.id}
              </option>
            ))}
          </Select>
        </FieldRow>
        <FieldRow label="Type" htmlFor="rule-kind">
          <Select id="rule-kind" className="w-full" value={kind} onChange={(e) => setKind(e.target.value as RuleKind)}>
            <option value="command">commande</option>
            <option value="tool">outil</option>
            <option value="path">chemin</option>
          </Select>
        </FieldRow>
        <FieldRow label="Motif" htmlFor="rule-pattern">
          <TextInput
            id="rule-pattern"
            className="w-full font-mono"
            value={pattern}
            placeholder={placeholder}
            onChange={(e) => setPattern(e.target.value)}
            onKeyDown={addOnEnter}
          />
        </FieldRow>
        {kind === 'path' ? (
          <FieldRow label="Accès" htmlFor="rule-access">
            <Select
              id="rule-access"
              className="w-full"
              value={access}
              onChange={(e) => setAccess(e.target.value as RuleAccess)}
            >
              <option value="readWrite">lecture et écriture</option>
              <option value="read">lecture</option>
              <option value="write">écriture</option>
            </Select>
          </FieldRow>
        ) : null}
        <FieldRow label="Action" htmlFor="rule-action">
          <Select id="rule-action" className="w-full" value={action} onChange={(e) => setAction(e.target.value as RuleAction)}>
            <option value="block">bloquer</option>
            <option value="ask">demander</option>
            <option value="allow">autoriser</option>
          </Select>
        </FieldRow>
      </div>
      <div className="mt-3 flex items-end gap-3">
        <div className="min-w-0 flex-1">
          <FieldRow label="Note" htmlFor="rule-note">
            <TextInput
              id="rule-note"
              className="w-full"
              value={note}
              placeholder="Pourquoi (optionnel)"
              onChange={(e) => setNote(e.target.value)}
              onKeyDown={addOnEnter}
            />
          </FieldRow>
        </div>
        <Button variant="primary" onClick={submit} disabled={!pattern.trim() || busy}>
          Ajouter
        </Button>
      </div>
      {formError ? <p className="mt-2 text-xs text-err">{formError}</p> : null}
    </div>
  );
}

export function RulesSection() {
  const [overview, setOverview] = useState<RulesOverview | null>(null);
  const [projects, setProjects] = useState<ProjectOption[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<number | null>(null);

  const load = async (showLoading = true) => {
    if (showLoading) setLoading(true);
    setError(null);
    try {
      const [rules, projectList] = await Promise.all([getRules(), getProjects()]);
      setOverview(rules);
      setProjects(projectList);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (showLoading) setLoading(false);
    }
  };

  useEffect(() => {
    void load(true);
  }, []);

  const handleDelete = async (id: number) => {
    setDeletingId(id);
    setActionError(null);
    try {
      await deleteRule(id);
      await load(false);
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeletingId(null);
    }
  };

  return (
    <>
      <Section
        title="Règles"
        description="Appliquées par fennec avant chaque appel d'outil et chaque commande de workflow : le modèle ne peut pas les contourner."
      >
        <details>
          <summary className="cursor-pointer text-xs text-muted">Comment les règles sont comparées</summary>
          <ul className="mt-2 flex list-disc flex-col gap-1 pl-5 text-xs text-muted">
            <li>Une commande est comparée mot à mot (programme, mots, options), à travers sudo, sh -c, && et |.</li>
            <li>Une règle « autoriser » pour une commande ne couvre que cette commande exacte.</li>
            <li>
              Un chemin hors du projet est refusé, en lecture comme en écriture, sauf si une règle « autoriser » le nomme ;
              une règle « demander » met le tour en pause jusqu'à ta réponse.
            </li>
          </ul>
        </details>
        {loading ? <p className="text-xs text-muted">Chargement…</p> : null}
        {!loading && error ? <p className="text-xs text-err">{error}</p> : null}
        {!loading && !error ? (
          <>
            <RuleForm projects={projects} onAdded={() => void load(false)} />
            {actionError ? <p className="mt-2 text-xs text-err">{actionError}</p> : null}
          </>
        ) : null}
      </Section>
      {!loading && !error && overview ? (
        <>
          <Section title="Globales" flush>
            <RuleList
              rules={overview.global}
              editable
              deletingId={deletingId}
              onDelete={handleDelete}
              emptyMessage="Aucune règle globale."
            />
          </Section>
          {overview.projects.map((project) => (
            <Section
              key={project.id}
              title={
                project.name ?? (
                  <span>
                    Projet supprimé <span className="font-mono text-muted">{project.id}</span>
                  </span>
                )
              }
              flush
            >
              <RuleList rules={project.rules} editable deletingId={deletingId} onDelete={handleDelete} />
            </Section>
          ))}
          {overview.sessions.length > 0 ? (
            <Section title="Accordées dans une session" flush>
              {overview.sessions.map((session, index) => (
                <div key={session.id}>
                  <div
                    className={`flex items-center justify-between gap-2 border-b border-border bg-raised/50 px-4 py-1.5 ${
                      index > 0 ? 'border-t border-border' : ''
                    }`.trim()}
                  >
                    <span className={session.title ? 'truncate text-xs font-medium' : 'truncate text-xs text-muted'}>
                      {session.title ?? 'Session supprimée'}
                    </span>
                    {session.title ? (
                      <IconLink
                        href={`#/s/${encodeURIComponent(session.id)}`}
                        title="Ouvrir la session"
                        label="Ouvrir la session"
                      >
                        <ExternalIcon className="h-4 w-4" />
                      </IconLink>
                    ) : null}
                  </div>
                  <RuleList rules={session.rules} editable deletingId={deletingId} onDelete={handleDelete} />
                </div>
              ))}
            </Section>
          ) : null}
        </>
      ) : null}
    </>
  );
}

export function ProjectRulesSummary({ projectId }: { projectId: string }) {
  const [data, setData] = useState<{ rules: RuleView[]; global: RuleView[] } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);
    getProjectRules(projectId)
      .then((res) => {
        if (active) setData(res);
      })
      .catch((err: unknown) => {
        if (active) setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [projectId]);

  return (
    <>
      <Section
        title="Règles du projet"
        actions={
          <IconLink
            href="#/settings/rules"
            title="Gérer les règles dans les Réglages"
            label="Gérer les règles dans les Réglages"
          >
            <ExternalIcon className="h-4 w-4" />
          </IconLink>
        }
        flush
      >
        {loading ? <p className="px-4 py-3 text-xs text-muted">Chargement…</p> : null}
        {!loading && error ? <p className="px-4 py-3 text-xs text-err">{error}</p> : null}
        {!loading && !error ? <RuleList rules={data?.rules ?? []} editable={false} deletingId={null} /> : null}
      </Section>
      {!loading && !error ? (
        <>
          <p className="text-xs text-muted">Les règles globales s'appliquent aussi :</p>
          <Section title="Règles globales" flush>
            <RuleList rules={data?.global ?? []} editable={false} deletingId={null} />
          </Section>
        </>
      ) : null}
    </>
  );
}
