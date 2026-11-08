import { useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { ArrowUpIcon, SpinnerIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { buttonClass } from '../ui/Button';
import { getUpdate, applyUpdate, getVersion, type UpdateCheck } from '../../lib/api';

// The human decides when to update; nothing installs on its own.
export function UpdateNotice() {
  const t = useT();
  const [state, setState] = useState<{
    check: UpdateCheck | null;
    installing: boolean;
  }>({
    check: null,
    installing: false,
  });
  const restartTimer = useRef<number | null>(null);

  useEffect(() => {
    const load = async () => {
      try {
        const check = await getUpdate();
        setState((prev) => ({ ...prev, check }));
      } catch {
        setState((prev) => ({ ...prev, check: prev.installing ? prev.check : null }));
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

  useEffect(() => {
    return () => {
      if (restartTimer.current !== null) window.clearInterval(restartTimer.current);
    };
  }, []);

  const handleClick = async () => {
    const { check, installing } = state;
    if (!check?.newer || installing) return;

    if (!window.confirm(t('updateConfirm', { latest: check.latest }))) return;

    setState((prev) => ({ ...prev, installing: true }));

    try {
      await applyUpdate();

      let tries = 0;
      const timer = window.setInterval(() => {
        void (async () => {
          tries += 1;

          try {
            const data = await getVersion();
            const current = check.current.replace(/^v/, '');
            const latest = data.version.replace(/^v/, '');
            if (current && latest && current !== latest) {
              window.clearInterval(timer);
              window.location.reload();
              return;
            }
          } catch {
            // The server may not be up yet.
          }

          if (tries >= 60) {
            window.clearInterval(timer);
            setState((prev) => ({ ...prev, installing: false }));
            window.alert(t('updateRestartTimeout'));
          }
        })();
      }, 1000);

      restartTimer.current = timer;
    } catch (error) {
      setState((prev) => ({ ...prev, installing: false }));
      const message = error instanceof Error ? error.message : String(error);
      window.alert(t('updateFailed', { error: message }));
    }
  };

  const { check, installing } = state;
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
      onClick={() => void handleClick()}
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
