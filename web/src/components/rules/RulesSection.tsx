import { useEffect, useState, type KeyboardEvent, type ReactNode } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { FieldRow, Select, TextInput } from '../ui/Field';
import { ListCard } from '../ui/ListCard';
import { Section } from '../ui/Section';
import { ExternalIcon, PinIcon, TrashIcon } from '../ui/Icons';
import { Tile } from '../ui/Tile';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';
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

const KIND_LABELS: Record<RuleKind, MsgKey> = {
  command: 'rulesKindCommand',
  tool: 'rulesKindTool',
  path: 'rulesKindPath',
};

const ACTION_LABELS: Record<RuleAction, MsgKey> = {
  allow: 'rulesActionAllow',
  ask: 'rulesActionAsk',
  block: 'rulesActionBlock',
};

const ACTION_TONES: Record<RuleAction, 'ok' | 'accent' | 'err'> = {
  allow: 'ok',
  ask: 'accent',
  block: 'err',
};

const ACCESS_LABELS: Record<RuleAccess, MsgKey> = {
  read: 'readAction',
  write: 'writeAction',
  readWrite: 'readWriteAction',
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
  const t = useT();
  return (
    <li className="grid grid-cols-[5.5rem_minmax(0,1fr)_auto_2rem] items-center gap-3 px-4 py-2.5 transition-colors hover:bg-accent-soft">
      <Badge tone="neutral">{t(KIND_LABELS[rule.kind])}</Badge>
      <div className="min-w-0">
        <div className="flex items-baseline gap-2">
          <span className="truncate font-mono text-sm" title={rule.pattern}>
            {rule.pattern}
          </span>
          {rule.kind === 'path' && rule.access ? (
            <Badge tone="neutral">{t(ACCESS_LABELS[rule.access])}</Badge>
          ) : null}
        </div>
        <div className="truncate text-xs text-faint" title={rule.understood}>
          {rule.understood}
        </div>
        {rule.note ? <div className="truncate text-xs text-muted italic">{rule.note}</div> : null}
      </div>
      <Badge tone={ACTION_TONES[rule.action]}>{t(ACTION_LABELS[rule.action])}</Badge>
      {editable ? (
        <Button
          variant="ghost"
          iconOnly
          title={t('delete')}
          aria-label={t('delete')}
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
  emptyMessage,
}: {
  rules: RuleView[];
  editable: boolean;
  deletingId: number | null;
  onDelete?: (id: number) => void;
  emptyMessage?: string;
}) {
  const t = useT();
  if (rules.length === 0) return <p className="px-4 py-3 text-xs text-muted">{emptyMessage ?? t('rulesEmpty')}</p>;

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
  const t = useT();
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

  const placeholder = kind === 'command' ? t('rulesPatternCommandPlaceholder') : kind === 'tool' ? t('rulesPatternToolPlaceholder') : t('rulesPatternPathPlaceholder');

  return (
    <div>
      <div
        className={
          kind === 'path'
            ? 'grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-[10rem_8rem_minmax(0,1fr)_auto_8rem]'
            : 'grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-[10rem_8rem_minmax(0,1fr)_8rem]'
        }
      >
        <FieldRow label={t('rulesScopeLabel')} htmlFor="rule-scope">
          <Select id="rule-scope" className="w-full" value={scope} onChange={(e) => setScope(e.target.value)}>
            <option value="global">{t('rulesScopeGlobal')}</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name ?? project.id}
              </option>
            ))}
          </Select>
        </FieldRow>
        <FieldRow label={t('rulesTypeLabel')} htmlFor="rule-kind">
          <Select id="rule-kind" className="w-full" value={kind} onChange={(e) => setKind(e.target.value as RuleKind)}>
            <option value="command">{t('rulesKindCommand')}</option>
            <option value="tool">{t('rulesKindTool')}</option>
            <option value="path">{t('rulesKindPath')}</option>
          </Select>
        </FieldRow>
        <FieldRow label={t('rulesPatternLabel')} htmlFor="rule-pattern">
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
          <FieldRow label={t('rulesAccessLabel')} htmlFor="rule-access">
            <Select
              id="rule-access"
              className="w-full"
              value={access}
              onChange={(e) => setAccess(e.target.value as RuleAccess)}
            >
              <option value="readWrite">{t('readWriteAction')}</option>
              <option value="read">{t('readAction')}</option>
              <option value="write">{t('writeAction')}</option>
            </Select>
          </FieldRow>
        ) : null}
        <FieldRow label={t('rulesActionLabel')} htmlFor="rule-action">
          <Select id="rule-action" className="w-full" value={action} onChange={(e) => setAction(e.target.value as RuleAction)}>
            <option value="block">{t('rulesActionBlock')}</option>
            <option value="ask">{t('rulesActionAsk')}</option>
            <option value="allow">{t('rulesActionAllow')}</option>
          </Select>
        </FieldRow>
      </div>
      <div className="mt-3 flex items-end gap-3">
        <div className="min-w-0 flex-1">
          <FieldRow label={t('rulesNoteLabel')} htmlFor="rule-note">
            <TextInput
              id="rule-note"
              className="w-full"
              value={note}
              placeholder={t('rulesNotePlaceholder')}
              onChange={(e) => setNote(e.target.value)}
              onKeyDown={addOnEnter}
            />
          </FieldRow>
        </div>
        <Button variant="primary" onClick={submit} disabled={!pattern.trim() || busy}>
          {t('add')}
        </Button>
      </div>
      {formError ? <p className="mt-2 text-xs text-err">{formError}</p> : null}
    </div>
  );
}

