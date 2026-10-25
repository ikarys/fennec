import type { ThreadView } from '../../lib/fold';
import { cachePercent, formatSpeed, formatTokens } from '../../lib/format';

interface ContextPanelProps {
  view: ThreadView;
  currentTokens: number;
  contextPercent: number;
  compactDisabled: boolean;
  onCompact: () => void;
  compactError: string | null;
}

export function ContextPanel({ view, currentTokens, contextPercent, compactDisabled, onCompact, compactError }: ContextPanelProps) {
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
          <button
            type="button"
            onClick={onCompact}
            disabled={compactDisabled}
            className="self-start rounded-lg border border-border bg-raised px-2.5 py-1 text-xs text-text disabled:opacity-60"
          >
            Compacter
          </button>
          {compactError && <div className="text-xs text-err">{compactError}</div>}
        </section>

        <section className="flex flex-col gap-1.5">
          <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">Tokens de la session</h2>
          {view.usage.calls === 0 && <div className="text-xs text-muted">Aucun appel mesuré.</div>}
          {view.usage.calls > 0 && (
            <dl className="m-0 grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-xs tabular-nums">
              <dt className="text-muted">Entrée</dt>
              <dd className="m-0 text-right">{formatTokens(view.usage.prefillTokens)}</dd>
              <dt className="text-muted">dont cache</dt>
              <dd className="m-0 text-right">
                {formatTokens(view.usage.cachedPromptTokens)} · {cachePercent(view.usage.cachedPromptTokens, view.usage.prefillTokens)} %
              </dd>
              <dt className="text-muted">Sortie</dt>
              <dd className="m-0 text-right">{formatTokens(view.usage.generationTokens)}</dd>
              {view.usage.last && (
                <>
                  <dt className="text-muted">Dernier appel</dt>
                  <dd className="m-0 text-right">{formatSpeed(view.usage.last.generationSpeed)}</dd>
                </>
              )}
              <dt className="text-muted">Appels</dt>
              <dd className="m-0 text-right">{view.usage.calls}</dd>
            </dl>
          )}
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
          Rien n'est intégré sans ta décision.
        </p>
      </aside>
  );
}
