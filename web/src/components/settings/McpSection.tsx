import { useState } from 'react';
import { Section, type SectionSave } from '../ui/Section';
import { TextInput, Select } from '../ui/Field';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { connectMcp, type McpServer } from '../../lib/api';

function emptyServer(): McpServer {
  return {
    name: '',
    transport: 'stdio',
    command: '',
    args: [],
    env: {},
    url: '',
    headers: {},
    disabled: false,
    visible: true,
    disabledTools: [],
    cachedTools: [],
    timeout: 0
  };
}

function ServerCard({
  index,
  server,
  connecting,
  saved,
  onPatch,
  onRemove,
  onConnect
}: {
  index: number;
  server: McpServer;
  connecting: boolean;
  saved: boolean;
  onPatch: (patch: Partial<McpServer>) => void;
  onRemove: () => void;
  onConnect: () => void;
}) {
  function toggleTool(name: string) {
    const disabled = server.disabledTools.includes(name)
      ? server.disabledTools.filter((tool) => tool !== name)
      : [...server.disabledTools, name];
    onPatch({ disabledTools: disabled });
  }

  return (
    <Card variant="inset">
      <div className="flex items-center gap-2">
        <TextInput
          value={server.name}
          onChange={(event) => onPatch({ name: event.target.value })}
          placeholder="nom"
          className="w-40"
          aria-label="Nom du serveur MCP"
        />
        <Select
          value={server.transport}
          onChange={(event) => onPatch({ transport: event.target.value as McpServer['transport'] })}
          className="w-28"
          aria-label="Transport"
        >
          <option value="stdio">stdio</option>
          <option value="http">http</option>
        </Select>
        {server.transport === 'stdio' ? (
          <>
            <TextInput
              value={server.command ?? ''}
              onChange={(event) => onPatch({ command: event.target.value })}
              placeholder="commande"
              className="w-40"
              aria-label="Commande"
            />
            <TextInput
              value={server.args.join(' ')}
              onChange={(event) =>
                onPatch({ args: event.target.value.split(/\s+/).filter((part) => part !== '') })
              }
              placeholder="arguments"
              className="w-40"
              aria-label="Arguments"
            />
          </>
        ) : (
          <TextInput
            value={server.url ?? ''}
            onChange={(event) => onPatch({ url: event.target.value })}
            placeholder="https://…"
            className="w-64"
            aria-label="URL"
          />
        )}
        <div className="ml-auto flex items-center gap-2">
          <Button
            variant="soft"
            size="sm"
            onClick={onConnect}
            disabled={connecting || server.name === '' || !saved}
            title={!saved ? 'Enregistrez les réglages d’abord' : undefined}
          >
            {connecting ? 'Connexion…' : 'Connecter'}
          </Button>
          <Button variant="ghost" size="sm" iconOnly onClick={onRemove} aria-label="Supprimer">
            ×
          </Button>
        </div>
      </div>

      <div className="flex items-center gap-4">
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={!server.disabled}
            onChange={(event) => onPatch({ disabled: !event.target.checked })}
            className="h-4 w-4 accent-[var(--accent)]"
          />
          Actif
        </label>
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={server.visible}
            onChange={(event) => onPatch({ visible: event.target.checked })}
            className="h-4 w-4 accent-[var(--accent)]"
          />
          Affiché dans le composer
        </label>
        {server.cachedTools.length > 0 && (
          <span className="text-xs text-muted">{server.cachedTools.length} outil(s)</span>
        )}
      </div>

      <div className="flex items-center gap-2">
        <label className="text-sm text-text" htmlFor={`mcp-auth-${index}`}>
          Token d'authentification
        </label>
        <TextInput
          id={`mcp-auth-${index}`}
          type="password"
          value={server.authToken ?? ''}
          onChange={(event) => onPatch({ authToken: event.target.value || null })}
          placeholder="optionnel"
          className="w-64"
          aria-label="Token d'authentification"
        />
        <span className="text-xs text-muted">
          {server.transport === 'http'
            ? 'envoyé comme entête Bearer'
            : 'variable d’environnement MCP_AUTH_TOKEN'}
        </span>
      </div>

      {server.cachedTools.length > 0 && (
        <div className="flex flex-col gap-1">
          {server.cachedTools.map((tool) => (
            <label key={tool.name} className="flex items-center gap-2 text-sm text-text">
              <input
                type="checkbox"
                checked={!server.disabledTools.includes(tool.name)}
                onChange={() => toggleTool(tool.name)}
                className="h-4 w-4 accent-[var(--accent)]"
              />
              {server.name}_{tool.name}
              {tool.description ? (
                <span className="text-xs text-muted">— {tool.description}</span>
              ) : null}
            </label>
          ))}
        </div>
      )}
    </Card>
  );
}

export function McpSection({
  value,
  onChange,
  savedNames,
  save
}: {
  value: McpServer[];
  onChange: (value: McpServer[]) => void;
  savedNames: Set<string>;
  /** The group's own save: its dot and its "Enregistrer". */
  save?: SectionSave;
}) {
  const [connecting, setConnecting] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  function patch(index: number, p: Partial<McpServer>) {
    onChange(value.map((server, i) => (i === index ? { ...server, ...p } : server)));
  }

  async function connect(index: number) {
    const name = value[index].name;
    if (name === '') return;
    setConnecting(index);
    setError(null);
    try {
      const { server } = await connectMcp(name);
      patch(index, { cachedTools: server.cachedTools });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setConnecting(null);
    }
  }

  return (
    <Section
      title="MCP"
      description="Les serveurs d'outils du modèle (Model Context Protocol). « Actif » envoie leurs outils au modèle ; « Affiché » les montre dans le dropdown du composer. Connecter liste les outils d'un serveur. Le réglage par session du composer prime sur « Actif »."
      actions={
        <Button variant="primary" onClick={() => onChange([...value, emptyServer()])}>
          Nouveau serveur
        </Button>
      }
      save={save}
    >
      {error ? <p className="text-xs text-err">{error}</p> : null}
      {value.length === 0 ? (
        <p className="text-sm text-muted">Aucun serveur MCP.</p>
      ) : (
        value.map((server, index) => (
          <ServerCard
            key={index}
            index={index}
            server={server}
            connecting={connecting === index}
            saved={savedNames.has(server.name)}
            onPatch={(p) => patch(index, p)}
            onRemove={() => onChange(value.filter((_, i) => i !== index))}
            onConnect={() => connect(index)}
          />
        ))
      )}
    </Section>
  );
}
