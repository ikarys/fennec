/**
 * The human's agents, managed from the Settings page: fennec's defaults and a
 * project's `.agents/agents` are shown read-only (duplicate a default to start
 * from its method), the files of `~/.agents/agents` are edited and deleted.
 *
 * Autonomous like `RulesSection`: it fetches and writes on its own, outside the
 * preferences form, because an agent is a file and not a stored preference.
 */
import { useEffect, useState } from 'react';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { ListCard, listRowClass } from '../ui/ListCard';
import { Section } from '../ui/Section';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { AlertIcon, BotIcon, CpuIcon, EditIcon, PlusIcon, ProjectIcon, TrashIcon } from '../ui/Icons';
import { InfoBar } from '../ui/InfoBar';
import { AgentEditor, EMPTY_DRAFT } from './AgentEditor';
import { useT } from '../../i18n';
import type { MsgKey } from '../../i18n/messages';
import {
  deleteAgent,
  getAgentSource,
  getAgents,
  saveAgent,
  type AgentDraft,
  type AgentOrigin,
  type AgentView,
  type AgentsOverview,
} from '../../lib/api/agents';

const ORIGIN_SECTIONS: { origin: AgentOrigin; Icon: typeof BotIcon; title: MsgKey; description: MsgKey; empty: MsgKey }[] = [
  {
    origin: 'user',
    Icon: BotIcon,
    title: 'agentsUserTitle',
    description: 'agentsUserDescription',
    empty: 'agentsUserEmpty',
  },
  {
    origin: 'default',
    Icon: CpuIcon,
    title: 'agentsDefaultTitle',
    description: 'agentsDefaultDescription',
    empty: 'agentsNone',
  },
  {
    origin: 'project',
    Icon: ProjectIcon,
    title: 'agentsProjectTitle',
    description: 'agentsProjectDescription',
    empty: 'agentsNone',
  },
];

const PHASE_LABELS: Record<string, MsgKey> = {
  start: 'phaseStart',
  frame: 'phaseFrame',
  build: 'phaseBuild',
  understand: 'phaseUnderstand',
  check: 'phaseCheck',
};

/** The draft a duplicate starts from: the default's surface, a free id. */
function duplicate(agent: AgentView, t: (key: MsgKey, vars?: Record<string, string | number>) => string): AgentDraft {
  return {
    ...EMPTY_DRAFT,
    id: `${agent.id}-mien`,
    name: t('agentsDuplicateName', { name: agent.name }),
    description: agent.description,
    phase: agent.phase,
    workflow: agent.workflow,
    color: agent.color,
    subagent: agent.subagent,
    readOnly: agent.readOnly,
    allowedTools: agent.allowedTools,
    // The default's prompt is not served (it is fennec's, not a file of
    // theirs): the human writes the method, the surface is already right.
    prompt: '',
  };
}

function AgentRow({
  agent,
  busy,
  onEdit,
  onDuplicate,
  onDelete,
}: {
  agent: AgentView;
  busy: boolean;
  onEdit: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
}) {
  const t = useT();
  return (
    <li className={listRowClass()}>
      <div className="min-w-0 grow">
        <div className="flex items-baseline gap-2">
          <span className="truncate text-sm font-medium">{agent.name}</span>
          <span className="truncate font-mono text-xs text-faint">{agent.id}</span>
        </div>
        <div className="truncate text-xs text-muted" title={agent.description}>
          {agent.description}
        </div>
        {agent.path ? (
          <div className="truncate font-mono text-xs text-faint" title={agent.path}>
            {agent.path}
          </div>
        ) : null}
      </div>
      {/* Fixed-width columns, so the phase, the access and the kind line up from row to row. */}
      <span className="w-24 shrink-0">
        {agent.phase ? <Badge tone="neutral">{PHASE_LABELS[agent.phase] ? t(PHASE_LABELS[agent.phase]) : agent.phase}</Badge> : null}
      </span>
      <span className="w-24 shrink-0">{agent.readOnly ? <Badge tone="ok">{t('agentsReadOnly')}</Badge> : null}</span>
      <span className="w-20 shrink-0">
        {agent.subagent ? <Badge tone="accent" appearance="pill">{t('agentsSubagent')}</Badge> : <Badge tone="neutral" appearance="pill">{t('agentsAgent')}</Badge>}
      </span>
      {agent.editable ? (
        <Button variant="ghost" onClick={onEdit} disabled={busy}>
          {t('agentsEdit')}
        </Button>
      ) : (
        <Button variant="ghost" onClick={onDuplicate} disabled={busy}>
          {t('agentsDuplicate')}
        </Button>
      )}
      {agent.editable ? (
        <Button variant="danger" size="sm" iconOnly title={t('delete')} aria-label={t('delete')} disabled={busy} onClick={onDelete}>
          <TrashIcon className="h-3.5 w-3.5" />
        </Button>
      ) : (
        <span className="w-7 shrink-0" />
      )}
    </li>
  );
}

