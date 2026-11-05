// The project's monthly budget tile: display-ready from the server, edited in place.
import { useEffect, useState } from 'react';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { TextInput } from '../ui/Field';
import { Tile } from '../ui/Tile';
import { CheckIcon, CloseIcon, CoinIcon, TrashIcon } from '../Icons';
import {
  getProjectBudget,
  setProjectBudget,
  type ProjectBudgetView,
} from '../../lib/api';
import { useT } from '../../i18n';

export function BudgetTile({ projectId }: { projectId: string }) {
  const t = useT();
  const [view, setView] = useState<ProjectBudgetView | null>(null);
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState('');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    getProjectBudget(projectId)
      .then((budget) => {
        if (!cancelled) setView(budget);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : t('unknownError'));
      });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  if (!view && error) return <p className="text-xs text-err">{error}</p>;
  if (!view) return <p className="text-xs text-muted">{t('loading')}</p>;

  if (!editing) {
    return (
      <Tile
        icon={<CoinIcon />}
        label={view.label}
        figure={view.figure}
        detail={view.detail}
        tone={view.tone}
        active={view.active}
        title={t('budgetEdit')}
        onClick={() => {
          setValue(view.limitUsd == null ? '' : String(view.limitUsd));
          setError(null);
          setEditing(true);
        }}
      />
    );
  }

  const save = async (limitUsd: number | null) => {
    setError(null);
    try {
      const next = await setProjectBudget(projectId, limitUsd);
      setView(next);
      setEditing(false);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : t('unknownError'));
    }
  };

  return (
    <Card className="flex flex-col gap-2 p-3">
      <TextInput
        type="number"
        min={0}
        step={1}
        value={value}
        placeholder={t('budgetPlaceholder')}
        aria-label={t('budgetPlaceholder')}
        onChange={(event) => setValue(event.target.value)}
      />
      {error ? <p className="m-0 text-xs text-err">{error}</p> : null}
      <div className="flex items-center justify-end gap-1">
        {view.limitUsd != null ? (
          <Button
            variant="danger"
            iconOnly
            title={t('budgetRemove')}
            onClick={() => void save(null)}
          >
            <TrashIcon />
          </Button>
        ) : null}
        <Button
          variant="icon"
          iconOnly
          title={t('budgetCancel')}
          onClick={() => {
            setError(null);
            setEditing(false);
          }}
        >
          <CloseIcon />
        </Button>
        <Button
          variant="icon"
          iconOnly
          title={t('budgetSave')}
          onClick={() => {
            const parsed = Number(value);
            if (!Number.isFinite(parsed) || parsed <= 0) {
              setError(t('budgetPlaceholder'));
              return;
            }
            void save(parsed);
          }}
        >
          <CheckIcon />
        </Button>
      </div>
    </Card>
  );
}
