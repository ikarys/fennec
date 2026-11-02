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
import { Section } from '../ui/Section';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import { TrashIcon } from '../Icons';
import { AgentEditor, EMPTY_DRAFT } from './AgentEditor';
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

const ORIGIN_SECTIONS: { origin: AgentOrigin; title: string; description: string; empty: string }[] = [
  {
    origin: 'user',
    title: 'Les tiens',
    description: 'Les fichiers de ~/.agents/agents : à toi de les écrire, de les modifier et de les supprimer.',
    empty: "Aucun agent à toi pour l'instant : duplique un défaut pour partir de sa méthode.",
  },
  {
    origin: 'default',
    title: 'Fournis par fennec',
    description: 'Embarqués dans le binaire, en lecture seule.',
    empty: 'Aucun.',
  },
  {
    origin: 'project',
    title: 'Du projet',
    description: 'Les fichiers de .agents/agents : ils appartiennent au dépôt, modifie-les dans le dépôt.',
    empty: 'Aucun.',
  },
];

const PHASE_LABELS: Record<string, string> = {
  start: 'Démarrer',
  frame: 'Cadrer',
  build: 'Construire',
  understand: 'Comprendre',
  check: 'Contrôler',
};

/** The draft a « Dupliquer » starts from: the default's surface, a free id. */
function duplicate(agent: AgentView): AgentDraft {
  return {
    ...EMPTY_DRAFT,
    id: `${agent.id}-mien`,
    name: `${agent.name} (le mien)`,
    description: agent.description,
    phase: agent.phase,
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
  return (
    <li className="flex items-center gap-3 px-4 py-2.5">
      <div className="min-w-0 grow">
        <div className="flex items-baseline gap-2">
          <span className="truncate text-sm font-medium">{agent.name}</span>
          <span className="truncate font-mono text-xs text-faint">{agent.id}</span>
          {agent.phase ? <Badge tone="neutral">{PHASE_LABELS[agent.phase] ?? agent.phase}</Badge> : null}
          {agent.readOnly ? <Badge tone="ok">lecture seule</Badge> : null}
          {agent.subagent ? <Badge tone="accent">sous-agent</Badge> : null}
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
      {agent.editable ? (
        <Button variant="ghost" onClick={onEdit} disabled={busy}>
          Modifier
        </Button>
      ) : (
        <Button variant="ghost" onClick={onDuplicate} disabled={busy}>
          Dupliquer
        </Button>
      )}
      {agent.editable ? (
        <Button variant="ghost" iconOnly title="Supprimer" aria-label="Supprimer" disabled={busy} onClick={onDelete}>
          <TrashIcon className="h-4 w-4" />
        </Button>
      ) : (
        <span className="w-7 shrink-0" />
      )}
    </li>
  );
}

export function AgentsSection() {
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
    setDraft(duplicate(agent));
  };

  const startEdit = async (id: string) => {
    setBusy(true);
    clearError();
    try {
      const { fields } = await getAgentSource(id);
      if (!fields) {
        setFormError(`Le fichier de « ${id} » ne se lit pas : corrige-le ou supprime-le.`);
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

  return (
    <>
      <Section
        title="Agents"
        description="Les modes d'une session : leur méthode et les outils qu'ils ont le droit d'utiliser. Un agent ajouté apparaît dans le menu du composer."
        actions={
          draft ? null : (
            <Button variant="primary" onClick={startNew} disabled={loading || busy}>
              Nouvel agent
            </Button>
          )
        }
      >
        {loading ? <p className="text-xs text-muted">Chargement…</p> : null}
        {!loading && error ? <p className="text-xs text-err">{error}</p> : null}
        {!loading && !error && draft ? (
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
        ) : null}
        {!loading && !error && !draft && formError ? <p className="text-xs text-err">{formError}</p> : null}
      </Section>

      {!loading && !error && overview
        ? ORIGIN_SECTIONS.map(({ origin, title, description, empty }) => {
            const agents = overview.agents.filter((agent) => agent.origin === origin);
            if (origin === 'project' && agents.length === 0) return null;
            return (
              <Section key={origin} title={title} description={description} flush>
                {agents.length === 0 ? (
                  <p className="px-4 py-3 text-xs text-muted">{empty}</p>
                ) : (
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
                )}
              </Section>
            );
          })
        : null}

      {overview && overview.broken.length > 0 ? (
        <Section
          title="Fichiers ignorés"
          description="Ces fichiers de ~/.agents/agents ne se lisent pas : leur agent n'apparaît dans aucun menu."
          flush
        >
          <ul className="divide-y divide-border">
            {overview.broken.map((file) => (
              <li key={file.id} className="flex items-center gap-3 px-4 py-2.5">
                <span className="font-mono text-sm">{file.id}</span>
                <span className="text-xs text-err">{file.error}</span>
                <Button
                  variant="ghost"
                  iconOnly
                  className="ml-auto"
                  title="Supprimer"
                  aria-label="Supprimer"
                  disabled={busy}
                  onClick={() =>
                    setPendingDelete({
                      id: file.id,
                      name: file.id,
                      description: '',
                      color: null,
                      subagent: false,
                      phase: null,
                      origin: 'user',
                      path: null,
                      editable: true,
                      readOnly: false,
                      allowedTools: [],
                    })
                  }
                >
                  <TrashIcon className="h-4 w-4" />
                </Button>
              </li>
            ))}
          </ul>
        </Section>
      ) : null}

      <ConfirmDialog
        open={pendingDelete !== null}
        title={pendingDelete ? `Supprimer l'agent « ${pendingDelete.name} » ?` : ''}
        body="Le fichier et sa méthode sont supprimés de ~/.agents/agents."
        confirmLabel="Supprimer"
        variant="danger"
        onConfirm={() => {
          if (!pendingDelete) return;
          const agent = pendingDelete;
          setPendingDelete(null);
          void doRemove(agent);
        }}
        onCancel={() => setPendingDelete(null)}
      />
    </>
  );
}
