import type { SandboxPrefs } from '../../lib/api';

interface SandboxSectionProps {
  value: SandboxPrefs;
  onChange: (value: SandboxPrefs) => void;
}

export function SandboxSection({ value, onChange }: SandboxSectionProps) {
  return (
    <section className="rounded-xl border border-border bg-panel px-5 py-4">
      <h2 className="text-[15px] font-semibold">Bac à sable</h2>
      <div className="mt-3">
        <label className="flex items-center gap-2 text-[13px] text-text">
          <input
            type="checkbox"
            checked={value.enabled}
            onChange={(event) => onChange({ ...value, enabled: event.target.checked })}
            className="h-4 w-4 accent-accent"
          />
          Exécuter les commandes dans bwrap
        </label>
        <p className="mt-1 text-xs text-muted">
          Les commandes du modèle et les nœuds shell des workflows voient le système en lecture seule, leur dossier de
          travail en écriture, sans tes secrets (~/.ssh, ~/.aws, …) ni la base de fennec. Désactivé : elles tournent sur
          ta machine, comme dans openfox.
        </p>
      </div>
      <div className="mt-3">
        <label className={`flex items-center gap-2 text-[13px] ${value.enabled ? 'text-text' : 'text-muted'}`}>
          <input
            type="checkbox"
            checked={value.network}
            disabled={!value.enabled}
            onChange={(event) => onChange({ ...value, network: event.target.checked })}
            className="h-4 w-4 accent-accent disabled:opacity-60"
          />
          Réseau pour les commandes
        </label>
        <p className="mt-1 text-xs text-muted">
          Coupé par défaut : npm et cargo passent hors ligne. À ouvrir pour installer des dépendances ou pour que le
          navigateur atteigne un serveur de dev. web_search n'en dépend pas : fennec l'exécute lui-même.
        </p>
      </div>
    </section>
  );
}
