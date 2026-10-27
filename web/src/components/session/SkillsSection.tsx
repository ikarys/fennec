import type { SessionSkills } from '../../lib/api';
import { panelBlock } from '../../lib/panelBlocks';
import { Panel } from '../ui/Panel';

interface SkillsSectionProps {
  skills: SessionSkills | null;
  hasSession: boolean;
}

export function SkillsSection({ skills, hasSession }: SkillsSectionProps) {
  return (
    <Panel
      id="skills"
      title={panelBlock('skills').title}
      summary={skills !== null && skills.loaded.length > 0 ? `${skills.active.length}/${skills.loaded.length}` : undefined}
    >
      <p className="m-0 text-[0.6875rem] text-muted">Les instructions spécialisées que le modèle a chargées. Point vert : encore dans son contexte.</p>
      {(!hasSession || (skills !== null && skills.loaded.length === 0)) && (
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
    </Panel>
  );
}
