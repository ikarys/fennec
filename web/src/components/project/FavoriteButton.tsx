/** The star that pins a project; the server lists favourites first. */
import type { Project } from '../../lib/api';
import { StarIcon } from '../Icons';
import { Button } from '../ui/Button';

export function FavoriteButton({
  project,
  onToggle
}: {
  project: Project;
  onToggle: (projectId: string, favorite: boolean) => void;
}) {
  const label = project.favorite ? 'Retirer des favoris' : 'Mettre en favori';
  return (
    <Button
      variant="ghost"
      size="sm"
      iconOnly
      className={project.favorite ? 'text-accent' : ''}
      title={label}
      aria-label={label}
      aria-pressed={project.favorite}
      onClick={() => onToggle(project.id, !project.favorite)}
    >
      <StarIcon className="h-3.5 w-3.5" filled={project.favorite} />
    </Button>
  );
}
