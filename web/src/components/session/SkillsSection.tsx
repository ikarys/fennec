import type { SessionSkills } from '../../lib/api';
import { panelBlock } from '../../lib/panelBlocks';
import { Panel } from '../ui/Panel';

interface SkillsSectionProps {
  skills: SessionSkills | null;
  hasSession: boolean;
}

export function SkillsSection({ skills, hasSession }: SkillsSectionProps) {
  const loadedCount = skills === null ? 0 : new Set([...skills.loaded, ...skills.alwaysOn]).size;
  const activeCount = skills === null ? 0 : new Set([...skills.active, ...skills.alwaysOn]).size;
  const alwaysOnOnly = skills ? skills.alwaysOn.filter((id) => !skills.loaded.includes(id)) : [];

  return (
    <Panel
      id="skills"
      title={panelBlock('skills').title}
      summary={skills !== null && loadedCount > 0 ? `${activeCount}/${loadedCount}` : undefined}
    >
      <p className="m-0 text-[0.6875rem] text-muted">Les instructions spécialisées dans le contexte du modèle. Point vert : encore dans son contexte.</p>
      {(!hasSession || (skills !== null && skills.loaded.length === 0 && skills.alwaysOn.length === 0)) && (
        <div className="text-xs text-muted">Aucun skill chargé.</div>
      )}
      {hasSession && skills === null && <div className="text-xs text-muted">Chargement…</div>}
      {hasSession && skills !== null && skills.loaded.length > 0 && skills.loaded.map((id) => {
        const active = skills.active.includes(id);
        return (
          <div
            key={id}
            title={active ? id : 'Plus dans le contexte du modèle'}
            className="flex items-center gap-2 py-1.5 font-mono text-xs"
          >
            <span className={active ? undefined : 'text-muted'}>{id}</span>
            {active && <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-ok" />}
          </div>
        );
      })}
      {hasSession && skills !== null && alwaysOnOnly.length > 0 && alwaysOnOnly.map((id) => (
        <div
          key={`always-on-${id}`}
          title={id}
          className="flex items-center gap-2 py-1.5 font-mono text-xs"
        >
          <span>{id}</span>
          <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-ok" />
          <span className="text-[0.6875rem] text-muted">toujours actif</span>
        </div>
      ))}
      {hasSession && skills !== null && skills.missedAlwaysOn.map((id) => (
        <div
          key={`missed-${id}`}
          title="Coché « toujours actif » après le début de cette session : son prompt est figé"
          className="flex items-center gap-2 py-1.5 font-mono text-xs text-muted"
        >
          <span>{id}</span>
          <span className="font-sans text-[0.6875rem]">absent, tape /{id}</span>
        </div>
      ))}
    </Panel>
  );
}
