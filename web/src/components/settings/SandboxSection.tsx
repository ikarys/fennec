import type { SandboxHealth, SandboxPrefs } from '../../lib/api';
import { CopyButton } from '../session/CopyButton';
import { Badge } from '../ui/Badge';
import { Section, type SectionSave } from '../ui/Section';

const FIX =
  "echo 'kernel.apparmor_restrict_unprivileged_userns=0' | sudo tee /etc/sysctl.d/60-bwrap-userns.conf && sudo sysctl --system";

interface SandboxSectionProps {
  value: SandboxPrefs;
  onChange: (value: SandboxPrefs) => void;
  health?: SandboxHealth;
  /** The group's own save: its dot and its "Enregistrer". */
  save?: SectionSave;
}

export function SandboxSection({ value, onChange, health, save }: SandboxSectionProps) {
  return (
    <Section title="Bac à sable" save={save}>
      {value.enabled && health && health.status !== 'ready' && (
        <div className="flex flex-col gap-1 rounded-md border border-err p-3">
          <Badge tone="err" className="self-start">bwrap indisponible</Badge>
          {health.status === 'missing' && (
            <p className="text-xs text-muted">
              bwrap n&apos;est pas installé : les commandes du modèle échoueront. Installe le paquet
              bubblewrap, puis recharge cette page.
            </p>
          )}
          {health.status === 'blocked' && (
            <>
              <p className="text-xs text-muted">
                bwrap ne peut pas créer son espace isolé : les commandes du modèle échoueront. Sur
                Ubuntu 23.10 et suivants, AppArmor interdit les user namespaces ; lance la commande
                ci-dessous puis recharge cette page (détails : docs/releasing.md).
              </p>
              {health.detail && <code className="font-mono text-xs text-err">{health.detail}</code>}
              <div className="flex items-center gap-2">
                <code
                  className="min-w-0 grow truncate rounded bg-surface px-2 py-1 font-mono text-xs"
                  title={FIX}
                >
                  {FIX}
                </code>
                <CopyButton text={FIX} />
              </div>
            </>
          )}
        </div>
      )}
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
          ta machine.
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