export function RulesSection() {
  const t = useT();
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
        title={t('settingsTabRules')}
        description={t('rulesDescription')}
      >
        <details>
          <summary className="cursor-pointer text-xs text-muted">{t('rulesComparisonTitle')}</summary>
          <ul className="mt-2 flex list-disc flex-col gap-1 pl-5 text-xs text-muted">
            <li>{t('rulesCommandComparison')}</li>
            <li>{t('rulesAllowExact')}</li>
            <li>
              {t('rulesPathOutside')}
            </li>
          </ul>
        </details>
        {loading ? <p className="text-xs text-muted">{t('loading')}</p> : null}
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
          <ListCard title={t('rulesGlobalSection')} count={overview.global.length}>
            <RuleList
              rules={overview.global}
              editable
              deletingId={deletingId}
              onDelete={handleDelete}
              emptyMessage={t('rulesGlobalEmpty')}
            />
          </ListCard>
          {overview.projects.map((project) => (
            <ListCard
              key={project.id}
              title={
                project.name ?? (
                  <span>
                    {t('rulesDeletedProject')} <span className="font-mono text-muted">{project.id}</span>
                  </span>
                )
              }
              count={project.rules.length}
            >
              <RuleList rules={project.rules} editable deletingId={deletingId} onDelete={handleDelete} />
            </ListCard>
          ))}
          {overview.sessions.length > 0 ? (
            <ListCard
              title={t('rulesSessionGranted')}
              count={overview.sessions.reduce((sum, session) => sum + session.rules.length, 0)}
            >
              {overview.sessions.map((session, index) => (
                <div key={session.id}>
                  <div
                    className={`flex items-center justify-between gap-2 border-b border-border bg-raised/50 px-4 py-1.5 ${
                      index > 0 ? 'border-t border-border' : ''
                    }`.trim()}
                  >
                    <span className={session.title ? 'truncate text-xs font-medium' : 'truncate text-xs text-muted'}>
                      {session.title ?? t('rulesDeletedSession')}
                    </span>
                    {session.title ? (
                      <IconLink
                        href={`#/s/${encodeURIComponent(session.id)}`}
                        title={t('openSession')}
                        label={t('openSession')}
                      >
                        <ExternalIcon className="h-4 w-4" />
                      </IconLink>
                    ) : null}
                  </div>
                  <RuleList rules={session.rules} editable deletingId={deletingId} onDelete={handleDelete} />
                </div>
              ))}
            </ListCard>
          ) : null}
        </>
      ) : null}
    </>
  );
}

export function ProjectRulesTile({ projectId }: { projectId: string }) {
  const t = useT();
  const [rules, setRules] = useState<RuleView[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setRules(null);
    setError(null);
    getProjectRules(projectId)
      .then((res) => {
        if (active) setRules(res.rules);
      })
      .catch((err: unknown) => {
        if (active) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      active = false;
    };
  }, [projectId]);

  return (
    <Tile
      icon={<PinIcon />}
      label={t('settingsTabRules')}
      figure={rules ? String(rules.length) : error ? '—' : '…'}
      detail={error ?? t('rulesTileDetail')}
      tone="accent"
      active={rules ? rules.length > 0 : false}
      onClick={() => {
        window.location.hash = '#/settings/rules';
      }}
    />
  );
}
