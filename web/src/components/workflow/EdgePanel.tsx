import type { WorkflowEdge, WorkflowIssue } from '../../lib/workflow';

interface EdgePanelProps {
  edge: WorkflowEdge;
  issues: WorkflowIssue[];
  onChange: (edge: WorkflowEdge) => void;
  onDelete: () => void;
}

export function EdgePanel({ edge, issues, onChange, onDelete }: EdgePanelProps) {
  const edgeIssues = issues;

  return (
    <div className="flex flex-col gap-3 p-3">
      <div>
        <div className="mb-1 text-[0.8125rem] text-muted">Lien</div>
        <div className="rounded-md bg-raised px-2 py-1 text-[0.8125rem]">
          {edge.from}.{edge.port} → {edge.to}
        </div>
      </div>

      <div>
        <label className="mb-1 block text-[0.8125rem] text-muted">Passages max</label>
        <input
          type="number"
          min={1}
          value={edge.maxPasses ?? ''}
          onChange={(event) => {
            const value = event.target.value;
            if (value === '') {
              onChange({ ...edge, maxPasses: undefined });
            } else {
              const n = Number(value);
              if (n >= 1) {
                onChange({ ...edge, maxPasses: n });
              }
            }
          }}
          className="w-full rounded-md border border-border bg-panel px-2 py-1 text-[0.8125rem]"
        />
        <div className="mt-1 text-xs text-muted">Le port exhausted de la source apparaît alors.</div>
      </div>

      <div>
        <label className="flex items-center gap-2 text-[0.8125rem]">
          <input
            type="checkbox"
            checked={edge.pause === true}
            onChange={(event) => onChange({ ...edge, pause: event.target.checked })}
            className="rounded"
          />
          Pause avant la cible
        </label>
      </div>

      {edgeIssues.length > 0 && (
        <div className="flex flex-col gap-1">
          <div className="text-[0.8125rem] font-medium text-err">Problèmes</div>
          {edgeIssues.map((issue, index) => (
            <div key={index} className="rounded-md bg-del-bg px-2 py-1 text-xs text-err">
              {issue.message}
            </div>
          ))}
        </div>
      )}

      <button
        type="button"
        onClick={onDelete}
        className="mt-2 rounded-md border border-err px-3 py-1 text-[0.8125rem] text-err hover:bg-del-bg"
      >
        Supprimer le lien
      </button>
    </div>
  );
}
