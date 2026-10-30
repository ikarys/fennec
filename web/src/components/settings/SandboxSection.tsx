import type { SandboxPrefs } from '../../lib/api';
import { Section } from '../ui/Section';

interface SandboxSectionProps {
  value: SandboxPrefs;
  onChange: (value: SandboxPrefs) => void;
}

export function SandboxSection({ value, onChange }: SandboxSectionProps) {
  return (
    <Section title="Bac à sable">
      <div className="flex flex-col gap-1">
        <label className="flex items-center gap-2 text-sm text-text">
          <input
            type="checkbox"
            checked={value.enabled}
            onChange={(event) => onChange({ ...value, enabled: event.target.checked })}
            className="h-4 w-4 accent-[var(--accent)]"
          />
          Exécuter les commandes dans bwrap
        </label>
        <p className="text-xs text-muted">
          Les commandes du modèle et les nœuds shell des workflows voient le système en lecture seule, leur dossier de
          travail en écriture, sans tes secrets (~/.ssh, ~/.aws, …) ni la base de fennec. Désactivé : elles tournent sur
          ta machine, comme dans openfox.
        </p>
      </div>
      <div className="flex flex-col gap-1">
        <label className={`flex items-center gap-2 text-sm ${value.enabled ? 'text-text' : 'text-muted'}`}>
          <input
            type="checkbox"
            checked={value.network}
            disabled={!value.enabled}
            onChange={(event) => onChange({ ...value, network: event.target.checked })}
            className="h-4 w-4 accent-[var(--accent)] disabled:opacity-60"
          />
          Réseau pour les commandes
        </label>
        <p className="text-xs text-muted">
          Coupé par défaut : npm et cargo passent hors ligne. À ouvrir pour installer des dépendances ou pour que le
          navigateur atteigne un serveur de dev. web_search et web_fetch n'en dépendent pas : fennec les exécute lui-même.
        </p>
      </div>
    </Section>
  );
}
