import { useEffect, useState } from 'react';
import { useT } from '../../i18n';
import { ArrowUpIcon, SpinnerIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { buttonClass } from '../ui/Button';
import { getUpdate, type UpdateCheck } from '../../lib/api';
import { useInstallUpdate } from '../../hooks/useInstallUpdate';

export function UpdateNotice() {
  const t = useT();
  const [check, setCheck] = useState<UpdateCheck | null>(null);
  const { installing, install } = useInstallUpdate();

  useEffect(() => {
    const load = async () => {
      try {
        setCheck(await getUpdate());
      } catch {
        setCheck(null);
      }
    };

    const onFocus = () => {
      void load();
    };

    void load();
    window.addEventListener('focus', onFocus);

    return () => {
      window.removeEventListener('focus', onFocus);
    };
  }, []);

  if (!check?.newer && !installing) return null;

  const title = installing
    ? t('updateInstalling')
    : t('updateAvailableTitle', { current: check?.current ?? '', latest: check?.latest ?? '' });

  return (
    <button
      type="button"
      className={buttonClass({ variant: 'ghost', size: 'sm' }, 'text-accent')}
      title={title}
      aria-label={title}
      disabled={installing}
      onClick={() => {
        if (check) void install(check);
      }}
    >
      <span className="flex items-center gap-1">
        {installing ? (
          <SpinnerIcon className="h-4 w-4 animate-spin" />
        ) : (
          <ArrowUpIcon className="h-4 w-4" />
        )}
        {check?.latest ? <Badge tone="accent">{check.latest}</Badge> : null}
      </span>
    </button>
  );
}
