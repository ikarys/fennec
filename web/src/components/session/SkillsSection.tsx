import type { SessionSkills } from '../../lib/api';
import { panelBlock } from '../../lib/panelBlocks';
import { WandIcon } from '../ui/Icons';
import { Panel } from '../ui/Panel';
import { useT } from '../../i18n';

interface SkillsSectionProps {
  skills: SessionSkills | null;
  hasSession: boolean;
}

export function SkillsSection({ skills, hasSession }: SkillsSectionProps) {
  const t = useT();
  const loadedCount = skills === null ? 0 : new Set([...skills.loaded, ...skills.alwaysOn]).size;
  const activeCount = skills === null ? 0 : new Set([...skills.active, ...skills.alwaysOn]).size;
  const alwaysOnOnly = skills ? skills.alwaysOn.filter((id) => !skills.loaded.includes(id)) : [];

  return (
    <Panel
      id="skills"
      title={t(panelBlock('skills').titleKey)}
      icon={<WandIcon className="h-3.5 w-3.5" />}
      summary={skills !== null && loadedCount > 0 ? `${activeCount}/${loadedCount}` : undefined}
    >
      <p className="m-0 text-xs text-muted">{t('skillsHint')}</p>
      {(!hasSession || (skills !== null && skills.loaded.length === 0 && skills.alwaysOn.length === 0)) && (
        <div className="text-xs text-muted">{t('noSkillsLoaded')}</div>
      )}
      {hasSession && skills === null && <div className="text-xs text-muted">{t('loading')}</div>}
      {hasSession && skills !== null && skills.loaded.length > 0 && skills.loaded.map((id) => {
        const active = skills.active.includes(id);
        return (
          <div
            key={id}
            title={active ? id : t('outOfContext')}
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
          <span className="text-xs text-muted">{t('alwaysActive')}</span>
        </div>
      ))}
      {hasSession && skills !== null && skills.missedAlwaysOn.map((id) => (
        <div
          key={`missed-${id}`}
          title={t('missedAlwaysOnTitle')}
          className="flex items-center gap-2 py-1.5 font-mono text-xs text-muted"
        >
          <span>{id}</span>
          <span className="font-sans text-xs">{t('absentTypeCommand', { id })}</span>
        </div>
      ))}
    </Panel>
  );
}
