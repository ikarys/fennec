import type { SkillSummary } from '../../lib/api';
import { Card } from '../ui/Card';

interface SkillsSectionProps {
  skills: SkillSummary[];
  value: string[];
  onChange: (value: string[]) => void;
}

export function SkillsSection({ skills, value, onChange }: SkillsSectionProps) {
  function toggle(id: string, checked: boolean) {
    onChange(checked ? [...value, id] : value.filter((skillId) => skillId !== id));
  }

  return (
    <Card className="p-4">
      <div className="flex flex-col gap-4">
        <h2 className="text-sm font-semibold">Skills toujours actifs</h2>
        <p className="text-xs text-muted">
          Leurs instructions entrent dans le prompt système des nouvelles sessions, sans load_skill. Une session déjà
          commencée garde son prompt.
        </p>
        {skills.length === 0 ? (
          <p className="text-xs text-muted">Aucun skill trouvé.</p>
        ) : (
          <div className="flex flex-col gap-1">
            {skills.map((skill) => (
              <div key={skill.id} className="flex flex-col gap-1">
                <label className="flex items-center gap-2 text-sm text-text">
                  <input
                    type="checkbox"
                    checked={value.includes(skill.id)}
                    onChange={(event) => toggle(skill.id, event.target.checked)}
                    className="h-4 w-4 accent-[var(--accent)]"
                  />
                  <span className="font-mono">{skill.id}</span>
                </label>
                <p className="text-xs text-muted">{skill.description}</p>
              </div>
            ))}
          </div>
        )}
      </div>
    </Card>
  );
}