export function AgentsSection() {
  const t = useT();
  const [overview, setOverview] = useState<AgentsOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<AgentDraft | null>(null);
  const [newAgent, setNewAgent] = useState(true);
  const [busy, setBusy] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [formField, setFormField] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<AgentView | null>(null);

  const load = async (showLoading = true) => {
    if (showLoading) setLoading(true);
    setError(null);
    try {
      setOverview(await getAgents());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (showLoading) setLoading(false);
    }
  };

  useEffect(() => {
    void load(true);
  }, []);

  const clearError = () => {
    setFormError(null);
    setFormField(null);
  };

  const fail = (err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    setFormError(message);
    // The server names the field at fault in its message; `request` only
    // surfaces the message, so read it back from there.
    setFormField(message.includes('tool') ? 'allowedTools' : message.includes('prompt') ? 'prompt' : 'id');
  };

  const startNew = () => {
    clearError();
    setNewAgent(true);
    setDraft(EMPTY_DRAFT);
  };

  const startDuplicate = (agent: AgentView) => {
    clearError();
    setNewAgent(true);
    setDraft(duplicate(agent, t));
  };

  const startEdit = async (id: string) => {
    setBusy(true);
    clearError();
    try {
      const { fields } = await getAgentSource(id);
      if (!fields) {
        setFormError(t('agentsFileUnreadable', { id }));
        return;
      }
      setNewAgent(false);
      setDraft(fields);
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };

  const save = async () => {
    if (!draft) return;
    setBusy(true);
    clearError();
    try {
      await saveAgent({ ...draft, id: draft.id.trim() });
      setDraft(null);
      await load(false);
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };

  const doRemove = async (agent: AgentView) => {
    setBusy(true);
    clearError();
    try {
      await deleteAgent(agent.id);
      if (draft?.id === agent.id) setDraft(null);
      await load(false);
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };

  const list = (agents: AgentView[]) => (
    <ul className="divide-y divide-border">
      {agents.map((agent) => (
        <AgentRow
          key={agent.id}
          agent={agent}
          busy={busy}
          onEdit={() => void startEdit(agent.id)}
          onDuplicate={() => startDuplicate(agent)}
          onDelete={() => setPendingDelete(agent)}
        />
      ))}
    </ul>
  );

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <InfoBar>{t('agentsSectionDescription')}</InfoBar>
        </div>
        {draft ? null : (
          <Button variant="primary" onClick={startNew} disabled={loading || busy}>
            <PlusIcon className="h-3.5 w-3.5" />
            {t('agentsNew')}
          </Button>
        )}
      </div>
      {loading ? <p className="m-0 text-xs text-muted">{t('loading')}</p> : null}
      {!loading && error ? <p className="m-0 text-xs text-err">{error}</p> : null}
      {!loading && !error && draft ? (
        <Section Icon={EditIcon} title={newAgent ? t('agentsNew') : draft.name}>
          <AgentEditor
            draft={draft}
            tools={overview?.tools ?? []}
            busy={busy}
            error={formError}
            field={formField}
            newAgent={newAgent}
            onChange={setDraft}
            onSave={() => void save()}
            onCancel={() => {
              setDraft(null);
              clearError();
            }}
          />
        </Section>
      ) : null}
      {!loading && !error && !draft && formError ? <p className="m-0 text-xs text-err">{formError}</p> : null}

      {!loading && !error && overview
        ? ORIGIN_SECTIONS.map(({ origin, Icon, title, description, empty }) => {
            const agents = overview.agents.filter((agent) => agent.origin === origin);
            if (origin === 'project' && agents.length === 0) return null;
            return (
              <ListCard
                key={origin}
                Icon={Icon}
                title={t(title)}
                description={t(description)}
                count={agents.length}
                // Fennec's defaults never change: folded, they leave the human's own agents in view.
                folded={origin === 'default'}
              >
                {agents.length === 0 ? <p className="px-4 py-3 text-xs text-muted">{t(empty)}</p> : list(agents)}
              </ListCard>
            );
          })
        : null}

      {overview && overview.broken.length > 0 ? (
        <Section
          Icon={AlertIcon}
          title={t('agentsBrokenTitle')}
          description={t('agentsBrokenDescription')}
          flush
        >
          <ul className="divide-y divide-border">
            {overview.broken.map((file) => (
              <li key={file.id} className="flex items-center gap-3 px-4 py-2.5">
                <span className="font-mono text-sm">{file.id}</span>
                <span className="text-xs text-err">{file.error}</span>
                <Button
                  variant="danger"
                  size="sm"
                  iconOnly
                  className="ml-auto"
                  title={t('delete')}
                  aria-label={t('delete')}
                  disabled={busy}
                  onClick={() =>
                    setPendingDelete({
                      id: file.id,
                      name: file.id,
                      description: '',
                      color: null,
                      subagent: false,
                      phase: null,
                      workflow: null,
                      origin: 'user',
                      path: null,
                      editable: true,
                      readOnly: false,
                      allowedTools: [],
                    })
                  }
                >
                  <TrashIcon className="h-3.5 w-3.5" />
                </Button>
              </li>
            ))}
          </ul>
        </Section>
      ) : null}

      <ConfirmDialog
        open={pendingDelete !== null}
        title={pendingDelete ? t('agentsDeleteTitle', { name: pendingDelete.name }) : ''}
        body={t('agentsDeleteBody')}
        confirmLabel={t('delete')}
        variant="danger"
        onConfirm={() => {
          if (!pendingDelete) return;
          const agent = pendingDelete;
          setPendingDelete(null);
          void doRemove(agent);
        }}
        onCancel={() => setPendingDelete(null)}
      />
    </div>
  );
}
