import { useEffect, useRef, useState } from 'react';
import { useT } from '../i18n';
import { applyUpdate, getVersion, type UpdateCheck } from '../lib/api';

const RESTART_POLL_MS = 1000;
const RESTART_MAX_TRIES = 60;

// The human decides when to update: `install` asks first, nothing installs on its own.
export function useInstallUpdate() {
  const t = useT();
  const [installing, setInstalling] = useState(false);
  const restartTimer = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (restartTimer.current !== null) window.clearInterval(restartTimer.current);
    };
  }, []);

  const install = async (check: UpdateCheck) => {
    if (!check.newer || installing) return;
    if (!window.confirm(t('updateConfirm', { latest: check.latest }))) return;

    setInstalling(true);

    try {
      await applyUpdate();

      let tries = 0;
      const timer = window.setInterval(() => {
        void (async () => {
          tries += 1;

          try {
            const data = await getVersion();
            const current = check.current.replace(/^v/, '');
            const running = data.version.replace(/^v/, '');
            if (current && running && current !== running) {
              window.clearInterval(timer);
              window.location.reload();
              return;
            }
          } catch {
            // The server may not be up yet.
          }

          if (tries >= RESTART_MAX_TRIES) {
            window.clearInterval(timer);
            setInstalling(false);
            window.alert(t('updateRestartTimeout'));
          }
        })();
      }, RESTART_POLL_MS);

      restartTimer.current = timer;
    } catch (error) {
      setInstalling(false);
      const message = error instanceof Error ? error.message : String(error);
      window.alert(t('updateFailed', { error: message }));
    }
  };

  return { installing, install };
}
