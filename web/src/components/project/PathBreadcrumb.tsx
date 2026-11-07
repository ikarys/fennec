import { useT } from '../../i18n';

interface PathBreadcrumbProps {
  path: string;
  onNavigate: (path: string) => void;
}

/** One button per segment of `path`, each opening that prefix. */
export function PathBreadcrumb({ path, onNavigate }: PathBreadcrumbProps) {
  const t = useT();
  const segments = path.split('/').filter(Boolean);
  const crumbClass = 'max-w-full truncate rounded-md px-1.5 py-0.5 text-xs text-muted hover:bg-raised';
  return (
    <nav aria-label={t('folderPathNav')} className="flex flex-wrap items-center gap-1">
      <button type="button" onClick={() => onNavigate('/')} title="/" className={crumbClass}>
        /
      </button>
      {segments.map((segment, index) => {
        const prefix = `/${segments.slice(0, index + 1).join('/')}`;
        return (
          <button key={prefix} type="button" onClick={() => onNavigate(prefix)} title={prefix} className={crumbClass}>
            {segment}
          </button>
        );
      })}
    </nav>
  );
}
