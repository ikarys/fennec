import { ListCard } from '../ui/ListCard';
import { useT } from '../../i18n';

interface OptInListProps {
  title: string;
  items: {
    id: string;
    label: string;
    mono?: boolean;
    description: string;
  }[];
  value: string[];
  onChange: (value: string[]) => void;
  empty: string;
}

export function OptInList({ title, items, value, onChange, empty }: OptInListProps) {
  const t = useT();

  function toggle(id: string, checked: boolean) {
    onChange(checked ? [...value, id] : value.filter((item) => item !== id));
  }

  if (items.length === 0) {
    return <p className="text-xs text-muted">{empty}</p>;
  }

  const active = items.filter((item) => value.includes(item.id)).length;
  const total = items.length;

  return (
    <ListCard title={title} count={t('optInCount', { active, total })}>
      <ul className="divide-y divide-border">
        {items.map((item) => {
          const checked = value.includes(item.id);
          return (
            <li key={item.id}>
              <label
                className={`flex cursor-pointer items-start gap-3 px-4 py-2.5 transition-colors hover:bg-accent-soft ${
                  checked ? 'bg-accent-soft/50' : ''
                }`}
              >
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={(event) => toggle(item.id, event.target.checked)}
                  className="mt-0.5 h-4 w-4 shrink-0"
                />
                <div className="min-w-0">
                  <div className={`text-sm text-text ${item.mono ? 'font-mono' : ''}`}>{item.label}</div>
                  <div className="text-xs text-muted">{item.description}</div>
                </div>
              </label>
            </li>
          );
        })}
      </ul>
    </ListCard>
  );
}
