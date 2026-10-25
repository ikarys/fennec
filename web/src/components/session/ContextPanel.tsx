import type { ThreadView } from '../../lib/fold';
import { formatTokens } from '../../lib/format';

interface ContextPanelProps {
  view: ThreadView;
  currentTokens: number;
  contextPercent: number;
}

export function ContextPanel({ view, currentTokens, contextPercent }: ContextPanelProps) {
  return (
      <aside
        aria-label="Contexte de la session"
        className="hidden min-[1100px]:flex flex-1 flex-col gap-5 border-l border-border bg-panel px-4 py-4.5 max-w-[300px] min-w-[240px]"
      >
        <section className="flex flex-col gap-2">
          <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Contexte</h2>
          <div className="h-1.5 overflow-hidden rounded-full bg-raised">
            <div className="h-full bg-accent" style={{ width: `${contextPercent}%` }} />
          </div>
          <div className="flex justify-between text-xs text-muted">
            <span>{formatTokens(currentTokens)} tokens</span>
            <span>{contextPercent} %</span>
          </div>
        </section>

        <section className="flex flex-col gap-1.5">
          <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Fichiers modifiés</h2>
          {view.modifiedFiles.length === 0 && <div className="text-xs text-muted">Aucun fichier modifié.</div>}
          {view.modifiedFiles.map((file) => (
            <div key={file.path} className="flex items-center gap-2 py-1.5 font-mono text-xs">
              <span className="grow overflow-hidden text-ellipsis whitespace-nowrap">{file.path}</span>
              <span className="text-ok">+{file.added}</span>
              <span className="text-muted">−{file.removed}</span>
            </div>
          ))}
        </section>

        <section className="flex flex-col gap-2">
          <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Outils de ce tour</h2>
          {view.toolCounts.length === 0 && <div className="text-xs text-muted">Aucun outil appelé.</div>}
          {view.toolCounts.map((tool) => (
            <div key={tool.name} className="flex justify-between text-xs">
              <span className="font-mono">{tool.name}</span>
              <span className="text-muted">{tool.count}</span>
            </div>
          ))}
        </section>

        <div className="grow" />
        <p className="m-0 rounded-lg bg-raised px-3 py-2.5 text-xs text-muted">
          Rien n'est intégré sans ta décision. Les outils tournent sur l'hôte : dépôt jetable conseillé.
        </p>
      </aside>
  );
}
